ALTER TABLE snapshot_manifests ADD COLUMN capture_number BIGINT NOT NULL DEFAULT 0 CHECK (capture_number>=0);
ALTER TABLE snapshot_manifests ADD COLUMN event_sequence BIGINT NOT NULL DEFAULT 0 CHECK (event_sequence>=0);
ALTER TABLE snapshot_manifests ADD COLUMN source_checksum_sha256 TEXT CHECK (source_checksum_sha256 ~ '^[0-9a-f]{64}$');
ALTER TABLE replication_generations ADD COLUMN manifest_revision BIGINT NOT NULL DEFAULT 0 CHECK (manifest_revision>=0);
