-- Shared-table tenant foundation. Existing rows belong to the bootstrap business.
-- New operational tenants must supply organizationId explicitly in application writes;
-- the default remains during the single-tenant compatibility rollout.
CREATE TEMP TABLE tenant_scope_tables (name text PRIMARY KEY);
INSERT INTO tenant_scope_tables (name) VALUES
  ('activity_log'),
  ('analytics_outbox'),
  ('cash_deposits'),
  ('cash_register_entries'),
  ('cash_registers'),
  ('configurations'),
  ('credit_settlement_items'),
  ('credit_settlements'),
  ('devices'),
  ('expense_categories'),
  ('expenses'),
  ('games'),
  ('inventory_locations'),
  ('inventory_reorder_rules'),
  ('kiosk_order_items'),
  ('kiosk_orders'),
  ('kitchen_menu_settings'),
  ('kitchen_ticket_events'),
  ('kitchen_tickets'),
  ('location_stock'),
  ('plans'),
  ('player_plan_balances'),
  ('player_plan_ledger'),
  ('player_plans_legacy'),
  ('pricing_rule_sets'),
  ('pricing_rule_versions'),
  ('product_option_groups'),
  ('product_option_ingredients'),
  ('product_options'),
  ('product_recipe_items'),
  ('products'),
  ('purchase_order_lines'),
  ('purchase_orders'),
  ('setting_overrides'),
  ('setting_revisions'),
  ('shifts'),
  ('stock_adjustment_lines'),
  ('stock_adjustments'),
  ('stock_movements'),
  ('stock_receipt_lines'),
  ('stock_receipts'),
  ('stock_transfer_lines'),
  ('stock_transfer_requests'),
  ('stock_waste_events'),
  ('stock_waste_lines'),
  ('transaction_product_options'),
  ('transaction_products'),
  ('transactions'),
  ('usage_sessions'),
  ('user_notifications'),
  ('vendors'),
  ('venue_locations');

DO $tenant$
DECLARE
  table_name text;
  has_id boolean;
BEGIN
  FOR table_name IN SELECT name FROM tenant_scope_tables ORDER BY name LOOP
    IF NOT EXISTS (
      SELECT 1 FROM pg_attribute
      WHERE attrelid = format('public.%I', table_name)::regclass
        AND attname = 'organizationId' AND attnum > 0 AND NOT attisdropped
    ) THEN
      EXECUTE format(
        'ALTER TABLE public.%I ADD COLUMN "organizationId" uuid NOT NULL DEFAULT %L::uuid REFERENCES organizations(id)',
        table_name, '00000000-0000-4000-8000-000000000001'
      );
    END IF;
    EXECUTE format(
      'CREATE INDEX IF NOT EXISTS %I ON public.%I ("organizationId")',
      'tenant_' || table_name || '_org_idx', table_name
    );
    SELECT EXISTS (
      SELECT 1 FROM pg_attribute
      WHERE attrelid = format('public.%I', table_name)::regclass
        AND attname = 'id' AND attnum > 0 AND NOT attisdropped
    ) INTO has_id;
    IF has_id THEN
      EXECUTE format(
        'CREATE UNIQUE INDEX IF NOT EXISTS %I ON public.%I (id, "organizationId")',
        'tenant_' || table_name || '_id_org_uq', table_name
      );
    END IF;
  END LOOP;
END $tenant$;

-- Propagate ownership from rows that already carried an organization ID,
-- including secondary-business pricing sets and their published versions.
DO $tenant$
DECLARE
  edge record;
  join_condition text;
  pass integer;
  changed integer;
  total_changed integer;
