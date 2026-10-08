ALTER TABLE replication_generations ADD COLUMN start_reason TEXT NOT NULL DEFAULT 'LEASE_CHANGE'
  CHECK (start_reason IN ('LEASE_CHANGE','RESTORE','WAL_GAP'));
ALTER TABLE replication_generations ADD COLUMN transition_id UUID;
CREATE UNIQUE INDEX replication_generation_transition
  ON replication_generations(tenant_id,transition_id) WHERE transition_id IS NOT NULL;
