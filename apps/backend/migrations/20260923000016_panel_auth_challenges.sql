CREATE TABLE panel_auth_challenges (
  "tokenHash" VARCHAR(64) PRIMARY KEY,
  "userId" UUID NOT NULL UNIQUE REFERENCES users(id) ON DELETE CASCADE,
  "expiresAt" TIMESTAMPTZ NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts >= 0),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_panel_auth_challenges_expiry
  ON panel_auth_challenges("expiresAt");
