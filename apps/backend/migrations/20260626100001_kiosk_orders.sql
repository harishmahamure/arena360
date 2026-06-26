-- Kiosk player ordering (DRAFT-0051)

CREATE TYPE kiosk_order_status AS ENUM (
  'pending',
  'preparing',
  'fulfilled',
  'cancelled'
);

CREATE TABLE kiosk_orders (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "sessionId" UUID NOT NULL REFERENCES usage_sessions(id),
  "playerId" UUID NOT NULL REFERENCES users(id),
  "deviceId" UUID NOT NULL REFERENCES devices(id),
  status kiosk_order_status NOT NULL DEFAULT 'pending',
  "playerNote" TEXT,
  "transactionId" UUID REFERENCES transactions(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "fulfilledAt" TIMESTAMPTZ
);

CREATE TABLE kiosk_order_items (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "orderId" UUID NOT NULL REFERENCES kiosk_orders(id) ON DELETE CASCADE,
  "productId" UUID NOT NULL REFERENCES products(id),
  quantity INTEGER NOT NULL CHECK (quantity > 0),
  "productName" TEXT NOT NULL,
  "unitPrice" NUMERIC(10,2) NOT NULL CHECK ("unitPrice" >= 0),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX idx_kiosk_orders_session ON kiosk_orders ("sessionId");
CREATE INDEX idx_kiosk_orders_status ON kiosk_orders (status) WHERE status IN ('pending', 'preparing');
CREATE INDEX idx_kiosk_orders_device ON kiosk_orders ("deviceId", "createdAt" DESC);
CREATE INDEX idx_kiosk_order_items_order ON kiosk_order_items ("orderId");

-- One open order per session
CREATE UNIQUE INDEX uniq_kiosk_orders_session_open
  ON kiosk_orders ("sessionId")
  WHERE status IN ('pending', 'preparing');

ALTER TYPE activity_kind ADD VALUE IF NOT EXISTS 'kiosk_order_placed';
ALTER TYPE activity_kind ADD VALUE IF NOT EXISTS 'kiosk_order_fulfilled';
ALTER TYPE activity_kind ADD VALUE IF NOT EXISTS 'kiosk_order_cancelled';
