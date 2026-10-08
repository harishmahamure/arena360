-- Retirement is durable before object deletion. Rows remain for audit/ordering;
-- restore must exclude retired objects even if a stale remote index lists them.
ALTER TABLE snapshot_manifests ADD COLUMN retired_at TIMESTAMPTZ;
ALTER TABLE snapshot_manifests ADD COLUMN deleted_at TIMESTAMPTZ;
ALTER TABLE replication_segments ADD COLUMN retired_at TIMESTAMPTZ;
ALTER TABLE replication_segments ADD COLUMN deleted_at TIMESTAMPTZ;
ALTER TABLE snapshot_manifests ADD CHECK (retired_at IS NULL OR verified_at IS NOT NULL);
ALTER TABLE replication_segments ADD CHECK (retired_at IS NULL OR verified_at IS NOT NULL);
ALTER TABLE snapshot_manifests ADD CHECK (deleted_at IS NULL OR retired_at IS NOT NULL);
ALTER TABLE replication_segments ADD CHECK (deleted_at IS NULL OR retired_at IS NOT NULL);
