CREATE TABLE access_roles (
 id uuid PRIMARY KEY DEFAULT gen_random_uuid(), organization_id uuid NOT NULL REFERENCES organizations(id),
 name text NOT NULL CHECK(length(name) BETWEEN 1 AND 80), description text NOT NULL DEFAULT '',
 permissions jsonb NOT NULL CHECK(jsonb_typeof(permissions)='array'), is_template boolean NOT NULL DEFAULT false,
 system_key text, revision integer NOT NULL DEFAULT 1, updated_at timestamptz NOT NULL DEFAULT now(),
 UNIQUE(organization_id,id), UNIQUE(organization_id,system_key)
);
CREATE UNIQUE INDEX access_role_name ON access_roles(organization_id,lower(name),is_template);
CREATE TABLE access_assignments (
 organization_id uuid NOT NULL, user_id uuid NOT NULL REFERENCES users(id), role_id uuid NOT NULL,
 PRIMARY KEY(organization_id,user_id,role_id),
 FOREIGN KEY(organization_id,role_id) REFERENCES access_roles(organization_id,id) ON DELETE CASCADE
);
CREATE TABLE access_modules (
 organization_id uuid NOT NULL REFERENCES organizations(id), module text NOT NULL,
 enabled boolean NOT NULL DEFAULT true, revision integer NOT NULL DEFAULT 1,
 PRIMARY KEY(organization_id,module), CHECK(module <> 'access' OR enabled)
);
CREATE TABLE access_audit (
 id bigserial PRIMARY KEY, organization_id uuid NOT NULL REFERENCES organizations(id), actor_id uuid REFERENCES users(id),
 action text NOT NULL, target_id text NOT NULL, before_value jsonb, after_value jsonb,
 created_at timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX access_audit_org ON access_audit(organization_id,id DESC);
CREATE TABLE access_member_versions (
 organization_id uuid NOT NULL REFERENCES organizations(id), user_id uuid NOT NULL REFERENCES users(id), revision integer NOT NULL DEFAULT 1,
 PRIMARY KEY(organization_id,user_id)
);
CREATE FUNCTION initialize_access_roles(org uuid) RETURNS void LANGUAGE sql AS $$
 INSERT INTO access_roles(organization_id,system_key,name,permissions,is_template)
 SELECT org,s.key,s.name,s.permissions,s.template FROM (VALUES
('admin','Administrator','["access:read", "access:manage", "team:read", "team:write", "kitchen:read", "kitchen:write", "kitchen:manage", "finance:read", "activity:read", "events:admin", "events:staff", "notifications:read", "devices:read", "devices:write", "plans:read", "plans:write", "products:read", "products:write", "sessions:read", "sessions:write", "stats:read", "transactions:read", "transactions:write", "player-plans:read", "player-plans:write", "players:read", "players:write", "units:read", "units:write", "shifts:read", "shifts:write", "shifts:force_close", "cash-registers:read", "cash-registers:write", "cash-registers:reconcile", "cash-registers:adjust_opening", "cash-deposits:read", "cash-deposits:write", "cash-deposits:approve", "credit:read", "credit:write", "credit-limit:write", "staff-gaming-allowance:read", "staff-gaming-allowance:write", "expenses:read", "expenses:write", "expenses:approve", "vendors:read", "vendors:write", "config:read", "config:write", "settings:read", "settings:write", "rules:read", "rules:edit", "rules:publish", "games:read", "games:write", "inventory:read", "inventory:manage", "inventory:transfer_request", "inventory:transfer_fulfill", "inventory:waste_record", "inventory:waste_approve", "procurement:read", "procurement:write", "procurement:approve", "procurement:receive", "inventory:reorder_manage"]'::jsonb,false),
('staff','Counter operator','["devices:read", "plans:read", "products:read", "sessions:read", "sessions:write", "stats:read", "transactions:read", "transactions:write", "player-plans:read", "player-plans:write", "players:read", "players:write", "units:read", "shifts:read", "shifts:write", "cash-registers:read", "cash-registers:write", "cash-deposits:read", "cash-deposits:write", "credit:read", "credit:write", "expenses:read", "settings:read", "rules:read", "games:read", "inventory:read", "inventory:transfer_request", "inventory:waste_record", "procurement:read", "procurement:write", "procurement:receive", "kitchen:read", "kitchen:write", "events:staff", "team:read", "notifications:read"]'::jsonb,false),
('template-manager','Venue manager','["kitchen:read", "kitchen:write", "kitchen:manage", "finance:read", "activity:read", "notifications:read", "devices:read", "devices:write", "plans:read", "plans:write", "products:read", "products:write", "sessions:read", "sessions:write", "stats:read", "transactions:read", "transactions:write", "player-plans:read", "player-plans:write", "players:read", "players:write", "units:read", "units:write", "shifts:read", "shifts:force_close", "cash-registers:read", "cash-registers:reconcile", "cash-registers:adjust_opening", "cash-deposits:read", "cash-deposits:write", "cash-deposits:approve", "credit:read", "credit:write", "credit-limit:write", "staff-gaming-allowance:read", "staff-gaming-allowance:write", "expenses:read", "expenses:write", "expenses:approve", "vendors:read", "vendors:write", "config:read", "config:write", "settings:read", "settings:write", "rules:read", "rules:edit", "rules:publish", "games:read", "games:write", "inventory:read", "inventory:manage", "inventory:transfer_request", "inventory:transfer_fulfill", "inventory:waste_record", "inventory:waste_approve", "procurement:read", "procurement:write", "procurement:approve", "procurement:receive", "inventory:reorder_manage"]'::jsonb,true),
('template-counter','Counter operator','["devices:read", "plans:read", "products:read", "sessions:read", "sessions:write", "stats:read", "transactions:read", "transactions:write", "player-plans:read", "player-plans:write", "players:read", "players:write", "units:read", "shifts:read", "shifts:write", "cash-registers:read", "cash-registers:write", "cash-deposits:read", "cash-deposits:write", "credit:read", "credit:write", "expenses:read", "settings:read", "rules:read", "games:read", "inventory:read", "inventory:transfer_request", "inventory:waste_record", "procurement:read", "procurement:write", "procurement:receive", "kitchen:read", "kitchen:write", "events:staff", "team:read", "notifications:read"]'::jsonb,true),
('template-kitchen','Kitchen operator','["kitchen:read", "kitchen:write"]'::jsonb,true),
('template-finance','Finance reviewer','["finance:read", "expenses:read", "vendors:read", "transactions:read", "credit:read", "cash-deposits:read", "cash-registers:read", "shifts:read"]'::jsonb,true),
('template-auditor','Auditor','["finance:read", "expenses:read", "vendors:read", "transactions:read", "credit:read", "cash-deposits:read", "cash-registers:read", "shifts:read", "activity:read"]'::jsonb,true)
 ) AS s(key,name,permissions,template) ON CONFLICT(organization_id,system_key) DO NOTHING;
$$;
SELECT initialize_access_roles(id) FROM organizations;
INSERT INTO access_assignments(organization_id,user_id,role_id)
 SELECT m."organizationId",m."userId",r.id FROM organization_memberships m
 JOIN users u ON u.id=m."userId" JOIN access_roles r ON r.organization_id=m."organizationId" AND r.system_key=u.role
 WHERE u.role IN ('admin','staff') ON CONFLICT DO NOTHING;
CREATE FUNCTION initialize_organization_access() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN PERFORM initialize_access_roles(NEW.id); RETURN NEW; END $$;
CREATE TRIGGER organization_access_defaults AFTER INSERT ON organizations FOR EACH ROW EXECUTE FUNCTION initialize_organization_access();
CREATE FUNCTION initialize_member_access() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 INSERT INTO access_assignments(organization_id,user_id,role_id)
 SELECT NEW."organizationId", NEW."userId",r.id FROM access_roles r JOIN users u ON u.id=NEW."userId"
 WHERE r.organization_id=NEW."organizationId" AND r.system_key=u.role AND u.role IN ('admin','staff') ON CONFLICT DO NOTHING;
 RETURN NEW;
END $$;
CREATE TRIGGER membership_access_defaults AFTER INSERT ON organization_memberships FOR EACH ROW EXECUTE FUNCTION initialize_member_access();
