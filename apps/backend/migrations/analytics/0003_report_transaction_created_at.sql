-- Dashboard periods use creation time; business sales use transaction time.
-- Rebuild fills this from authoritative SQLite, rather than guessing one from the other.
ALTER TABLE transactions ADD COLUMN created_at TIMESTAMP;
