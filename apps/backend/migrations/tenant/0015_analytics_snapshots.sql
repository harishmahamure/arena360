-- Full, secret-free analytics projection captured in the business transaction.
-- NULL on pre-upgrade events requires a consistent snapshot rebuild.
ALTER TABLE outbox_events ADD COLUMN analytics_snapshot TEXT
  CHECK (analytics_snapshot IS NULL OR (json_valid(analytics_snapshot) AND json_type(analytics_snapshot)='object'));
