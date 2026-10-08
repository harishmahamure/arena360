-- Mark only after the retired month's entire derived prefix has been deleted.
ALTER TABLE archive_manifests ADD COLUMN hot_cleaned_at TIMESTAMPTZ;
