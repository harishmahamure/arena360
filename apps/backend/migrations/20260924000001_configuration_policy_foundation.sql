-- Arena360 tenancy-aware configuration and governed pricing policy foundation.
-- Additive by design: the legacy configurations table remains available during rollout.

CREATE TABLE organizations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  slug VARCHAR(80) NOT NULL UNIQUE,
  name VARCHAR(160) NOT NULL,
  "isActive" BOOLEAN NOT NULL DEFAULT TRUE,
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE TABLE venue_locations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "organizationId" UUID NOT NULL REFERENCES organizations(id),
  slug VARCHAR(80) NOT NULL,
  name VARCHAR(160) NOT NULL,
  timezone VARCHAR(80) NOT NULL DEFAULT 'Asia/Kolkata',
  currency CHAR(3) NOT NULL DEFAULT 'INR',
  "isActive" BOOLEAN NOT NULL DEFAULT TRUE,
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE ("organizationId", slug),
  UNIQUE (id, "organizationId")
);

CREATE TABLE organization_memberships (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "organizationId" UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  "userId" UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  role VARCHAR(40) NOT NULL,
  permissions JSONB NOT NULL DEFAULT '[]'::jsonb,
  "isActive" BOOLEAN NOT NULL DEFAULT TRUE,
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE ("organizationId", "userId"),
  UNIQUE (id, "organizationId")
);

CREATE TABLE location_access_assignments (
  "organizationId" UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  "membershipId" UUID NOT NULL,
  "locationId" UUID NOT NULL,
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  PRIMARY KEY ("membershipId", "locationId"),
  CONSTRAINT location_access_membership_scope_fk
    FOREIGN KEY ("membershipId", "organizationId")
    REFERENCES organization_memberships(id, "organizationId") ON DELETE CASCADE,
  CONSTRAINT location_access_location_scope_fk
    FOREIGN KEY ("locationId", "organizationId")
    REFERENCES venue_locations(id, "organizationId") ON DELETE CASCADE
);

CREATE TABLE setting_overrides (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "organizationId" UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  "locationId" UUID REFERENCES venue_locations(id) ON DELETE CASCADE,
  key VARCHAR(120) NOT NULL,
  value JSONB NOT NULL,
  revision BIGINT NOT NULL DEFAULT 1 CHECK (revision > 0),
  "createdBy" UUID REFERENCES users(id),
  "updatedBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT setting_override_location_scope_fk
    FOREIGN KEY ("locationId", "organizationId")
    REFERENCES venue_locations(id, "organizationId")
);

CREATE UNIQUE INDEX setting_overrides_scope_key_unique
  ON setting_overrides (
    "organizationId",
    COALESCE("locationId", '00000000-0000-0000-0000-000000000000'::uuid),
    key
  );
CREATE INDEX setting_overrides_resolve_idx
  ON setting_overrides ("organizationId", "locationId", key);

CREATE TABLE setting_revisions (
  id BIGSERIAL PRIMARY KEY,
  "organizationId" UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  "locationId" UUID REFERENCES venue_locations(id) ON DELETE CASCADE,
  key VARCHAR(120) NOT NULL,
  revision BIGINT NOT NULL,
  operation VARCHAR(20) NOT NULL CHECK (operation IN ('create', 'update', 'delete')),
  "oldValue" JSONB,
  "newValue" JSONB,
  reason TEXT NOT NULL,
  "actorUserId" UUID REFERENCES users(id),
  "requestId" VARCHAR(128),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX setting_revisions_history_idx
  ON setting_revisions ("organizationId", "locationId", key, revision DESC);

CREATE TYPE pricing_rule_version_status AS ENUM (
  'draft', 'validated', 'scheduled', 'published', 'superseded'
);

CREATE TABLE pricing_rule_sets (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "organizationId" UUID NOT NULL REFERENCES organizations(id) ON DELETE CASCADE,
  "locationId" UUID REFERENCES venue_locations(id) ON DELETE CASCADE,
  name VARCHAR(160) NOT NULL,
  description TEXT,
  "activeVersionId" UUID,
  "createdBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  CONSTRAINT pricing_rule_set_location_scope_fk
    FOREIGN KEY ("locationId", "organizationId")
    REFERENCES venue_locations(id, "organizationId")
);
CREATE INDEX pricing_rule_sets_scope_idx
  ON pricing_rule_sets ("organizationId", "locationId");

CREATE TABLE pricing_rule_versions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "ruleSetId" UUID NOT NULL REFERENCES pricing_rule_sets(id) ON DELETE CASCADE,
  version INTEGER NOT NULL CHECK (version > 0),
  status pricing_rule_version_status NOT NULL DEFAULT 'draft',
  policy JSONB NOT NULL,
  "simulationHash" VARCHAR(64),
  "validatedAt" TIMESTAMPTZ,
  "effectiveAt" TIMESTAMPTZ,
  "publishedAt" TIMESTAMPTZ,
  "createdBy" UUID REFERENCES users(id),
  "publishedBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE ("ruleSetId", version)
);

