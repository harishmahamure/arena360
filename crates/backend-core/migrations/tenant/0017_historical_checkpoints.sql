-- Replicated together with the bounded purge/import transactions themselves.
CREATE TABLE archive_purge_checkpoints (
 archive_id TEXT PRIMARY KEY,
 manifest_checksum TEXT NOT NULL CHECK(length(manifest_checksum)=64),
 checkpoint TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(checkpoint) AND json_type(checkpoint)='object'),
 rows_deleted INTEGER NOT NULL DEFAULT 0 CHECK(rows_deleted>=0),
 completed INTEGER NOT NULL DEFAULT 0 CHECK(completed IN (0,1)),
 updated_at TEXT NOT NULL
) STRICT;
CREATE TABLE archive_backfill_checkpoints (
 backfill_id TEXT PRIMARY KEY,
 validation_checksum TEXT NOT NULL CHECK(length(validation_checksum)=64),
 checkpoint TEXT NOT NULL DEFAULT '{}' CHECK(json_valid(checkpoint) AND json_type(checkpoint)='object'),
 rows_processed INTEGER NOT NULL DEFAULT 0 CHECK(rows_processed>=0),
 completed INTEGER NOT NULL DEFAULT 0 CHECK(completed IN (0,1)),
 updated_at TEXT NOT NULL
) STRICT;
