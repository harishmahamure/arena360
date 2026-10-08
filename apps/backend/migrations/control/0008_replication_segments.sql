-- Several WAL lineages can occur within one ownership lease (restore or gap).
ALTER TABLE replication_generations DROP CONSTRAINT replication_generations_tenant_id_ownership_generation_key;
CREATE TABLE replication_segments (
  generation_id UUID NOT NULL REFERENCES replication_generations(id) ON DELETE CASCADE,
  segment_number BIGINT NOT NULL CHECK (segment_number > 0),
  source_checksum TEXT NOT NULL CHECK (source_checksum ~ '^[0-9a-f]{64}$'),
  object_key TEXT NOT NULL UNIQUE,
  capture JSONB NOT NULL,
  checksum_sha256 TEXT CHECK (checksum_sha256 ~ '^[0-9a-f]{64}$'),
  encrypted_size_bytes BIGINT CHECK (encrypted_size_bytes > 0),
  verified_at TIMESTAMPTZ,
  PRIMARY KEY (generation_id, segment_number),
  UNIQUE (generation_id, source_checksum),
  CHECK (verified_at IS NULL OR (checksum_sha256 IS NOT NULL AND encrypted_size_bytes IS NOT NULL))
);
