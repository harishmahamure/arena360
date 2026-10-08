CREATE TABLE tenant_recovery_jobs (
  id UUID PRIMARY KEY,
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  cell_id UUID NOT NULL REFERENCES cells(id),
  ownership_generation BIGINT NOT NULL CHECK (ownership_generation > 0),
  source_generation UUID NOT NULL REFERENCES replication_generations(id),
  phase TEXT NOT NULL DEFAULT 'DOWNLOADING' CHECK (phase IN ('DOWNLOADING','INSTALLED','BASELINED','COMPLETE')),
  capture_number BIGINT CHECK (capture_number >= 0),
  recovered_at TIMESTAMPTZ,
  image_checksum TEXT CHECK (image_checksum ~ '^[0-9a-f]{64}$'),
  last_error TEXT,
  analytics_ready_at TIMESTAMPTZ,
  started_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  operations_ready_at TIMESTAMPTZ,
  UNIQUE (tenant_id,ownership_generation)
);
