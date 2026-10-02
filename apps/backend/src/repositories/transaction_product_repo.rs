use sqlx::{PgPool, Postgres, Transaction as SqlxTransaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{CreateLineItemDto, SelectedOption, TransactionProductResponse};

pub struct TransactionProductRepository {
    pool: PgPool,
}

impl TransactionProductRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Returns the new line IDs in the order of `line_items`.
    pub async fn insert_many(
        tx: &mut SqlxTransaction<'_, Postgres>,
        transaction_id: Uuid,
        line_items: &[CreateLineItemDto],
        actor_id: Option<Uuid>,
    ) -> Result<Vec<Uuid>, AppError> {
        let mut ids = Vec::with_capacity(line_items.len());
        for item in line_items {
            let unit_price = item.unit_price.unwrap_or(0.0);
            let subtotal = unit_price * f64::from(item.quantity);
            // "priceAtPurchase" and "subtotal" are legacy NOT NULL columns from the
            // TypeORM baseline kept in sync with "unitPrice" (see migration
            // 20260530000003).
            let id: Uuid = sqlx::query_scalar(
                r#"
                INSERT INTO transaction_products (
                    id, "transactionId", "productId", quantity, "unitPrice",
                    "priceAtPurchase", subtotal,
                    "createdBy", "updatedBy", "createdAt", "updatedAt"
                )
                VALUES (gen_random_uuid(), $1, $2, $3, $4, $4, $5, $6, $6, NOW(), NOW())
                RETURNING id
                "#,
            )
            .bind(transaction_id)
            .bind(item.product_id)
            .bind(item.quantity)
            .bind(unit_price)
            .bind(subtotal)
            .bind(actor_id)
            .fetch_one(&mut **tx)
            .await?;
            ids.push(id);
        }
        Ok(ids)
    }

    pub async fn insert_options(
        tx: &mut SqlxTransaction<'_, Postgres>,
        line_id: Uuid,
        options: &[SelectedOption],
    ) -> Result<(), AppError> {
        for option in options {
            sqlx::query(
                r#"
                INSERT INTO transaction_product_options
                    ("transactionProductId", "optionId", "groupName", name, "priceDelta")
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(line_id)
            .bind(option.option_id)
            .bind(&option.group_name)
            .bind(&option.name)
            .bind(option.price_delta)
            .execute(&mut **tx)
            .await?;
        }
        Ok(())
    }

    pub async fn list_by_transaction(
        &self,
        transaction_id: Uuid,
    ) -> Result<Vec<TransactionProductResponse>, AppError> {
        let items = sqlx::query_as::<_, TransactionProductResponse>(
            r#"
            SELECT tp.id,
                   tp."transactionId" as transaction_id,
                   tp."productId" as product_id,
                   tp.quantity,
                   tp."unitPrice"::float8 as unit_price,
                   p.name as product_name,
                   p.sku as product_sku,
                   p.price::float8 as product_price,
                   ARRAY(
                     SELECT o.name::text FROM transaction_product_options o
                     WHERE o."transactionProductId" = tp.id ORDER BY o.id
                   ) as option_names,
                   tp."createdAt" as created_at
            FROM transaction_products tp
            INNER JOIN products p ON p.id = tp."productId"
            WHERE tp."transactionId" = $1
            ORDER BY tp."createdAt" ASC
            "#,
        )
        .bind(transaction_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(items)
    }
}