ALTER TABLE pricing_rule_sets
  ADD CONSTRAINT pricing_rule_sets_active_version_fk
  FOREIGN KEY ("activeVersionId") REFERENCES pricing_rule_versions(id);

-- Stable bootstrap identities let old single-venue installations migrate deterministically.
INSERT INTO organizations (id, slug, name)
VALUES ('00000000-0000-4000-8000-000000000001', 'default', 'Arena360 Organization')
ON CONFLICT (id) DO NOTHING;

INSERT INTO venue_locations (id, "organizationId", slug, name, timezone, currency)
VALUES (
  '00000000-0000-4000-8000-000000000002',
  '00000000-0000-4000-8000-000000000001',
  'default',
  'Main Venue',
  COALESCE(NULLIF(current_setting('arena360.bootstrap_timezone', true), ''), 'Asia/Kolkata'),
  'INR'
)
ON CONFLICT (id) DO NOTHING;

INSERT INTO organization_memberships ("organizationId", "userId", role, permissions)
SELECT
  '00000000-0000-4000-8000-000000000001',
  u.id,
  COALESCE(u.role, 'player'),
  CASE COALESCE(u.role, 'player')
    WHEN 'admin' THEN '["config:read","config:write","settings:read","settings:write","rules:read","rules:edit","rules:publish"]'::jsonb
    WHEN 'staff' THEN '["config:read","settings:read","rules:read"]'::jsonb
    ELSE '[]'::jsonb
  END
FROM users u
ON CONFLICT ("organizationId", "userId") DO NOTHING;

INSERT INTO location_access_assignments ("organizationId", "membershipId", "locationId")
SELECT m."organizationId", m.id, '00000000-0000-4000-8000-000000000002'
FROM organization_memberships m
WHERE m."organizationId" = '00000000-0000-4000-8000-000000000001'
ON CONFLICT DO NOTHING;

-- Keep the compatibility tenant usable for users created during the staged rollout.
-- Multi-organization provisioning can replace this assignment once its invitation flow lands.
CREATE OR REPLACE FUNCTION arena360_assign_default_membership()
RETURNS TRIGGER AS $$
DECLARE
  membership_id UUID;
  membership_permissions JSONB;
BEGIN
  membership_permissions := CASE COALESCE(NEW.role, 'player')
    WHEN 'admin' THEN '["config:read","config:write","settings:read","settings:write","rules:read","rules:edit","rules:publish"]'::jsonb
    WHEN 'staff' THEN '["config:read","settings:read","rules:read"]'::jsonb
    ELSE '[]'::jsonb
  END;

  INSERT INTO organization_memberships (
    "organizationId", "userId", role, permissions, "updatedAt"
  )
  VALUES (
    '00000000-0000-4000-8000-000000000001',
    NEW.id,
    COALESCE(NEW.role, 'player'),
    membership_permissions,
    NOW()
  )
  ON CONFLICT ("organizationId", "userId") DO UPDATE SET
    role = EXCLUDED.role,
    permissions = EXCLUDED.permissions,
    "updatedAt" = NOW()
  RETURNING id INTO membership_id;

  INSERT INTO location_access_assignments ("organizationId", "membershipId", "locationId")
  VALUES (
    '00000000-0000-4000-8000-000000000001',
    membership_id,
    '00000000-0000-4000-8000-000000000002'
  )
  ON CONFLICT DO NOTHING;

  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER users_default_membership_trigger
