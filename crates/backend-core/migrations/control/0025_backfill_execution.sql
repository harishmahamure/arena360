ALTER TABLE historical_backfills ADD COLUMN source_objects JSONB NOT NULL DEFAULT '[]' CHECK(jsonb_typeof(source_objects)='array');
ALTER TABLE historical_backfills ADD COLUMN expected_rows BIGINT CHECK(expected_rows>=0);
ALTER TABLE historical_backfills ADD COLUMN batch_rows INTEGER NOT NULL DEFAULT 100 CHECK(batch_rows BETWEEN 1 AND 1000);
ALTER TABLE historical_backfills ADD COLUMN oltp_p99_target_milliseconds BIGINT NOT NULL DEFAULT 25 CHECK(oltp_p99_target_milliseconds BETWEEN 1 AND 60000);
ALTER TABLE historical_backfills ADD COLUMN retry_after TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp();
CREATE FUNCTION guard_backfill_execution() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE item JSONB; prefix TEXT;
BEGIN
 IF TG_OP='UPDATE' AND NEW.source_objects IS DISTINCT FROM OLD.source_objects THEN RAISE EXCEPTION 'Backfill source objects are immutable'; END IF;
 IF TG_OP='UPDATE' AND OLD.state IN ('BACKFILLING','VERIFYING','COMPLETE') AND NEW.expected_rows IS DISTINCT FROM OLD.expected_rows THEN RAISE EXCEPTION 'Validated row count is immutable'; END IF;
 IF NEW.state IN ('BACKFILLING','VERIFYING','COMPLETE') THEN
  IF NEW.expected_rows IS NULL OR NEW.rows_processed>NEW.expected_rows THEN RAISE EXCEPTION 'Backfill progress exceeds validated rows'; END IF;
  prefix:='tenants/'||NEW.tenant_id||'/backfills/'||NEW.id||'/';
  FOR item IN SELECT value FROM jsonb_array_elements(NEW.staging_objects) LOOP
   IF COALESCE(item->>'key','') NOT LIKE prefix||'%' OR strpos(item->>'key','..')>0 OR COALESCE(item->>'checksum_sha256','') !~ '^[0-9a-f]{64}$' THEN RAISE EXCEPTION 'Invalid backfill staging object'; END IF;
  END LOOP;
 END IF;
 IF NEW.state='COMPLETE' AND NEW.rows_processed<>NEW.expected_rows THEN RAISE EXCEPTION 'Backfill completion requires all rows'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER backfill_execution_guard BEFORE INSERT OR UPDATE ON historical_backfills FOR EACH ROW EXECUTE FUNCTION guard_backfill_execution();
