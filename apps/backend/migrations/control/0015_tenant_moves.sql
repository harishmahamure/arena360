CREATE TABLE tenant_moves (
  id UUID PRIMARY KEY,
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  source_cell UUID NOT NULL REFERENCES cells(id),
  target_cell UUID NOT NULL REFERENCES cells(id),
  source_ownership_generation BIGINT NOT NULL CHECK(source_ownership_generation>0),
  source_replication_generation UUID REFERENCES replication_generations(id),
  phase TEXT NOT NULL DEFAULT 'PREPARING_MOVE' CHECK(phase IN ('PREPARING_MOVE','COPYING','CUTOVER','VERIFYING','ACTIVE','CANCELLED')),
  target_capture_number BIGINT CHECK(target_capture_number>=0),
  final_capture_number BIGINT CHECK(final_capture_number>=0),
  target_image_checksum TEXT CHECK(target_image_checksum ~ '^[0-9a-f]{64}$'),
  target_prepared_at TIMESTAMPTZ,
  cutover_started_at TIMESTAMPTZ,
  handed_off_at TIMESTAMPTZ,
  completed_at TIMESTAMPTZ,
  write_gate_milliseconds BIGINT CHECK(write_gate_milliseconds>=0),
  retain_source_until TIMESTAMPTZ,
  last_error TEXT,
  retry_after TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  CHECK(source_cell<>target_cell),
  CHECK((phase NOT IN ('VERIFYING','ACTIVE')) OR (handed_off_at IS NOT NULL AND source_replication_generation IS NOT NULL AND target_prepared_at IS NOT NULL AND final_capture_number IS NOT NULL AND target_capture_number IS NOT NULL AND final_capture_number=target_capture_number AND target_image_checksum IS NOT NULL)),
  CHECK((phase<>'ACTIVE') OR completed_at IS NOT NULL)
);
CREATE UNIQUE INDEX tenant_moves_one_pending ON tenant_moves(tenant_id) WHERE phase NOT IN ('ACTIVE','CANCELLED');
CREATE INDEX tenant_moves_source_queue ON tenant_moves(source_cell,retry_after) WHERE phase IN ('PREPARING_MOVE','COPYING','CUTOVER');
CREATE INDEX tenant_moves_target_queue ON tenant_moves(target_cell,retry_after) WHERE phase IN ('COPYING','CUTOVER','VERIFYING');
