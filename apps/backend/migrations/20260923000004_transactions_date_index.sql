-- no-transaction
CREATE INDEX CONCURRENTLY IF NOT EXISTS idx_transactions_active_date_id ON transactions ("transactionDate" DESC, id DESC) WHERE "deletedAt" IS NULL;
