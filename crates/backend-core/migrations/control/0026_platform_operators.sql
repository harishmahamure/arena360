CREATE TABLE platform_operators (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  username TEXT NOT NULL UNIQUE CHECK (username ~ '^[A-Za-z0-9][A-Za-z0-9._-]{0,99}$'),
  password_hash TEXT NOT NULL,
  is_active BOOLEAN NOT NULL DEFAULT TRUE,
  totp_secret TEXT,
  totp_pending_secret TEXT,
  totp_enabled BOOLEAN NOT NULL DEFAULT FALSE,
  failed_logins INTEGER NOT NULL DEFAULT 0,
  locked_until TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CHECK (NOT totp_enabled OR totp_secret IS NOT NULL)
);
CREATE UNIQUE INDEX platform_operators_username_ci ON platform_operators(lower(username));

CREATE TABLE platform_auth_tokens (
  token_hash TEXT PRIMARY KEY,
  operator_id UUID NOT NULL REFERENCES platform_operators(id) ON DELETE CASCADE,
  purpose TEXT NOT NULL CHECK (purpose IN ('CHALLENGE', 'SESSION')),
  expires_at TIMESTAMPTZ NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX platform_auth_tokens_operator ON platform_auth_tokens(operator_id);

ALTER TABLE tenants ADD COLUMN is_enabled BOOLEAN NOT NULL DEFAULT TRUE;

CREATE TABLE platform_plans (
  code TEXT PRIMARY KEY,
  name TEXT NOT NULL,
  entitlements JSONB NOT NULL DEFAULT '{}'::jsonb CHECK (jsonb_typeof(entitlements)='object'),
  grace_days INTEGER NOT NULL DEFAULT 7 CHECK (grace_days BETWEEN 0 AND 30),
  is_active BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
INSERT INTO platform_plans(code,name,entitlements) VALUES('trial','Trial','{}');
