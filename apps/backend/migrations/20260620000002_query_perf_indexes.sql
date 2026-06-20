-- Composite and partial B-tree indexes for hot query paths.

CREATE INDEX idx_transactions_credit_player
  ON transactions ("playerId", "paymentMethod", "paymentStatus")
  WHERE "deletedAt" IS NULL;

CREATE INDEX idx_sessions_open_device
  ON usage_sessions ("deviceId")
  WHERE "endTime" IS NULL AND "deletedAt" IS NULL;

CREATE INDEX idx_cash_register_entries_register_type
  ON cash_register_entries ("cashRegisterId", "entryType");

CREATE INDEX idx_transactions_player_date
  ON transactions ("playerId", "transactionDate" DESC)
  WHERE "deletedAt" IS NULL;

CREATE INDEX idx_users_session_otp
  ON users ("sessionOtpId")
  WHERE "sessionOtpId" IS NOT NULL;

CREATE INDEX idx_users_role_created
  ON users (role, "createdAt")
  WHERE "deletedAt" IS NULL;
