CREATE TABLE outbox_events (
  sequence INTEGER PRIMARY KEY AUTOINCREMENT,
  event_id TEXT NOT NULL UNIQUE
    CHECK (
      length(event_id) = 36
      AND substr(event_id, 9, 1) = '-'
      AND substr(event_id, 14, 1) = '-'
      AND substr(event_id, 19, 1) = '-'
      AND substr(event_id, 24, 1) = '-'
      AND length(event_id) - length(replace(event_id, '-', '')) = 4
      AND event_id = lower(event_id)
      AND event_id NOT GLOB '*[^0-9a-f-]*'
    ),
  location_id TEXT
    CHECK (
      location_id IS NULL
      OR (
        length(location_id) = 36
        AND substr(location_id, 9, 1) = '-'
        AND substr(location_id, 14, 1) = '-'
        AND substr(location_id, 19, 1) = '-'
        AND substr(location_id, 24, 1) = '-'
        AND length(location_id) - length(replace(location_id, '-', '')) = 4
        AND location_id = lower(location_id)
        AND location_id NOT GLOB '*[^0-9a-f-]*'
      )
    ),
  aggregate_type TEXT NOT NULL
    CHECK (length(trim(aggregate_type)) > 0),
  aggregate_id TEXT NOT NULL
    CHECK (
      length(aggregate_id) = 36
      AND substr(aggregate_id, 9, 1) = '-'
      AND substr(aggregate_id, 14, 1) = '-'
      AND substr(aggregate_id, 19, 1) = '-'
      AND substr(aggregate_id, 24, 1) = '-'
      AND length(aggregate_id) - length(replace(aggregate_id, '-', '')) = 4
      AND aggregate_id = lower(aggregate_id)
      AND aggregate_id NOT GLOB '*[^0-9a-f-]*'
    ),
  event_type TEXT NOT NULL
    CHECK (length(trim(event_type)) > 0),
  occurred_at TEXT NOT NULL
    CHECK (
      length(occurred_at) = 27
      AND occurred_at GLOB
        '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]T[0-9][0-9]:[0-9][0-9]:[0-9][0-9].[0-9][0-9][0-9][0-9][0-9][0-9]Z'
    ),
  schema_version INTEGER NOT NULL
    CHECK (schema_version > 0),
  deleted INTEGER NOT NULL DEFAULT 0
    CHECK (deleted IN (0, 1)),
  payload TEXT NOT NULL
    CHECK (json_valid(payload) = 1 AND json_type(payload) = 'object')
) STRICT;
