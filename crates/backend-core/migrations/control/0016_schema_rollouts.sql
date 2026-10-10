CREATE TABLE schema_rollouts (
 id UUID PRIMARY KEY,
 target_version BIGINT NOT NULL UNIQUE CHECK(target_version>0),
 phase INTEGER NOT NULL DEFAULT 0 CHECK(phase BETWEEN 0 AND 4),
 state TEXT NOT NULL DEFAULT 'RUNNING' CHECK(state IN ('RUNNING','HALTED','COMPLETE')),
 canary_count BIGINT NOT NULL CHECK(canary_count>0),
 tenant_count BIGINT NOT NULL CHECK(tenant_count>=canary_count),
 soak_seconds INTEGER NOT NULL CHECK(soak_seconds BETWEEN 0 AND 86400),
 stage_ready_at TIMESTAMPTZ,
 last_error TEXT,
 created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
 updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE schema_rollout_tenants (
 rollout_id UUID NOT NULL REFERENCES schema_rollouts(id) ON DELETE CASCADE,
 tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
 ordinal BIGINT NOT NULL CHECK(ordinal>0),
 state TEXT NOT NULL DEFAULT 'QUEUED' CHECK(state IN ('QUEUED','RUNNING','SUCCEEDED','FAILED')),
 attempts INTEGER NOT NULL DEFAULT 0 CHECK(attempts>=0),
 last_error TEXT,
 completed_at TIMESTAMPTZ,
 PRIMARY KEY(rollout_id,tenant_id),
 UNIQUE(rollout_id,ordinal)
);
CREATE INDEX schema_rollout_pending ON schema_rollout_tenants(rollout_id,ordinal) WHERE state IN ('QUEUED','RUNNING');
