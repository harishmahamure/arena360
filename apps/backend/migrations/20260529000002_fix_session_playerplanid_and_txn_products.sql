-- Fix 1: Make playerPlanId nullable on usage_sessions (balanceId is the new FK)
ALTER TABLE usage_sessions ALTER COLUMN "playerPlanId" DROP NOT NULL;

-- Fix 2: Add unitPrice to transaction_products if the TypeORM baseline table lacked it
ALTER TABLE transaction_products ADD COLUMN IF NOT EXISTS "unitPrice" NUMERIC(10,2) NOT NULL DEFAULT 0;
