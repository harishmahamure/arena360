# DB spike: transaction product line items

**Purpose**: Read-only investigation before implementing product line-item persistence.  
**Owner**: Backend / platform  
**Estimated effort**: 1–2 hours  
**Blocks**: [DRAFT-0010](../adr/DRAFT-0010-transaction-product-line-items.md)

## Questions to answer

| # | Question | Answer (fill in) |
|---|----------|------------------|
| 1 | Does `transaction_products` (or similar) exist in Postgres? | |
| 2 | Exact table name and column list (`\d table_name`) | |
| 3 | Foreign keys to `transactions` and `products`? | |
| 4 | Approximate row count / sample data | |
| 5 | Does NestJS TypeORM entity still exist in old repo? | |
| 6 | Is `products.stockQuantity` updated on purchase today? | |
| 7 | Should refunds restore stock per line item? | |

## How to run

Connect to dev or staging Postgres:

```bash
psql "$DATABASE_URL"
```

```sql
-- List candidate tables
\dt *transaction*
\dt *product*

-- If found, describe schema
\d transaction_products

-- Row count
SELECT COUNT(*) FROM transaction_products;

-- Sample join
SELECT t.id, tp.*
FROM transactions t
LEFT JOIN transaction_products tp ON tp."transactionId" = t.id
WHERE t."transactionType" = 'product_purchase'
LIMIT 5;
```

## Deliverables

1. Completed table above pasted into this doc or linked issue.
2. Recommendation: reuse legacy table vs new migration vs JSONB.
3. If new migration needed → keep DRAFT-0010 in **Proposed** until team signs off.

## Out of scope for spike

- Writing Rust repo code
- Running migrations
- Changing admin UI beyond informational note
