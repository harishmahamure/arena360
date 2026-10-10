ALTER TABLE cells ADD COLUMN hydration_heartbeat_at TIMESTAMPTZ;
CREATE TABLE tenant_cold_jobs (
 id UUID PRIMARY KEY,
 tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
 source_cell UUID NOT NULL REFERENCES cells(id),
 source_ownership_generation BIGINT NOT NULL CHECK(source_ownership_generation>0),
 minimum_idle_seconds BIGINT NOT NULL CHECK(minimum_idle_seconds BETWEEN 0 AND 31536000),
 phase TEXT NOT NULL DEFAULT 'SNAPSHOTTING' CHECK(phase IN ('SNAPSHOTTING','RELEASED','HYDRATING','ACTIVE','CANCELLED')),
 source_generation UUID REFERENCES replication_generations(id),
 snapshot_id UUID REFERENCES snapshot_manifests(id),
 capture_number BIGINT CHECK(capture_number>=0),
 released_at TIMESTAMPTZ,
 source_cleaned_at TIMESTAMPTZ,
 target_cell UUID REFERENCES cells(id),
 target_ownership_generation BIGINT,
 hydration_requested_at TIMESTAMPTZ,
 operations_ready_at TIMESTAMPTZ,
 hydration_milliseconds BIGINT CHECK(hydration_milliseconds>=0),
 last_error TEXT,
 retry_after TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 CHECK(phase IN ('SNAPSHOTTING','CANCELLED') OR (source_generation IS NOT NULL AND snapshot_id IS NOT NULL AND capture_number IS NOT NULL AND released_at IS NOT NULL)),
 CHECK(phase NOT IN ('HYDRATING','ACTIVE') OR (source_cleaned_at IS NOT NULL AND target_cell IS NOT NULL AND target_ownership_generation>source_ownership_generation AND hydration_requested_at IS NOT NULL)),
 CHECK(phase<>'ACTIVE' OR operations_ready_at IS NOT NULL)
);
CREATE UNIQUE INDEX tenant_cold_one_pending ON tenant_cold_jobs(tenant_id) WHERE phase NOT IN ('ACTIVE','CANCELLED');
CREATE INDEX tenant_cold_source_queue ON tenant_cold_jobs(source_cell,retry_after) WHERE phase='SNAPSHOTTING' OR source_cleaned_at IS NULL;
