-- Raw materials, product recipes and product options.
-- A product with a recipe deducts its ingredients on sale instead of its own stock.
-- Quantities are whole numbers in each ingredient's own unit (pieces, grams, millilitres).

ALTER TABLE products ADD COLUMN IF NOT EXISTS "isRawMaterial" BOOLEAN NOT NULL DEFAULT false;

CREATE TABLE product_recipe_items (
  "productId" UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  "ingredientId" UUID NOT NULL REFERENCES products(id),
  quantity INTEGER NOT NULL CHECK (quantity > 0),
  PRIMARY KEY ("productId", "ingredientId"),
  CHECK ("productId" <> "ingredientId")
);
CREATE INDEX idx_product_recipe_items_ingredient ON product_recipe_items("ingredientId");

CREATE TABLE product_option_groups (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "productId" UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
  name VARCHAR(120) NOT NULL,
  required BOOLEAN NOT NULL DEFAULT false,
  multiple BOOLEAN NOT NULL DEFAULT false,
  "sortOrder" INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_product_option_groups_product ON product_option_groups("productId");

CREATE TABLE product_options (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "groupId" UUID NOT NULL REFERENCES product_option_groups(id) ON DELETE CASCADE,
  name VARCHAR(120) NOT NULL,
  "priceDelta" NUMERIC(19,4) NOT NULL DEFAULT 0,
  "sortOrder" INTEGER NOT NULL DEFAULT 0
);
CREATE INDEX idx_product_options_group ON product_options("groupId");

-- Negative quantities remove part of the base recipe, for example "No cheese".
CREATE TABLE product_option_ingredients (
  "optionId" UUID NOT NULL REFERENCES product_options(id) ON DELETE CASCADE,
  "ingredientId" UUID NOT NULL REFERENCES products(id),
  quantity INTEGER NOT NULL CHECK (quantity <> 0),
  PRIMARY KEY ("optionId", "ingredientId")
);

-- Name and price are copied so receipts survive later recipe edits.
CREATE TABLE transaction_product_options (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  "transactionProductId" UUID NOT NULL REFERENCES transaction_products(id) ON DELETE CASCADE,
  "optionId" UUID REFERENCES product_options(id) ON DELETE SET NULL,
  "groupName" VARCHAR(120) NOT NULL,
  name VARCHAR(120) NOT NULL,
  "priceDelta" NUMERIC(19,4) NOT NULL
);
CREATE INDEX idx_transaction_product_options_line
  ON transaction_product_options("transactionProductId");
