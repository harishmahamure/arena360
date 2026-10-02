DROP TABLE IF EXISTS transaction_product_options;
DROP TABLE IF EXISTS product_option_ingredients;
DROP TABLE IF EXISTS product_options;
DROP TABLE IF EXISTS product_option_groups;
DROP TABLE IF EXISTS product_recipe_items;
ALTER TABLE products DROP COLUMN IF EXISTS "isRawMaterial";
