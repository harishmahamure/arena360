CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE cells (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  name TEXT NOT NULL UNIQUE,
  address TEXT NOT NULL UNIQUE,
  state TEXT NOT NULL DEFAULT 'ACTIVE'
    CHECK (state IN ('ACTIVE', 'DRAINING', 'OFFLINE')),
  capacity_weights JSONB NOT NULL DEFAULT '{}'::jsonb
    CHECK (jsonb_typeof(capacity_weights) = 'object'),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE tenants (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  slug TEXT NOT NULL UNIQUE,
  name TEXT NOT NULL,
  owner_cell UUID REFERENCES cells(id),
  storage_engine TEXT NOT NULL DEFAULT 'SQLITE'
    CHECK (storage_engine = 'SQLITE'),
  ownership_generation BIGINT NOT NULL DEFAULT 0
    CHECK (ownership_generation >= 0),
  schema_version BIGINT NOT NULL DEFAULT 0 CHECK (schema_version >= 0),
  state TEXT NOT NULL DEFAULT 'PROVISIONING'
    CHECK (state IN (
      'PROVISIONING', 'ACTIVE', 'FENCED', 'PREPARING_MOVE', 'COPYING',
      'CUTOVER', 'VERIFYING', 'COLD', 'RESTORING', 'FAILED', 'DELETED'
    )),
  timezone TEXT NOT NULL CHECK (
    timezone = 'UTC'
    OR timezone ~ '^[A-Za-z]+(?:[._+-]?[A-Za-z0-9]+)*/[A-Za-z0-9._+-]+$'
  ),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CHECK (
    (owner_cell IS NULL AND state IN ('PROVISIONING', 'COLD', 'FAILED', 'DELETED'))
    OR owner_cell IS NOT NULL
  )
);

CREATE INDEX idx_tenants_owner_cell ON tenants(owner_cell) WHERE owner_cell IS NOT NULL;
CREATE INDEX idx_tenants_state ON tenants(state);

CREATE TABLE tenant_leases (
  tenant_id UUID PRIMARY KEY REFERENCES tenants(id) ON DELETE CASCADE,
  owner_cell UUID NOT NULL REFERENCES cells(id),
  ownership_generation BIGINT NOT NULL CHECK (ownership_generation > 0),
  expires_at TIMESTAMPTZ NOT NULL,
  renewed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CHECK (expires_at > renewed_at)
);

CREATE INDEX idx_tenant_leases_expiry ON tenant_leases(expires_at);

CREATE TABLE users (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  username TEXT NOT NULL UNIQUE,
  email TEXT UNIQUE,
  phone_number TEXT,
  password_hash TEXT NOT NULL,
  first_name TEXT,
  last_name TEXT,
  is_active BOOLEAN NOT NULL DEFAULT TRUE,
  totp_secret TEXT,
  totp_enabled BOOLEAN NOT NULL DEFAULT FALSE,
  avatar_url TEXT,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  deleted_at TIMESTAMPTZ,
  CHECK (NOT totp_enabled OR totp_secret IS NOT NULL)
);

CREATE TABLE organization_memberships (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  role TEXT NOT NULL CHECK (role IN ('admin', 'staff')),
  permissions JSONB NOT NULL DEFAULT '[]'::jsonb
    CHECK (jsonb_typeof(permissions) = 'array'),
  is_active BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (tenant_id, user_id)
);

CREATE INDEX idx_memberships_user ON organization_memberships(user_id);

CREATE TABLE auth_challenges (
  token_hash TEXT PRIMARY KEY,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  kind TEXT NOT NULL CHECK (kind IN ('PANEL_MFA', 'PASSWORD_RESET')),
  expires_at TIMESTAMPTZ NOT NULL,
  attempts INTEGER NOT NULL DEFAULT 0 CHECK (attempts BETWEEN 0 AND 5),
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (user_id, kind)
);

CREATE INDEX idx_auth_challenges_expiry ON auth_challenges(expires_at);

CREATE TABLE locations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  slug TEXT NOT NULL,
  name TEXT NOT NULL,
  is_active BOOLEAN NOT NULL DEFAULT TRUE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (tenant_id, slug),
  UNIQUE (id, tenant_id)
);

CREATE TABLE subscriptions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  plan_code TEXT NOT NULL,
  status TEXT NOT NULL
    CHECK (status IN ('TRIAL', 'ACTIVE', 'PAST_DUE', 'SUSPENDED', 'CANCELLED')),
  starts_at TIMESTAMPTZ NOT NULL,
  ends_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CHECK (ends_at IS NULL OR ends_at > starts_at)
);

CREATE UNIQUE INDEX idx_subscriptions_current
  ON subscriptions(tenant_id)
  WHERE status IN ('TRIAL', 'ACTIVE', 'PAST_DUE', 'SUSPENDED');

CREATE TABLE licenses (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'SUSPENDED', 'EXPIRED', 'REVOKED')),
  revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
  entitlements JSONB NOT NULL DEFAULT '{}'::jsonb
    CHECK (jsonb_typeof(entitlements) = 'object'),
  valid_from TIMESTAMPTZ NOT NULL,
  valid_until TIMESTAMPTZ NOT NULL,
  grace_until TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CHECK (valid_until > valid_from),
  CHECK (grace_until >= valid_until),
  UNIQUE (tenant_id, revision)
);

CREATE INDEX idx_licenses_tenant_status ON licenses(tenant_id, status);

CREATE TABLE replication_generations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  ownership_generation BIGINT NOT NULL CHECK (ownership_generation > 0),
  state TEXT NOT NULL DEFAULT 'ACTIVE'
    CHECK (state IN ('ACTIVE', 'SEALED', 'GAPPED', 'RESTORED', 'FAILED')),
  manifest_object_key TEXT NOT NULL,
  started_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  sealed_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (id, tenant_id),
  UNIQUE (tenant_id, ownership_generation),
  CHECK (sealed_at IS NULL OR sealed_at >= started_at)
);

ALTER TABLE tenants ADD COLUMN current_replication_generation UUID;
ALTER TABLE tenants ADD CONSTRAINT tenants_current_generation_fk
  FOREIGN KEY (current_replication_generation, id)
  REFERENCES replication_generations(id, tenant_id);

CREATE TABLE snapshot_manifests (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
  generation_id UUID NOT NULL,
  kind TEXT NOT NULL CHECK (kind IN ('DAILY', 'PRE_MIGRATION', 'POST_MIGRATION', 'MANUAL')),
  schema_version BIGINT NOT NULL CHECK (schema_version >= 0),
  object_key TEXT NOT NULL UNIQUE,
  checksum_sha256 TEXT NOT NULL CHECK (checksum_sha256 ~ '^[0-9a-f]{64}$'),
  encrypted_size_bytes BIGINT NOT NULL CHECK (encrypted_size_bytes > 0),
  snapshot_at TIMESTAMPTZ NOT NULL,
  verified_at TIMESTAMPTZ,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  FOREIGN KEY (generation_id, tenant_id)
    REFERENCES replication_generations(id, tenant_id) ON DELETE CASCADE,
  CHECK (verified_at IS NULL OR verified_at >= snapshot_at)
);

CREATE INDEX idx_snapshots_generation_time
  ON snapshot_manifests(generation_id, snapshot_at DESC);
