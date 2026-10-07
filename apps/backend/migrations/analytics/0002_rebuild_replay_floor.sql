-- Global broker position recorded before the SQLite snapshot. After a restore,
-- retained deliveries below it can be newer than authoritative restored SQLite.
ALTER TABLE _ingest_state ADD COLUMN replay_start_sequence UBIGINT;
