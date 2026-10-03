-- Recipes and inventory quantities are stored in the product's stock unit.
-- Correct metric purchase multipliers without guessing what existing stock
-- counts meant; incorrect physical counts must use an audited stock adjustment.
UPDATE products p
SET "unitsPerPurchaseUnit" = CASE
      WHEN purchase.type::text = stock.type::text THEN 1
      ELSE 1000
    END,
    "updatedAt" = NOW()
FROM units purchase, units stock
WHERE purchase.id = p."purchaseUnitId"
  AND stock.id = p."unitId"
  AND (
    (purchase.type::text = 'kilogram' AND stock.type::text = 'gram')
    OR (purchase.type::text = 'liter' AND stock.type::text = 'milliliter')
    OR (purchase.type::text = stock.type::text
        AND stock.type::text IN ('kilogram', 'gram', 'liter', 'milliliter'))
  )
  AND p."unitsPerPurchaseUnit" IS DISTINCT FROM CASE
    WHEN purchase.type::text = stock.type::text THEN 1 ELSE 1000 END;

-- Open orders must use the corrected conversion for their remaining receipts.
-- Completed receipts keep their original audit quantities.
UPDATE purchase_order_lines line
SET "unitsPerBoxSnapshot" = p."unitsPerPurchaseUnit"
FROM products p, units purchase, units stock, purchase_orders po
WHERE line."productId" = p.id
  AND po.id = line."purchaseOrderId"
  AND po.status::text IN ('draft', 'submitted', 'rejected', 'approved', 'ordered', 'partially_received')
  AND purchase.id = p."purchaseUnitId"
  AND stock.id = p."unitId"
  AND (
    (purchase.type::text = 'kilogram' AND stock.type::text = 'gram')
    OR (purchase.type::text = 'liter' AND stock.type::text = 'milliliter')
    OR (purchase.type::text = stock.type::text
        AND stock.type::text IN ('kilogram', 'gram', 'liter', 'milliliter'))
  )
  AND line."unitsPerBoxSnapshot" IS DISTINCT FROM p."unitsPerPurchaseUnit";
