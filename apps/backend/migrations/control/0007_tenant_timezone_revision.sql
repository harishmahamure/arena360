-- Every timezone change, including direct administration, has an ordered revision.
ALTER TABLE tenants ADD COLUMN timezone_revision BIGINT NOT NULL DEFAULT 0 CHECK(timezone_revision>=0);
CREATE FUNCTION stamp_tenant_timezone_revision() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    NEW.timezone_revision := OLD.timezone_revision;
    IF OLD.timezone IS DISTINCT FROM NEW.timezone THEN
        NEW.timezone_revision := OLD.timezone_revision + 1;
        NEW.updated_at := clock_timestamp();
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER tenant_timezone_revision BEFORE UPDATE ON tenants
FOR EACH ROW EXECUTE FUNCTION stamp_tenant_timezone_revision();
