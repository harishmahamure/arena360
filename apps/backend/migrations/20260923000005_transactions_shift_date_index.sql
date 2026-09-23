-- no-transaction
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_transactions_active_shift_date_id ON transactions ("shiftId", "transactionDate" DESC, id DESC) WHERE "deletedAt" IS NULL;
