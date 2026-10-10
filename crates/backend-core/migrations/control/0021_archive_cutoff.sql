-- Freeze interval eligibility so old starts with recent ends remain available to hot reports.
ALTER TABLE archive_manifests ADD COLUMN source_cutoff_at TIMESTAMPTZ;
CREATE FUNCTION guard_archive_cutoff() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.source_cutoff_at IS DISTINCT FROM OLD.source_cutoff_at THEN
    RAISE EXCEPTION 'Archive source cutoff is immutable; replan instead';
  END IF;
  RETURN NEW;
END;
$$;
CREATE TRIGGER archive_cutoff_immutable BEFORE UPDATE ON archive_manifests
FOR EACH ROW EXECUTE FUNCTION guard_archive_cutoff();
