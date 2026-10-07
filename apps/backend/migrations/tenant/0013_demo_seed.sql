-- Operational seed coordination stays with the tenant file and survives snapshots.
CREATE TABLE demo_seed_runs (
  version TEXT PRIMARY KEY,
  state TEXT NOT NULL CHECK(state IN('RUNNING','COMPLETE')),
  requested_date TEXT NOT NULL,
  summary TEXT CHECK(summary IS NULL OR json_valid(summary)),
  started_at TEXT NOT NULL,
  completed_at TEXT
) STRICT;
