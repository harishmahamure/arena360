-- JetStream acknowledgements survive realtime lag and cell restarts.
CREATE TABLE outbox_publish_state (
  singleton INTEGER PRIMARY KEY CHECK (singleton=1),
  acknowledged_sequence INTEGER NOT NULL CHECK (acknowledged_sequence>=0)
) STRICT;
INSERT INTO outbox_publish_state VALUES(1,0);
