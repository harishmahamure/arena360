use chrono::Utc;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    kiosk_order_status, KioskOrder, KioskOrderFilterDto, KioskOrderItem, KioskOrderWithItems,
};

pub struct KioskOrderRepository {
    pool: PgPool,
}

impl KioskOrderRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn find_by_id(&self, id: Uuid) -> Result<Option<KioskOrder>, AppError> {
        let row = sqlx::query_as::<_, KioskOrder>(
            r#"
            SELECT id,
                   "sessionId" as session_id,
                   "playerId" as player_id,
                   "deviceId" as device_id,
                   status::text as status,
                   "playerNote" as player_note,
                   "transactionId" as transaction_id,
                   "createdAt" as created_at,
                   "updatedAt" as updated_at,
                   "fulfilledAt" as fulfilled_at
            FROM kiosk_orders
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list_items(&self, order_id: Uuid) -> Result<Vec<KioskOrderItem>, AppError> {
        let rows = sqlx::query_as::<_, KioskOrderItem>(
            r#"
            SELECT id,
                   "orderId" as order_id,
                   "productId" as product_id,
                   quantity,
                   "productName" as product_name,
                   "unitPrice"::float8 as unit_price,
                   "createdAt" as created_at
            FROM kiosk_order_items
            WHERE "orderId" = $1
            ORDER BY "createdAt" ASC
            "#,
        )
        .bind(order_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows)
    }

    pub async fn find_open_for_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<KioskOrder>, AppError> {
        let row = sqlx::query_as::<_, KioskOrder>(
            r#"
            SELECT id,
                   "sessionId" as session_id,
                   "playerId" as player_id,
                   "deviceId" as device_id,
                   status::text as status,
                   "playerNote" as player_note,
                   "transactionId" as transaction_id,
                   "createdAt" as created_at,
                   "updatedAt" as updated_at,
                   "fulfilledAt" as fulfilled_at
            FROM kiosk_orders
            WHERE "sessionId" = $1
              AND status IN ('pending', 'preparing')
            ORDER BY "createdAt" DESC
            LIMIT 1
            "#,
        )
        .bind(session_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row)
    }

    pub async fn list(
        &self,
        filters: &KioskOrderFilterDto,
    ) -> Result<PaginationResult<KioskOrderWithItems>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut count_qb = QueryBuilder::<Postgres>::new(
            r#"SELECT COUNT(*)::bigint FROM kiosk_orders ko WHERE 1=1 "#,
        );
        let mut list_qb = QueryBuilder::<Postgres>::new(
            r#"
            SELECT ko.id,
                   ko."sessionId" as session_id,
                   ko."playerId" as player_id,
                   ko."deviceId" as device_id,
                   ko.status::text as status,
                   ko."playerNote" as player_note,
                   ko."transactionId" as transaction_id,
                   ko."createdAt" as created_at,
                   ko."updatedAt" as updated_at,
                   ko."fulfilledAt" as fulfilled_at,
                   d.name as device_name,
                   u.username as player_username
            FROM kiosk_orders ko
            LEFT JOIN devices d ON d.id = ko."deviceId"
            LEFT JOIN users u ON u.id = ko."playerId"
            WHERE 1=1
            "#,
        );

        if let Some(status) = &filters.status {
            count_qb.push(" AND ko.status = ");
            count_qb.push_bind(status);
            list_qb.push(" AND ko.status = ");
            list_qb.push_bind(status);
        }
        if let Some(device_id) = filters.device_id {
            count_qb.push(r#" AND ko."deviceId" = "#);
            count_qb.push_bind(device_id);
            list_qb.push(r#" AND ko."deviceId" = "#);
            list_qb.push_bind(device_id);
        }

        let total: i64 = count_qb
            .build_query_scalar()
            .fetch_one(&self.pool)
            .await?;

        list_qb.push(r#" ORDER BY ko."createdAt" DESC LIMIT "#);
        list_qb.push_bind(limit);
        list_qb.push(" OFFSET ");
        list_qb.push_bind(offset);

        #[derive(sqlx::FromRow)]
        struct OrderRow {
            id: Uuid,
            session_id: Uuid,
            player_id: Uuid,
            device_id: Uuid,
            status: String,
            player_note: Option<String>,
            transaction_id: Option<Uuid>,
            created_at: chrono::DateTime<Utc>,
            updated_at: chrono::DateTime<Utc>,
            fulfilled_at: Option<chrono::DateTime<Utc>>,
            device_name: Option<String>,
            player_username: Option<String>,
        }

        let rows: Vec<OrderRow> = list_qb.build_query_as().fetch_all(&self.pool).await?;

        let mut data = Vec::with_capacity(rows.len());
        for row in rows {
            let order = KioskOrder {
                id: row.id,
                session_id: row.session_id,
                player_id: row.player_id,
                device_id: row.device_id,
                status: row.status,
                player_note: row.player_note,
                transaction_id: row.transaction_id,
                created_at: row.created_at,
                updated_at: row.updated_at,
                fulfilled_at: row.fulfilled_at,
            };
            let items = self.list_items(order.id).await?;
            data.push(KioskOrderWithItems::from_parts(
                order,
                items,
                row.device_name,
                row.player_username,
            ));
        }

        Ok(PaginationResult::new(data, total, page, limit))
    }

    pub async fn get_with_details(&self, id: Uuid) -> Result<KioskOrderWithItems, AppError> {
        #[derive(sqlx::FromRow)]
        struct DetailRow {
            id: Uuid,
            session_id: Uuid,
            player_id: Uuid,
            device_id: Uuid,
            status: String,
            player_note: Option<String>,
            transaction_id: Option<Uuid>,
            created_at: chrono::DateTime<Utc>,
            updated_at: chrono::DateTime<Utc>,
            fulfilled_at: Option<chrono::DateTime<Utc>>,
            device_name: Option<String>,
            player_username: Option<String>,
        }

        let row = sqlx::query_as::<_, DetailRow>(
            r#"
            SELECT ko.id,
                   ko."sessionId" as session_id,
                   ko."playerId" as player_id,
                   ko."deviceId" as device_id,
                   ko.status::text as status,
                   ko."playerNote" as player_note,
                   ko."transactionId" as transaction_id,
                   ko."createdAt" as created_at,
                   ko."updatedAt" as updated_at,
                   ko."fulfilledAt" as fulfilled_at,
                   d.name as device_name,
                   u.username as player_username
            FROM kiosk_orders ko
            LEFT JOIN devices d ON d.id = ko."deviceId"
            LEFT JOIN users u ON u.id = ko."playerId"
            WHERE ko.id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found_code("KIOSK_ORDER_NOT_FOUND"))?;

        let order = KioskOrder {
            id: row.id,
            session_id: row.session_id,
            player_id: row.player_id,
            device_id: row.device_id,
            status: row.status,
            player_note: row.player_note,
            transaction_id: row.transaction_id,
            created_at: row.created_at,
            updated_at: row.updated_at,
            fulfilled_at: row.fulfilled_at,
        };
        let items = self.list_items(id).await?;
        Ok(KioskOrderWithItems::from_parts(
            order,
            items,
            row.device_name,
            row.player_username,
        ))
    }

    pub async fn create_with_items(
        &self,
        session_id: Uuid,
        player_id: Uuid,
        device_id: Uuid,
        player_note: Option<String>,
        line_items: &[(Uuid, i32, String, f64)],
    ) -> Result<KioskOrderWithItems, AppError> {
        let mut tx = self.pool.begin().await?;

        let order = sqlx::query_as::<_, KioskOrder>(
            r#"
            INSERT INTO kiosk_orders ("sessionId", "playerId", "deviceId", "playerNote")
            VALUES ($1, $2, $3, $4)
            RETURNING id,
                      "sessionId" as session_id,
                      "playerId" as player_id,
                      "deviceId" as device_id,
                      status::text as status,
                      "playerNote" as player_note,
                      "transactionId" as transaction_id,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "fulfilledAt" as fulfilled_at
            "#,
        )
        .bind(session_id)
        .bind(player_id)
        .bind(device_id)
        .bind(&player_note)
        .fetch_one(&mut *tx)
        .await
        .map_err(|e| {
            if let sqlx::Error::Database(db) = &e {
                if db.constraint() == Some("uniq_kiosk_orders_session_open") {
                    return AppError::conflict_code("KIOSK_ORDER_ALREADY_OPEN", None);
                }
            }
            AppError::from(e)
        })?;

        for (product_id, quantity, product_name, unit_price) in line_items {
            sqlx::query(
                r#"
                INSERT INTO kiosk_order_items ("orderId", "productId", quantity, "productName", "unitPrice")
                VALUES ($1, $2, $3, $4, $5)
                "#,
            )
            .bind(order.id)
            .bind(product_id)
            .bind(quantity)
            .bind(product_name)
            .bind(unit_price)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;

        self.get_with_details(order.id).await
    }

    pub async fn update_status(
        &self,
        id: Uuid,
        status: &str,
    ) -> Result<KioskOrder, AppError> {
        if !matches!(
            status,
            kiosk_order_status::PREPARING | kiosk_order_status::CANCELLED
        ) {
            return Err(AppError::BadRequest(format!(
                "Invalid status transition to {status}"
            )));
        }

        let order = sqlx::query_as::<_, KioskOrder>(
            r#"
            UPDATE kiosk_orders
            SET status = $2::kiosk_order_status,
                "updatedAt" = NOW()
            WHERE id = $1
              AND status IN ('pending', 'preparing')
            RETURNING id,
                      "sessionId" as session_id,
                      "playerId" as player_id,
                      "deviceId" as device_id,
                      status::text as status,
                      "playerNote" as player_note,
                      "transactionId" as transaction_id,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "fulfilledAt" as fulfilled_at
            "#,
        )
        .bind(id)
        .bind(status)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found_code("KIOSK_ORDER_NOT_FOUND"))?;

        Ok(order)
    }

    pub async fn mark_fulfilled(
        &self,
        id: Uuid,
        transaction_id: Uuid,
    ) -> Result<KioskOrder, AppError> {
        let order = sqlx::query_as::<_, KioskOrder>(
            r#"
            UPDATE kiosk_orders
            SET status = 'fulfilled'::kiosk_order_status,
                "transactionId" = $2,
                "fulfilledAt" = NOW(),
                "updatedAt" = NOW()
            WHERE id = $1
              AND status IN ('pending', 'preparing')
            RETURNING id,
                      "sessionId" as session_id,
                      "playerId" as player_id,
                      "deviceId" as device_id,
                      status::text as status,
                      "playerNote" as player_note,
                      "transactionId" as transaction_id,
                      "createdAt" as created_at,
                      "updatedAt" as updated_at,
                      "fulfilledAt" as fulfilled_at
            "#,
        )
        .bind(id)
        .bind(transaction_id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| AppError::not_found_code("KIOSK_ORDER_NOT_FOUND"))?;

        Ok(order)
    }
}
