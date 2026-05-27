use sqlx::{PgPool, Postgres, Transaction as SqlxTransaction};
use uuid::Uuid;

use crate::error::AppError;
use crate::models::{CreateLineItemDto, TransactionProductResponse};

pub struct TransactionProductRepository {
    pool: PgPool,
}

impl TransactionProductRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert_many(
        tx: &mut SqlxTransaction<'_, Postgres>,
        transaction_id: Uuid,
        line_items: &[CreateLineItemDto],
        actor_id: Option<Uuid>,
    ) -> Result<(), AppError> {
        for item in line_items {
            sqlx::query(
                r#"
                INSERT INTO transaction_products (
                    id, "transactionId", "productId", quantity, "unitPrice",
                    "createdBy", "updatedBy", "createdAt", "updatedAt"
                )
                VALUES (gen_random_uuid(), $1, $2, $3, $4, $5, $5, NOW(), NOW())
                "#,
            )
            .bind(transaction_id)
            .bind(item.product_id)
            .bind(item.quantity)
            .bind(item.unit_price.unwrap_or(0.0))
            .bind(actor_id)
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
