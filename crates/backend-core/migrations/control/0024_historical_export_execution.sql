CREATE TABLE historical_export_limits (
 singleton BOOLEAN PRIMARY KEY DEFAULT true CHECK(singleton),
 platform_slots INTEGER NOT NULL DEFAULT 4 CHECK(platform_slots BETWEEN 1 AND 64),
 tenant_slots INTEGER NOT NULL DEFAULT 1 CHECK(tenant_slots BETWEEN 1 AND 16),
 cell_slots INTEGER NOT NULL DEFAULT 2 CHECK(cell_slots BETWEEN 1 AND 16)
);
INSERT INTO historical_export_limits(singleton) VALUES(true);
ALTER TABLE historical_exports ADD COLUMN result_object JSONB;
ALTER TABLE historical_exports ADD COLUMN retry_after TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp();
CREATE FUNCTION guard_export_result_object() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='UPDATE' AND OLD.state IN ('READY','EXPIRED') AND NEW.result_object IS DISTINCT FROM OLD.result_object THEN
   RAISE EXCEPTION 'Published export object evidence is immutable';
 END IF;
 IF NEW.result_object IS NOT NULL AND NEW.result_object->>'key' IS DISTINCT FROM NEW.result_object_key THEN
   RAISE EXCEPTION 'Export object evidence does not match its download key';
 END IF;
 RETURN NEW;
END;
$$;
CREATE TRIGGER export_result_object_guard BEFORE INSERT OR UPDATE ON historical_exports FOR EACH ROW EXECUTE FUNCTION guard_export_result_object();
