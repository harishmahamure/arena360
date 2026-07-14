use sqlx::PgPool;
use uuid::Uuid;

use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    activity_kind, ActivityLog, ActivityLogFilterDto, NotificationFilterDto, NotificationItem,
};
use crate::cache::keys::MAX_INBOX_NOTIFICATIONS;

#[derive(Clone)]
pub struct NotificationRepository {
    pool: PgPool,
}

impl NotificationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert_activity(
        &self,
        kind: &str,
        title: &str,
        summary: Option<&str>,
        payload: &serde_json::Value,
        actor_user_id: Option<Uuid>,
        entity_type: Option<&str>,
        entity_id: Option<Uuid>,
    ) -> Result<ActivityLog, AppError> {
        sqlx::query_as::<_, ActivityLog>(
            r#"INSERT INTO activity_log (kind, title, summary, payload, "actorUserId", "entityType", "entityId")
               VALUES ($1::activity_kind, $2, $3, $4, $5, $6, $7)
               RETURNING id, kind::text as kind, title, summary, payload,
                         "actorUserId" as actor_user_id, "entityType" as entity_type,
                         "entityId" as entity_id, "createdAt" as created_at"#,
        )
        .bind(kind)
        .bind(title)
        .bind(summary)
        .bind(payload)
        .bind(actor_user_id)
        .bind(entity_type)
        .bind(entity_id)
        .fetch_one(&self.pool)
        .await
        .map_err(Into::into)
    }

    pub async fn insert_user_notifications(
        &self,
        activity_id: Uuid,
        user_ids: &[Uuid],
    ) -> Result<Vec<Uuid>, AppError> {
        if user_ids.is_empty() {
            return Ok(vec![]);
        }

        let mut notification_ids = Vec::with_capacity(user_ids.len());
        for user_id in user_ids {
            let row: (Uuid,) = sqlx::query_as(
                r#"INSERT INTO user_notifications ("activityId", "userId")
                   VALUES ($1, $2)
                   ON CONFLICT ("activityId", "userId") DO UPDATE SET "activityId" = EXCLUDED."activityId"
                   RETURNING id"#,
            )
            .bind(activity_id)
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;
            notification_ids.push(row.0);
        }
        Ok(notification_ids)
    }

    pub async fn list_admin_user_ids(&self) -> Result<Vec<Uuid>, AppError> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r#"SELECT id FROM users
               WHERE role = 'admin' AND "isActive" = true AND "deletedAt" IS NULL"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    pub async fn list_staff_user_ids(&self) -> Result<Vec<Uuid>, AppError> {
        let rows: Vec<(Uuid,)> = sqlx::query_as(
            r#"SELECT id FROM users
               WHERE role IN ('admin', 'staff') AND "isActive" = true AND "deletedAt" IS NULL"#,
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(|r| r.0).collect())
    }

    pub async fn user_display_name(&self, user_id: Uuid) -> Result<String, AppError> {
        let row: Option<(Option<String>, Option<String>, String)> = sqlx::query_as(
            r#"SELECT "firstName", "lastName", username FROM users
               WHERE id = $1 AND "deletedAt" IS NULL"#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row
            .map(|(first, last, username)| {
                let full = format!(
                    "{} {}",
                    first.unwrap_or_default(),
                    last.unwrap_or_default()
                )
                .trim()
                .to_string();
                if full.is_empty() {
                    username
                } else {
                    full
                }
            })
            .unwrap_or_else(|| "Unknown".to_string()))
    }

    pub async fn list_notifications(
        &self,
        user_id: Uuid,
        filters: &NotificationFilterDto,
    ) -> Result<PaginationResult<NotificationItem>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters
            .limit
            .unwrap_or(MAX_INBOX_NOTIFICATIONS)
            .clamp(1, MAX_INBOX_NOTIFICATIONS);
        let offset = (page - 1) * limit;
        let unread_only = filters.unread_only.unwrap_or(false);
        let important_only = filters.important_only.unwrap_or(false);

        let mut count_builder = sqlx::QueryBuilder::new(
            r#"SELECT COUNT(*) FROM user_notifications un
               INNER JOIN activity_log al ON al.id = un."activityId"
               WHERE un."userId" = "#,
        );
        count_builder.push_bind(user_id);
        if unread_only {
            count_builder.push(r#" AND un."readAt" IS NULL"#);
        }
        if important_only {
            count_builder.push(r#" AND al.kind::text = ANY("#);
            let kinds: Vec<String> = activity_kind::STAFF_IMPORTANT
                .iter()
                .map(|s| s.to_string())
                .collect();
            count_builder.push_bind(kinds);
            count_builder.push(")");
        }
        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        let mut list_builder = sqlx::QueryBuilder::new(
            r#"SELECT un.id, un."activityId" as activity_id, al.kind::text as kind,
                      al.title, al.summary, al.payload,
                      al."actorUserId" as actor_user_id, al."entityType" as entity_type,
                      al."entityId" as entity_id, un."readAt" as read_at, un."createdAt" as created_at
               FROM user_notifications un
               INNER JOIN activity_log al ON al.id = un."activityId"
               WHERE un."userId" = "#,
        );
        list_builder.push_bind(user_id);
        if unread_only {
            list_builder.push(r#" AND un."readAt" IS NULL"#);
        }
        if important_only {
            list_builder.push(r#" AND al.kind::text = ANY("#);
            let kinds: Vec<String> = activity_kind::STAFF_IMPORTANT
                .iter()
                .map(|s| s.to_string())
                .collect();
            list_builder.push_bind(kinds);
            list_builder.push(")");
        }
        list_builder.push(r#" ORDER BY un."createdAt" DESC LIMIT "#);
        list_builder.push_bind(limit);
        list_builder.push(" OFFSET ");
        list_builder.push_bind(offset);

        let items: Vec<NotificationItem> = list_builder.build_query_as().fetch_all(&self.pool).await?;

        Ok(PaginationResult::new(items, total.0, page, limit))
    }

    pub async fn unread_count(&self, user_id: Uuid, important_only: bool) -> Result<i64, AppError> {
        if important_only {
            let kinds: Vec<String> = activity_kind::STAFF_IMPORTANT
                .iter()
                .map(|s| s.to_string())
                .collect();
            let row: (i64,) = sqlx::query_as(
                r#"SELECT COUNT(*) FROM user_notifications un
                   INNER JOIN activity_log al ON al.id = un."activityId"
                   WHERE un."userId" = $1 AND un."readAt" IS NULL
                     AND al.kind::text = ANY($2)"#,
            )
            .bind(user_id)
            .bind(kinds)
            .fetch_one(&self.pool)
            .await?;
            return Ok(row.0);
        }

        let row: (i64,) = sqlx::query_as(
            r#"SELECT COUNT(*) FROM user_notifications
               WHERE "userId" = $1 AND "readAt" IS NULL"#,
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;
        Ok(row.0)
    }

    pub async fn mark_read(&self, notification_id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
        let result = sqlx::query(
            r#"UPDATE user_notifications SET "readAt" = NOW()
               WHERE id = $1 AND "userId" = $2 AND "readAt" IS NULL"#,
        )
        .bind(notification_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() > 0)
    }

    pub async fn mark_all_read(&self, user_id: Uuid) -> Result<i64, AppError> {
        let result = sqlx::query(
            r#"UPDATE user_notifications SET "readAt" = NOW()
               WHERE "userId" = $1 AND "readAt" IS NULL"#,
        )
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected() as i64)
    }

    pub async fn list_activity_log(
        &self,
        user_id: Uuid,
        is_admin: bool,
        filters: &ActivityLogFilterDto,
    ) -> Result<PaginationResult<ActivityLog>, AppError> {
        let page = filters.page.unwrap_or(1).max(1);
        let limit = filters.limit.unwrap_or(20).clamp(1, 100);
        let offset = (page - 1) * limit;

        let mut count_builder = sqlx::QueryBuilder::new("SELECT COUNT(*) FROM activity_log al WHERE 1=1");
        let mut list_builder = sqlx::QueryBuilder::new(
            r#"SELECT al.id, al.kind::text as kind, al.title, al.summary, al.payload,
                      al."actorUserId" as actor_user_id, al."entityType" as entity_type,
                      al."entityId" as entity_id, al."createdAt" as created_at
               FROM activity_log al WHERE 1=1"#,
        );

        if !is_admin {
            let staff_shared: Vec<String> = activity_kind::STAFF_SHARED
                .iter()
                .map(|s| s.to_string())
                .collect();
            count_builder.push(
                r#" AND (
                    EXISTS (
                        SELECT 1 FROM user_notifications un
                        WHERE un."activityId" = al.id AND un."userId" = "#,
            );
            count_builder.push_bind(user_id);
            count_builder.push(
                r#")
                    OR al.kind::text = ANY("#,
            );
            count_builder.push_bind(staff_shared.clone());
            count_builder.push("))");

            list_builder.push(
                r#" AND (
                    EXISTS (
                        SELECT 1 FROM user_notifications un
                        WHERE un."activityId" = al.id AND un."userId" = "#,
            );
            list_builder.push_bind(user_id);
            list_builder.push(
                r#")
                    OR al.kind::text = ANY("#,
            );
            list_builder.push_bind(staff_shared);
            list_builder.push("))");
        }

        if let Some(ref kind) = filters.kind {
            count_builder.push(" AND al.kind::text = ");
            count_builder.push_bind(kind);
            list_builder.push(" AND al.kind::text = ");
            list_builder.push_bind(kind);
        }
        if let Some(actor) = filters.actor_user_id {
            count_builder.push(r#" AND al."actorUserId" = "#);
            count_builder.push_bind(actor);
            list_builder.push(r#" AND al."actorUserId" = "#);
            list_builder.push_bind(actor);
        }
        if let Some(from) = filters.from {
            count_builder.push(r#" AND al."createdAt" >= "#);
            count_builder.push_bind(from);
            list_builder.push(r#" AND al."createdAt" >= "#);
            list_builder.push_bind(from);
        }
        if let Some(to) = filters.to {
            count_builder.push(r#" AND al."createdAt" <= "#);
            count_builder.push_bind(to);
            list_builder.push(r#" AND al."createdAt" <= "#);
            list_builder.push_bind(to);
        }

        let total: (i64,) = count_builder.build_query_as().fetch_one(&self.pool).await?;

        list_builder.push(r#" ORDER BY al."createdAt" DESC LIMIT "#);
        list_builder.push_bind(limit);
        list_builder.push(" OFFSET ");
        list_builder.push_bind(offset);

        let items: Vec<ActivityLog> = list_builder.build_query_as().fetch_all(&self.pool).await?;

        Ok(PaginationResult::new(items, total.0, page, limit))
    }
}
