-- Keep the canonical export projection alongside lossless operational rows.
-- Projection/dimension rows are never counted as purgeable operational facts.
ALTER TABLE archive_manifests ADD COLUMN analytics_objects JSONB NOT NULL DEFAULT '[]' CHECK(jsonb_typeof(analytics_objects)='array');
ALTER TABLE archive_manifests ADD COLUMN analytics_checksum_sha256 TEXT CHECK(analytics_checksum_sha256 ~ '^[0-9a-f]{64}$');
ALTER TABLE archive_manifests ADD COLUMN analytics_projection_version BIGINT;
ALTER TABLE archive_manifests ADD CONSTRAINT archive_projection_evidence CHECK(jsonb_array_length(analytics_objects)=0 OR (analytics_checksum_sha256 IS NOT NULL AND analytics_projection_version IS NOT NULL AND analytics_projection_version>0));
CREATE FUNCTION guard_archive_projection() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE item JSONB; prefix TEXT;
BEGIN
 IF TG_OP='UPDATE' AND OLD.state IN ('UPLOADED','VERIFIED','PURGING','COMPLETE') AND
   (NEW.analytics_objects IS DISTINCT FROM OLD.analytics_objects OR NEW.analytics_checksum_sha256 IS DISTINCT FROM OLD.analytics_checksum_sha256 OR NEW.analytics_projection_version IS DISTINCT FROM OLD.analytics_projection_version) THEN
   RAISE EXCEPTION 'Archive analytics evidence is immutable; create another revision';
 END IF;
 prefix:='tenants/'||NEW.tenant_id||'/archive/'||to_char(NEW.period_start,'YYYY/MM')||'/';
 FOR item IN SELECT value FROM jsonb_array_elements(NEW.analytics_objects) LOOP
   IF left(item->>'key',length(prefix)) IS DISTINCT FROM prefix OR NOT COALESCE(item->>'checksum_sha256' ~ '^[0-9a-f]{64}$',false) OR NOT COALESCE((item->>'rows')::bigint>=0,false) THEN
     RAISE EXCEPTION 'Invalid archive analytics object';
   END IF;
 END LOOP;
 RETURN NEW;
END;
$$;
CREATE TRIGGER archive_projection_guard BEFORE INSERT OR UPDATE ON archive_manifests FOR EACH ROW EXECUTE FUNCTION guard_archive_projection();
