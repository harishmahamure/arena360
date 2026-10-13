-- Setup belongs to the tenant file and follows its backup, move, and recovery lifecycle.
CREATE TABLE tenant_onboarding (
  singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
  current_step TEXT NOT NULL DEFAULT 'venue' CHECK (current_step IN ('venue','settings','stations','review')),
  revision INTEGER NOT NULL DEFAULT 0 CHECK (revision >= 0),
  started_at TEXT CHECK (started_at IS NULL OR (length(started_at)=27 AND substr(started_at,-1)='Z')),
  settings_reviewed_at TEXT CHECK (settings_reviewed_at IS NULL OR (length(settings_reviewed_at)=27 AND substr(settings_reviewed_at,-1)='Z')),
  completed_at TEXT CHECK (completed_at IS NULL OR (length(completed_at)=27 AND substr(completed_at,-1)='Z')),
  updated_at TEXT CHECK (updated_at IS NULL OR (length(updated_at)=27 AND substr(updated_at,-1)='Z')),
  updated_by TEXT REFERENCES users(id),
  CHECK (completed_at IS NULL OR (started_at IS NOT NULL AND current_step='review'))
) STRICT;

-- Venues already running operations must keep their normal landing page on upgrade.
INSERT INTO tenant_onboarding(singleton,current_step,started_at,settings_reviewed_at,completed_at,updated_at)
SELECT 1, CASE WHEN operating THEN 'review' ELSE 'venue' END,
  CASE WHEN operating THEN at END, CASE WHEN operating THEN at END,
  CASE WHEN operating THEN at END, CASE WHEN operating THEN at END
FROM (SELECT EXISTS(SELECT 1 FROM shifts UNION ALL SELECT 1 FROM transactions UNION ALL SELECT 1 FROM usage_sessions) AS operating,
  strftime('%Y-%m-%dT%H:%M:%f','now') || '000Z' AS at);
