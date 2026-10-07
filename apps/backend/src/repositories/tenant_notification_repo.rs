//! Tenant-local activity and inbox operations; writes are lease fenced and atomic.
use super::tenant_back_office::{event, now, write};
use crate::{
    dto::PaginationResult,
    error::AppError,
    models::*,
    services::notification_service::{Recipients, RecordNotification},
    tenancy::{format_sqlite_timestamp, TenantDb},
};
use serde_json::json;
use sqlx::{QueryBuilder, Sqlite, SqliteConnection};
use std::sync::Arc;
use uuid::Uuid;
#[derive(Clone)]
pub struct TenantNotificationRepository {
    db: Arc<TenantDb>,
}
const ACTIVITY: &str = "SELECT unhex(replace(al.id,'-','')) AS id,al.kind,al.title,al.summary,al.payload,unhex(replace(al.actor_user_id,'-','')) AS actor_user_id,al.entity_type,unhex(replace(al.entity_id,'-','')) AS entity_id,al.created_at FROM activity_log al";
const INBOX: &str = "SELECT unhex(replace(un.id,'-','')) AS id,unhex(replace(un.activity_id,'-','')) AS activity_id,al.kind,al.title,al.summary,al.payload,unhex(replace(al.actor_user_id,'-','')) AS actor_user_id,al.entity_type,unhex(replace(al.entity_id,'-','')) AS entity_id,un.read_at,un.created_at FROM user_notifications un JOIN activity_log al ON al.id=un.activity_id";
impl TenantNotificationRepository {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    pub(crate) async fn record_on(
        c: &mut SqliteConnection,
        input: RecordNotification,
    ) -> Result<ActivityLog, AppError> {
        let id = Uuid::now_v7();
        let at = now()?;
        sqlx::query("INSERT INTO activity_log(id,kind,title,summary,payload,actor_user_id,entity_type,entity_id,created_at) VALUES(?,?,?,?,?,?,?,?,?)").bind(id.to_string()).bind(&input.kind).bind(&input.title).bind(&input.summary).bind(input.payload.to_string()).bind(input.actor_user_id.map(|x| x.to_string())).bind(&input.entity_type).bind(input.entity_id.map(|x| x.to_string())).bind(&at).execute(&mut *c).await?;
        let row: ActivityLog = sqlx::query_as(&format!("{ACTIVITY} WHERE al.id=?"))
            .bind(id.to_string())
            .fetch_one(&mut *c)
            .await?;
        if input.kind == activity_kind::KIOSK_ORDER_PLACED
            && matches!(input.recipients, Recipients::AllStaff)
        {
            let users: Vec<String> = sqlx::query_scalar("SELECT id FROM users WHERE role='staff' AND is_active=1 AND deleted_at IS NULL ORDER BY id").fetch_all(&mut *c).await?;
            for user in users {
                let notification = Uuid::now_v7();
                sqlx::query("INSERT INTO user_notifications(id,activity_id,user_id,created_at) VALUES(?,?,?,?)").bind(notification.to_string()).bind(id.to_string()).bind(&user).bind(&at).execute(&mut *c).await?;
                event(c, "notification", notification, "notification.created", None, false, json!({"userId":user,"notificationId":notification,"activityId":id,"kind":row.kind,"title":row.title,"summary":row.summary,"payload":row.payload,"entityType":row.entity_type,"entityId":row.entity_id,"createdAt":at})).await?;
            }
        }
        Ok(row)
    }
    pub async fn record(&self, input: RecordNotification) -> Result<ActivityLog, AppError> {
        write(
            &self.db,
            Box::new(move |c| Box::pin(async move { Self::record_on(c, input).await })),
        )
        .await
    }
    pub async fn record_activity(
        &self,
        mut input: RecordNotification,
    ) -> Result<ActivityLog, AppError> {
        input.recipients = Recipients::Users(vec![]);
        self.record(input).await
    }
    pub async fn list_staff_user_ids(&self) -> Result<Vec<Uuid>, AppError> {
        Ok(sqlx::query_scalar("SELECT unhex(replace(id,'-','')) FROM users WHERE role='staff' AND is_active=1 AND deleted_at IS NULL ORDER BY id").fetch_all(&self.db.read_pool()?).await?)
    }
    pub async fn user_display_name(&self, id: Uuid) -> Result<String, AppError> {
        let row: Option<(Option<String>, Option<String>, String)> = sqlx::query_as(
            "SELECT first_name,last_name,username FROM users WHERE id=? AND deleted_at IS NULL",
        )
        .bind(id.to_string())
        .fetch_optional(&self.db.read_pool()?)
        .await?;
        Ok(row
            .map(|(f, l, u)| {
                let name = format!("{} {}", f.unwrap_or_default(), l.unwrap_or_default())
                    .trim()
                    .to_owned();
                if name.is_empty() {
                    u
                } else {
                    name
                }
            })
            .unwrap_or_else(|| "Unknown".into()))
    }
    fn inbox_where(b: &mut QueryBuilder<'_, Sqlite>, user: Uuid, unread: bool, cutoff: String) {
        b.push(" WHERE un.user_id=")
            .push_bind(user.to_string())
            .push(" AND al.kind='kiosk_order_placed' AND un.created_at>=")
            .push_bind(cutoff);
        if unread {
            b.push(" AND un.read_at IS NULL");
        }
    }
    fn cutoff(days: i64) -> Result<String, AppError> {
        let d = chrono::Duration::try_days(days)
            .ok_or_else(|| AppError::BadRequest("Retention duration is out of range".into()))?;
        let date = chrono::Utc::now()
            .checked_sub_signed(d)
            .ok_or_else(|| AppError::BadRequest("Retention duration is out of range".into()))?;
        format_sqlite_timestamp(&date).map_err(|e| AppError::Internal(e.to_string()))
    }
    pub async fn list_notifications(
        &self,
        user: Uuid,
        f: &NotificationFilterDto,
    ) -> Result<PaginationResult<NotificationItem>, AppError> {
        let page = f.page.unwrap_or(1).max(1);
        let limit = f
            .limit
            .unwrap_or(crate::cache::keys::MAX_INBOX_NOTIFICATIONS)
            .clamp(1, crate::cache::keys::MAX_INBOX_NOTIFICATIONS);
        let cutoff = Self::cutoff(7)?;
        let mut count = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM user_notifications un JOIN activity_log al ON al.id=un.activity_id");
        Self::inbox_where(
            &mut count,
            user,
            f.unread_only.unwrap_or(false),
            cutoff.clone(),
        );
        let total: i64 = count
            .build_query_scalar()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        let mut list = QueryBuilder::<Sqlite>::new(INBOX);
        Self::inbox_where(&mut list, user, f.unread_only.unwrap_or(false), cutoff);
        list.push(" ORDER BY un.created_at DESC,un.id DESC LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind((page - 1).saturating_mul(limit));
        let items = list
            .build_query_as()
            .fetch_all(&self.db.read_pool()?)
            .await?;
        Ok(PaginationResult::new(items, total, page, limit))
    }
    pub async fn unread_count(&self, user: Uuid, _important: bool) -> Result<i64, AppError> {
        let mut b = QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM user_notifications un JOIN activity_log al ON al.id=un.activity_id");
        Self::inbox_where(&mut b, user, true, Self::cutoff(7)?);
        Ok(b.build_query_scalar()
            .fetch_one(&self.db.read_pool()?)
            .await?)
    }
    pub async fn mark_read(&self, id: Uuid, user: Uuid) -> Result<bool, AppError> {
        Ok(self.mark(Some(id), user).await? > 0)
    }
    pub async fn mark_all_read(&self, user: Uuid) -> Result<i64, AppError> {
        self.mark(None, user).await
    }
    async fn mark(&self, id: Option<Uuid>, user: Uuid) -> Result<i64, AppError> {
        let cutoff = Self::cutoff(7)?;
        write(
            &self.db,
            Box::new(move |c| {
                Box::pin(async move {
                    let mut b = QueryBuilder::<Sqlite>::new("UPDATE user_notifications SET read_at=");
                    b.push_bind(now()?).push(" WHERE user_id=").push_bind(user.to_string()).push(" AND read_at IS NULL AND created_at>=").push_bind(cutoff).push(" AND EXISTS(SELECT 1 FROM activity_log al WHERE al.id=user_notifications.activity_id AND al.kind='kiosk_order_placed')");
                    if let Some(id) = id {
                        b.push(" AND id=").push_bind(id.to_string());
                    }
                    Ok(b.build().execute(c).await?.rows_affected() as i64)
                })
            }),
        )
        .await
    }
    pub async fn cleanup_notifications(&self, days: i64) -> Result<u64, AppError> {
        if days < 0 {
            return Err(AppError::BadRequest(
                "Retention days must be nonnegative".into(),
            ));
        }
        let cutoff = Self::cutoff(days)?;
        write(&self.db, Box::new(move |c| Box::pin(async move { Ok(sqlx::query("DELETE FROM user_notifications WHERE id IN(SELECT un.id FROM user_notifications un JOIN activity_log al ON al.id=un.activity_id JOIN users u ON u.id=un.user_id WHERE al.kind<>'kiosk_order_placed' OR u.role<>'staff' OR u.is_active=0 OR u.deleted_at IS NOT NULL OR un.created_at<? ORDER BY un.id LIMIT 5000)").bind(cutoff).execute(c).await?.rows_affected()) }))).await
    }
    pub async fn list_activity_log(
        &self,
        user: Uuid,
        admin: bool,
        f: &ActivityLogFilterDto,
    ) -> Result<PaginationResult<ActivityLog>, AppError> {
        let page = f.page.unwrap_or(1).max(1);
        let limit = f.limit.unwrap_or(20).clamp(1, 100);
        let mut count =
            QueryBuilder::<Sqlite>::new("SELECT COUNT(*) FROM activity_log al WHERE 1=1");
        let mut list = QueryBuilder::<Sqlite>::new(format!("{ACTIVITY} WHERE 1=1"));
        for b in [&mut count, &mut list] {
            if !admin {
                b.push(" AND (EXISTS(SELECT 1 FROM user_notifications un WHERE un.activity_id=al.id AND un.user_id=").push_bind(user.to_string()).push(") OR al.kind IN(");
                let mut sep = b.separated(",");
                for kind in activity_kind::STAFF_SHARED {
                    sep.push_bind(*kind);
                }
                sep.push_unseparated("))");
            }
            if let Some(kind) = &f.kind {
                b.push(" AND al.kind=").push_bind(kind.clone());
            }
            if let Some(actor) = f.actor_user_id {
                b.push(" AND al.actor_user_id=")
                    .push_bind(actor.to_string());
            }
            if let Some(from) = f.from {
                b.push(" AND al.created_at>=").push_bind(
                    format_sqlite_timestamp(&from)
                        .map_err(|e| AppError::BadRequest(e.to_string()))?,
                );
            }
            if let Some(to) = f.to {
                b.push(" AND al.created_at<=").push_bind(
                    format_sqlite_timestamp(&to)
                        .map_err(|e| AppError::BadRequest(e.to_string()))?,
                );
            }
        }
        let total: i64 = count
            .build_query_scalar()
            .fetch_one(&self.db.read_pool()?)
            .await?;
        list.push(" ORDER BY al.created_at DESC,al.id DESC LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind((page - 1).saturating_mul(limit));
        Ok(PaginationResult::new(
            list.build_query_as()
                .fetch_all(&self.db.read_pool()?)
                .await?,
            total,
            page,
            limit,
        ))
    }
}
