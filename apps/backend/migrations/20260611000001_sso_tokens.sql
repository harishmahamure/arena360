-- SSO tokens for Android TV staff provisioning (DRAFT-0035)

CREATE TABLE IF NOT EXISTS sso_tokens (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    token_hash TEXT NOT NULL UNIQUE,
    purpose TEXT NOT NULL,
    device_id UUID REFERENCES devices(id),
    created_by UUID REFERENCES users(id),
    expires_at TIMESTAMPTZ NOT NULL,
    redeemed_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_sso_tokens_device_id ON sso_tokens(device_id);
CREATE INDEX IF NOT EXISTS idx_sso_tokens_expires_at ON sso_tokens(expires_at);
