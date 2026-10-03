-- Global identities can participate in several businesses, but operational
-- player and shift rows must refer to a membership in their own business.
ALTER TABLE shifts ADD CONSTRAINT tenant_shifts_member_fk
  FOREIGN KEY ("organizationId", "userId")
  REFERENCES organization_memberships ("organizationId", "userId")
  DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE transactions ADD CONSTRAINT tenant_transactions_player_fk
  FOREIGN KEY ("organizationId", "playerId")
  REFERENCES organization_memberships ("organizationId", "userId")
  DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE kiosk_orders ADD CONSTRAINT tenant_kiosk_orders_player_fk
  FOREIGN KEY ("organizationId", "playerId")
  REFERENCES organization_memberships ("organizationId", "userId")
  DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE player_plan_balances ADD CONSTRAINT tenant_balances_player_fk
  FOREIGN KEY ("organizationId", "playerId")
  REFERENCES organization_memberships ("organizationId", "userId")
  DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE player_plan_ledger ADD CONSTRAINT tenant_ledger_player_fk
  FOREIGN KEY ("organizationId", "playerId")
  REFERENCES organization_memberships ("organizationId", "userId")
  DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE player_plans_legacy ADD CONSTRAINT tenant_legacy_plans_player_fk
  FOREIGN KEY ("organizationId", "playerId")
  REFERENCES organization_memberships ("organizationId", "userId")
  DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE credit_settlements ADD CONSTRAINT tenant_credit_settlements_player_fk
  FOREIGN KEY ("organizationId", "playerId")
  REFERENCES organization_memberships ("organizationId", "userId")
  DEFERRABLE INITIALLY DEFERRED;
ALTER TABLE user_notifications ADD CONSTRAINT tenant_notifications_user_fk
  FOREIGN KEY ("organizationId", "userId")
  REFERENCES organization_memberships ("organizationId", "userId")
  DEFERRABLE INITIALLY DEFERRED;
