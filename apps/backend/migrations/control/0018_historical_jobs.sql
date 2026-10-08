-- Archive evidence is immutable once verified. Workers retry their current phase.
CREATE TABLE archive_manifests (
 id UUID PRIMARY KEY,
 tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
 source_cell UUID NOT NULL REFERENCES cells(id),
 ownership_generation BIGINT NOT NULL CHECK(ownership_generation>0),
 period_start DATE NOT NULL CHECK(extract(day FROM period_start)=1),
 period_end DATE NOT NULL,
 timezone TEXT NOT NULL,
 schema_version BIGINT NOT NULL CHECK(schema_version>0),
 source_watermark BIGINT NOT NULL DEFAULT 0 CHECK(source_watermark>=0),
 row_count BIGINT CHECK(row_count>=0),
 checksum_sha256 TEXT CHECK(checksum_sha256 ~ '^[0-9a-f]{64}$'),
 objects JSONB NOT NULL DEFAULT '[]' CHECK(jsonb_typeof(objects)='array'),
 state TEXT NOT NULL DEFAULT 'PLANNED' CHECK(state IN ('PLANNED','EXPORTING','UPLOADED','VERIFIED','PURGING','COMPLETE')),
 purge_checkpoint JSONB NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(purge_checkpoint)='object'),
 rows_purged BIGINT NOT NULL DEFAULT 0 CHECK(rows_purged>=0),
 last_error TEXT,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 verified_at TIMESTAMPTZ,
 purged_at TIMESTAMPTZ,
 UNIQUE(id,tenant_id),
 UNIQUE(tenant_id,period_start),
 CHECK(period_end=(period_start+INTERVAL '1 month')::date),
 CHECK(rows_purged<=COALESCE(row_count,0)),
 CHECK(state IN ('PLANNED','EXPORTING') OR (row_count IS NOT NULL AND checksum_sha256 IS NOT NULL AND jsonb_array_length(objects)>0)),
 CHECK(state NOT IN ('VERIFIED','PURGING','COMPLETE') OR verified_at IS NOT NULL),
 CHECK(state<>'COMPLETE' OR (purged_at IS NOT NULL AND rows_purged=row_count)),
 CHECK(rows_purged=0 OR state IN ('PURGING','COMPLETE'))
);
CREATE INDEX archive_work_queue ON archive_manifests(source_cell,created_at) WHERE state<>'COMPLETE';
CREATE TABLE historical_backfills (
 id UUID PRIMARY KEY,
 tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
 source_archive UUID NOT NULL,
 target_schema_version BIGINT NOT NULL CHECK(target_schema_version>0),
 state TEXT NOT NULL DEFAULT 'PLANNED' CHECK(state IN ('PLANNED','DOWNLOADING','TRANSFORMING','VALIDATING','BACKFILLING','VERIFYING','COMPLETE')),
 checkpoint JSONB NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(checkpoint)='object'),
 rows_processed BIGINT NOT NULL DEFAULT 0 CHECK(rows_processed>=0),
 rows_failed BIGINT NOT NULL DEFAULT 0 CHECK(rows_failed>=0),
 staging_objects JSONB NOT NULL DEFAULT '[]' CHECK(jsonb_typeof(staging_objects)='array'),
 validation_checksum TEXT CHECK(validation_checksum ~ '^[0-9a-f]{64}$'),
 last_error TEXT,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 started_at TIMESTAMPTZ,
 completed_at TIMESTAMPTZ,
 FOREIGN KEY(source_archive,tenant_id) REFERENCES archive_manifests(id,tenant_id),
 CHECK(state IN ('PLANNED','DOWNLOADING','TRANSFORMING','VALIDATING') OR validation_checksum IS NOT NULL),
 CHECK(state<>'COMPLETE' OR (completed_at IS NOT NULL AND rows_failed=0))
);
CREATE INDEX backfill_work_queue ON historical_backfills(tenant_id,created_at) WHERE state<>'COMPLETE';
CREATE TABLE historical_exports (
 id UUID PRIMARY KEY,
 tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
 requested_by UUID NOT NULL REFERENCES users(id),
 period_start TIMESTAMPTZ NOT NULL,
 period_end TIMESTAMPTZ NOT NULL CHECK(period_end>period_start),
 filters JSONB NOT NULL DEFAULT '{}' CHECK(jsonb_typeof(filters)='object'),
 format TEXT NOT NULL CHECK(format IN ('CSV','CSV_GZ','PARQUET')),
 state TEXT NOT NULL DEFAULT 'QUEUED' CHECK(state IN ('QUEUED','PREPARING','SCANNING_ARCHIVE','GENERATING','UPLOADING','READY','FAILED','CANCELLED','EXPIRED')),
 source_objects JSONB NOT NULL DEFAULT '[]' CHECK(jsonb_typeof(source_objects)='array'),
 result_object_key TEXT,
 checksum_sha256 TEXT CHECK(checksum_sha256 ~ '^[0-9a-f]{64}$'),
 result_size_bytes BIGINT CHECK(result_size_bytes>=0),
 row_count BIGINT CHECK(row_count>=0),
 worker_cell UUID REFERENCES cells(id),
 worker_token UUID,
 worker_expires_at TIMESTAMPTZ,
 last_error TEXT,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 started_at TIMESTAMPTZ,
 ready_at TIMESTAMPTZ,
 expires_at TIMESTAMPTZ NOT NULL,
 CHECK(expires_at>created_at),
 CHECK((worker_cell IS NULL AND worker_token IS NULL AND worker_expires_at IS NULL) OR (worker_cell IS NOT NULL AND worker_token IS NOT NULL AND worker_expires_at IS NOT NULL)),
 CHECK(state NOT IN ('READY','EXPIRED') OR (result_object_key IS NOT NULL AND checksum_sha256 IS NOT NULL AND result_size_bytes IS NOT NULL AND row_count IS NOT NULL AND ready_at IS NOT NULL))
);
CREATE INDEX export_work_queue ON historical_exports(created_at) WHERE state IN ('QUEUED','PREPARING','SCANNING_ARCHIVE','GENERATING','UPLOADING');
CREATE INDEX export_tenant_queue ON historical_exports(tenant_id,state);

