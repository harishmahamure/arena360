CREATE TABLE IF NOT EXISTS transaction_products (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    "transactionId" UUID NOT NULL REFERENCES transactions(id) ON DELETE CASCADE,
    "productId" UUID NOT NULL REFERENCES products(id),
    quantity INTEGER NOT NULL CHECK (quantity > 0),
    "unitPrice" NUMERIC(10,2) NOT NULL CHECK ("unitPrice" >= 0),
    "createdBy" UUID REFERENCES users(id),
    "updatedBy" UUID REFERENCES users(id),
    "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_transaction_products_txn ON transaction_products("transactionId");
CREATE INDEX IF NOT EXISTS idx_transaction_products_product ON transaction_products("productId");