BEGIN
  FOR pass IN 1..20 LOOP
    total_changed := 0;
    FOR edge IN
      SELECT c.*, child.relname AS child_name, parent.relname AS parent_name
      FROM pg_constraint c
      JOIN pg_class child ON child.oid = c.conrelid
      JOIN pg_class parent ON parent.oid = c.confrelid
      JOIN tenant_scope_tables ct ON ct.name = child.relname
      JOIN tenant_scope_tables pt ON pt.name = parent.relname
      WHERE c.contype = 'f'
        AND NOT EXISTS (
          SELECT 1 FROM unnest(c.conkey) key_att
          JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = key_att
          WHERE a.attname = 'organizationId'
        )
      ORDER BY child.relname, c.conname
    LOOP
      SELECT string_agg(
        format('c.%I = p.%I', child_att.attname, parent_att.attname),
        ' AND ' ORDER BY child_key.position
      ) INTO join_condition
      FROM unnest(edge.conkey) WITH ORDINALITY child_key(attnum, position)
      JOIN unnest(edge.confkey) WITH ORDINALITY parent_key(attnum, position)
        ON parent_key.position = child_key.position
      JOIN pg_attribute child_att
        ON child_att.attrelid = edge.conrelid AND child_att.attnum = child_key.attnum
      JOIN pg_attribute parent_att
        ON parent_att.attrelid = edge.confrelid AND parent_att.attnum = parent_key.attnum;
      EXECUTE format(
        'UPDATE public.%I c SET "organizationId" = p."organizationId"
         FROM public.%I p WHERE %s AND c."organizationId" = %L::uuid
           AND p."organizationId" <> %L::uuid',
        edge.child_name, edge.parent_name, join_condition,
        '00000000-0000-4000-8000-000000000001',
        '00000000-0000-4000-8000-000000000001'
      );
      GET DIAGNOSTICS changed = ROW_COUNT;
      total_changed := total_changed + changed;
    END LOOP;
    EXIT WHEN total_changed = 0;
  END LOOP;
  IF total_changed <> 0 THEN
    RAISE EXCEPTION 'Tenant ownership backfill did not converge';
  END IF;
END $tenant$;

-- Keep every tenant-owned reference inside the same business. The original
-- foreign keys remain for their delete behavior; these add the tenant match.
DO $tenant$
DECLARE
  edge record;
  child_columns text;
  parent_columns text;
  parent_index text;
  scope_constraint text;
BEGIN
  FOR edge IN
    SELECT c.*, child.relname AS child_name, parent.relname AS parent_name
    FROM pg_constraint c
    JOIN pg_class child ON child.oid = c.conrelid
    JOIN pg_class parent ON parent.oid = c.confrelid
    JOIN tenant_scope_tables ct ON ct.name = child.relname
    JOIN tenant_scope_tables pt ON pt.name = parent.relname
    WHERE c.contype = 'f'
      AND NOT EXISTS (
        SELECT 1 FROM unnest(c.conkey) key_att
        JOIN pg_attribute a ON a.attrelid = c.conrelid AND a.attnum = key_att
        WHERE a.attname = 'organizationId'
      )
    ORDER BY child.relname, c.conname
  LOOP
    SELECT string_agg(format('%I', a.attname), ', ' ORDER BY keys.position)
      INTO child_columns
    FROM unnest(edge.conkey) WITH ORDINALITY keys(attnum, position)
    JOIN pg_attribute a ON a.attrelid = edge.conrelid AND a.attnum = keys.attnum;
    SELECT string_agg(format('%I', a.attname), ', ' ORDER BY keys.position)
      INTO parent_columns
    FROM unnest(edge.confkey) WITH ORDINALITY keys(attnum, position)
    JOIN pg_attribute a ON a.attrelid = edge.confrelid AND a.attnum = keys.attnum;
    parent_index := 'tenant_ref_' || substr(md5(edge.parent_name || ':' || parent_columns), 1, 16);
    scope_constraint := 'tenant_scope_' || substr(md5(edge.child_name || ':' || edge.conname), 1, 16);
    IF parent_columns <> 'id' THEN
      EXECUTE format(
        'CREATE UNIQUE INDEX IF NOT EXISTS %I ON public.%I (%s, "organizationId")',
        parent_index, edge.parent_name, parent_columns
      );
    END IF;
    EXECUTE format(
      'ALTER TABLE public.%I ADD CONSTRAINT %I FOREIGN KEY (%s, "organizationId") REFERENCES public.%I (%s, "organizationId") DEFERRABLE INITIALLY DEFERRED',
      edge.child_name, scope_constraint, child_columns, edge.parent_name, parent_columns
    );
  END LOOP;
END $tenant$;

CREATE UNIQUE INDEX devices_tenant_active_name_uq
  ON devices ("organizationId", lower(name)) WHERE "deletedAt" IS NULL;
ALTER TABLE devices DROP CONSTRAINT "UQ_devices_name";

CREATE UNIQUE INDEX configurations_tenant_key_uq
  ON configurations ("organizationId", key);
ALTER TABLE configurations DROP CONSTRAINT configurations_key_key;
DROP INDEX IF EXISTS idx_configurations_key;

DROP TABLE tenant_scope_tables;