CREATE FUNCTION guard_historical_jobs() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE phases TEXT[]; item JSONB; expected_prefix TEXT; total BIGINT;
BEGIN
 IF TG_TABLE_NAME='archive_manifests' THEN
  phases:=ARRAY['PLANNED','EXPORTING','UPLOADED','VERIFIED','PURGING','COMPLETE'];
 ELSIF TG_TABLE_NAME='historical_backfills' THEN
  phases:=ARRAY['PLANNED','DOWNLOADING','TRANSFORMING','VALIDATING','BACKFILLING','VERIFYING','COMPLETE'];
 ELSE
  phases:=ARRAY['QUEUED','PREPARING','SCANNING_ARCHIVE','GENERATING','UPLOADING','READY'];
 END IF;
 IF TG_OP='INSERT' THEN
  IF NEW.state<>phases[1] THEN RAISE EXCEPTION 'Historical jobs must start in their initial state'; END IF;
 ELSE
  IF NEW.id<>OLD.id OR NEW.tenant_id<>OLD.tenant_id THEN RAISE EXCEPTION 'Historical job identity is immutable'; END IF;
  IF NEW.state<>OLD.state AND NOT COALESCE((
    array_position(phases,NEW.state)=array_position(phases,OLD.state)+1
    OR (TG_TABLE_NAME='historical_exports' AND ((NEW.state IN ('FAILED','CANCELLED') AND OLD.state=ANY(phases[1:5])) OR (OLD.state='READY' AND NEW.state='EXPIRED')))
   ),false) THEN RAISE EXCEPTION 'Invalid historical job transition: % -> %',OLD.state,NEW.state; END IF;
 END IF;
 IF TG_TABLE_NAME='archive_manifests' THEN
  IF TG_OP='UPDATE' THEN
   IF (NEW.period_start,NEW.period_end,NEW.timezone,NEW.schema_version,NEW.source_cell,NEW.ownership_generation) IS DISTINCT FROM (OLD.period_start,OLD.period_end,OLD.timezone,OLD.schema_version,OLD.source_cell,OLD.ownership_generation) THEN RAISE EXCEPTION 'Archive source identity is immutable'; END IF;
   IF OLD.state IN ('VERIFIED','PURGING','COMPLETE') AND (NEW.row_count,NEW.checksum_sha256,NEW.objects,NEW.source_watermark,NEW.verified_at) IS DISTINCT FROM (OLD.row_count,OLD.checksum_sha256,OLD.objects,OLD.source_watermark,OLD.verified_at) THEN RAISE EXCEPTION 'Verified archive evidence is immutable'; END IF;
   IF NEW.rows_purged<OLD.rows_purged THEN RAISE EXCEPTION 'Purge checkpoint cannot regress'; END IF;
  END IF;
  IF NEW.state NOT IN ('PLANNED','EXPORTING') THEN
   expected_prefix:='tenants/'||NEW.tenant_id||'/archive/'||to_char(NEW.period_start,'YYYY/MM')||'/'; total:=0;
   FOR item IN SELECT value FROM jsonb_array_elements(NEW.objects) LOOP
    IF jsonb_typeof(item)<>'object' OR COALESCE(item->>'key','') NOT LIKE expected_prefix||'%' OR strpos(item->>'key','..')>0 OR COALESCE(item->>'checksum_sha256','') !~ '^[0-9a-f]{64}$' OR COALESCE(item->>'rows','') !~ '^[0-9]+$' OR COALESCE(item->>'bytes','') !~ '^[0-9]+$' THEN RAISE EXCEPTION 'Invalid archive object evidence'; END IF;
    total:=total+(item->>'rows')::bigint;
   END LOOP;
   IF total<>NEW.row_count THEN RAISE EXCEPTION 'Archive object row totals differ'; END IF;
  END IF;
 ELSIF TG_TABLE_NAME='historical_backfills' THEN
  IF TG_OP='INSERT' AND NOT EXISTS(SELECT 1 FROM archive_manifests WHERE id=NEW.source_archive AND tenant_id=NEW.tenant_id AND state IN ('VERIFIED','PURGING','COMPLETE')) THEN RAISE EXCEPTION 'Backfill requires a verified source archive'; END IF;
  IF TG_OP='UPDATE' THEN
   IF (NEW.source_archive,NEW.target_schema_version) IS DISTINCT FROM (OLD.source_archive,OLD.target_schema_version) THEN RAISE EXCEPTION 'Backfill source identity is immutable'; END IF;
   IF NEW.rows_processed<OLD.rows_processed OR NEW.rows_failed<OLD.rows_failed THEN RAISE EXCEPTION 'Backfill counters cannot regress'; END IF;
   IF OLD.state IN ('BACKFILLING','VERIFYING','COMPLETE') AND (NEW.validation_checksum,NEW.staging_objects) IS DISTINCT FROM (OLD.validation_checksum,OLD.staging_objects) THEN RAISE EXCEPTION 'Validated staging evidence is immutable'; END IF;
  END IF;
 ELSE
  IF TG_OP='UPDATE' AND (NEW.requested_by,NEW.period_start,NEW.period_end,NEW.filters,NEW.format,NEW.expires_at) IS DISTINCT FROM (OLD.requested_by,OLD.period_start,OLD.period_end,OLD.filters,OLD.format,OLD.expires_at) THEN RAISE EXCEPTION 'Export request is immutable'; END IF;
  IF NEW.state IN ('READY','EXPIRED') AND (NEW.result_object_key NOT LIKE 'tenants/'||NEW.tenant_id||'/exports/'||NEW.id||'/%' OR strpos(NEW.result_object_key,'..')>0) THEN RAISE EXCEPTION 'Export result is outside its tenant/job prefix'; END IF;
  IF TG_OP='UPDATE' AND OLD.state IN ('READY','FAILED','CANCELLED','EXPIRED') AND (to_jsonb(NEW)-'state'-'updated_at') IS DISTINCT FROM (to_jsonb(OLD)-'state'-'updated_at') THEN RAISE EXCEPTION 'Terminal export evidence is immutable'; END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER archive_state_guard BEFORE INSERT OR UPDATE ON archive_manifests FOR EACH ROW EXECUTE FUNCTION guard_historical_jobs();
CREATE TRIGGER backfill_state_guard BEFORE INSERT OR UPDATE ON historical_backfills FOR EACH ROW EXECUTE FUNCTION guard_historical_jobs();
CREATE TRIGGER export_state_guard BEFORE INSERT OR UPDATE ON historical_exports FOR EACH ROW EXECUTE FUNCTION guard_historical_jobs();
