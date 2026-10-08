-- Restore-only drills never transfer ownership or install an operational image.
CREATE TABLE cell_restore_drills (
  id UUID PRIMARY KEY,
  cell_id UUID NOT NULL REFERENCES cells(id),
  started_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
  finished_at TIMESTAMPTZ,
  status TEXT NOT NULL DEFAULT 'RUNNING' CHECK (status IN ('RUNNING','PASSED','FAILED')),
  elapsed_milliseconds BIGINT CHECK (elapsed_milliseconds >= 0),
  tenant_count INTEGER NOT NULL CHECK (tenant_count >= 0),
  results JSONB NOT NULL DEFAULT '[]' CHECK (jsonb_typeof(results) = 'array'),
  CHECK ((status = 'RUNNING') = (finished_at IS NULL))
);
CREATE INDEX cell_restore_drills_history ON cell_restore_drills(cell_id,started_at DESC);