AFTER INSERT OR UPDATE OF role ON users
FOR EACH ROW EXECUTE FUNCTION arena360_assign_default_membership();

-- Refuse to shadow-resolve malformed supported values. Unknown legacy keys stay
-- in the compatibility table but are intentionally excluded from the catalog.
DO $$
BEGIN
  IF EXISTS (
    SELECT 1 FROM configurations c
    WHERE c.key IN (
      'business.name', 'business.address', 'business.phone', 'business.email',
      'business.gst_number', 'business.logo_url', 'venue.timezone',
      'pricing.currency', 'pricing.night_window_start', 'pricing.night_window_end',
      'receipt.header_text', 'receipt.footer_text'
    ) AND jsonb_typeof(c.value) <> 'string'
  ) OR EXISTS (
    SELECT 1 FROM configurations c
    WHERE c.key IN ('pricing.tax_rate', 'pricing.default_per_minute_rate')
      AND jsonb_typeof(c.value) <> 'number'
  ) OR EXISTS (
    SELECT 1 FROM configurations c
    WHERE c.key = 'receipt.show_gst' AND jsonb_typeof(c.value) <> 'boolean'
  ) OR EXISTS (
    SELECT 1 FROM configurations c
    WHERE c.key IN (
      'plans.default_validity_days', 'plans.default_time_credits',
      'staff.allowance_period_days', 'sessions.ending_warning_minutes',
      'sessions.offline_grace_minutes', 'notifications.retention_days'
    ) AND (
      jsonb_typeof(c.value) <> 'number'
      OR trunc((c.value #>> '{}')::numeric) <> (c.value #>> '{}')::numeric
    )
  ) OR EXISTS (
    SELECT 1 FROM configurations c
    WHERE c.key IN (
      'inventory.default_warehouse_id', 'inventory.default_store_id',
      'pos.default_sale_location_id'
    ) AND jsonb_typeof(c.value) NOT IN ('string', 'null')
  ) THEN
    RAISE EXCEPTION 'Supported legacy configuration contains an invalid value type';
  END IF;
END $$;

INSERT INTO setting_overrides (
  "organizationId", key, value, revision, "createdBy", "updatedBy", "createdAt", "updatedAt"
)
SELECT
  '00000000-0000-4000-8000-000000000001',
  c.key,
  c.value,
  1,
  c."createdBy",
  c."updatedBy",
  c."createdAt",
  c."updatedAt"
FROM configurations c
WHERE c.key IN (
  'business.name', 'business.address', 'business.phone', 'business.email',
  'business.gst_number', 'business.logo_url', 'venue.timezone',
  'pricing.currency', 'pricing.tax_rate', 'pricing.default_per_minute_rate',
  'pricing.night_window_start', 'pricing.night_window_end',
  'receipt.header_text', 'receipt.footer_text', 'receipt.show_gst',
  'plans.default_validity_days', 'plans.default_time_credits',
  'staff.allowance_period_days', 'sessions.ending_warning_minutes',
  'sessions.offline_grace_minutes', 'notifications.retention_days',
  'inventory.default_warehouse_id', 'inventory.default_store_id',
  'pos.default_sale_location_id'
)
ON CONFLICT DO NOTHING;

INSERT INTO setting_revisions (
  "organizationId", key, revision, operation, "newValue", reason, "actorUserId", "createdAt"
)
SELECT
  '00000000-0000-4000-8000-000000000001',
  c.key,
  1,
  'create',
  c.value,
  'Migrated from legacy configurations',
  c."updatedBy",
  c."updatedAt"
FROM configurations c
WHERE c.key IN (
  'business.name', 'business.address', 'business.phone', 'business.email',
  'business.gst_number', 'business.logo_url', 'venue.timezone',
  'pricing.currency', 'pricing.tax_rate', 'pricing.default_per_minute_rate',
  'pricing.night_window_start', 'pricing.night_window_end',
  'receipt.header_text', 'receipt.footer_text', 'receipt.show_gst',
  'plans.default_validity_days', 'plans.default_time_credits',
  'staff.allowance_period_days', 'sessions.ending_warning_minutes',
  'sessions.offline_grace_minutes', 'notifications.retention_days',
  'inventory.default_warehouse_id', 'inventory.default_store_id',
  'pos.default_sale_location_id'
);
