DROP TABLE IF EXISTS configurations CASCADE;

CREATE TABLE configurations (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  key VARCHAR(100) NOT NULL UNIQUE,
  value JSONB NOT NULL DEFAULT '{}',
  category VARCHAR(50) NOT NULL DEFAULT 'general',
  description TEXT,
  "createdBy" UUID REFERENCES users(id),
  "updatedBy" UUID REFERENCES users(id),
  "createdAt" TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  "updatedAt" TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE UNIQUE INDEX idx_configurations_key ON configurations(key);

-- Seed default configuration values
INSERT INTO configurations (key, value, category, description) VALUES
  ('business.name', '"Gaming Cafe"', 'business', 'Business name'),
  ('business.address', '""', 'business', 'Business address'),
  ('business.phone', '""', 'business', 'Business phone number'),
  ('business.email', '""', 'business', 'Business email'),
  ('business.gst_number', '""', 'business', 'GST registration number'),
  ('business.logo_url', '""', 'business', 'Business logo URL'),
  ('receipt.header_text', '"Thank you for visiting!"', 'receipt', 'Receipt header text'),
  ('receipt.footer_text', '"Visit again soon!"', 'receipt', 'Receipt footer text'),
  ('receipt.show_gst', 'true', 'receipt', 'Show GST on receipts'),
  ('pricing.default_per_minute_rate', '2.0', 'pricing', 'Default per-minute rate in INR'),
  ('pricing.currency', '"INR"', 'pricing', 'Currency code'),
  ('pricing.tax_rate', '18.0', 'pricing', 'Tax rate percentage')
ON CONFLICT (key) DO NOTHING;
