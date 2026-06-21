use serde_json::{json, Value};
use std::sync::Arc;
use uuid::Uuid;

use crate::cache::{get_or_set, keys, CacheService};
use crate::dto::PaginationResult;
use crate::error::AppError;
use crate::models::{
    ActivityLog, ActivityLogFilterDto, NotificationFilterDto, NotificationItem, Transaction,
    UnreadCountDto,
};
use crate::realtime::OutboxService;
use crate::repositories::NotificationRepository;

#[derive(Debug, Clone)]
pub enum Recipients {
    AllAdmins,
    AllStaff,
    Users(Vec<Uuid>),
    /// Admins plus specific users (e.g. approval requester).
    AdminAndUsers(Vec<Uuid>),
}

#[derive(Debug, Clone)]
pub struct RecordNotification {
    pub kind: String,
    pub title: String,
    pub summary: Option<String>,
    pub payload: Value,
    pub actor_user_id: Option<Uuid>,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub recipients: Recipients,
}

#[derive(Clone)]
pub struct NotificationService {
    repo: NotificationRepository,
    outbox: OutboxService,
    cache: Arc<dyn CacheService>,
}

impl NotificationService {
    pub fn new(pool: sqlx::PgPool, outbox: OutboxService, cache: Arc<dyn CacheService>) -> Self {
        Self {
            repo: NotificationRepository::new(pool),
            outbox,
            cache,
        }
    }

    async fn resolve_recipients(&self, recipients: Recipients) -> Result<Vec<Uuid>, AppError> {
        match recipients {
            Recipients::AllAdmins => self.repo.list_admin_user_ids().await,
            Recipients::AllStaff => self.repo.list_staff_user_ids().await,
            Recipients::Users(ids) => Ok(ids),
            Recipients::AdminAndUsers(mut ids) => {
                let mut admins = self.repo.list_admin_user_ids().await?;
                ids.append(&mut admins);
                ids.sort_unstable();
                ids.dedup();
                Ok(ids)
            }
        }
    }

    async fn invalidate_user_cache(&self, user_id: Uuid) -> Result<(), AppError> {
        self.cache
            .invalidate_prefix(&keys::notifications_user_prefix(&user_id))
            .await
    }

    async fn invalidate_recipients_cache(&self, user_ids: &[Uuid]) -> Result<(), AppError> {
        for user_id in user_ids {
            self.invalidate_user_cache(*user_id).await?;
        }
        Ok(())
    }

    pub async fn record(&self, input: RecordNotification) -> Result<ActivityLog, AppError> {
        let activity = self
            .repo
            .insert_activity(
                &input.kind,
                &input.title,
                input.summary.as_deref(),
                &input.payload,
                input.actor_user_id,
                input.entity_type.as_deref(),
                input.entity_id,
            )
            .await?;

        let user_ids = self.resolve_recipients(input.recipients).await?;
        let notification_ids = self
            .repo
            .insert_user_notifications(activity.id, &user_ids)
            .await?;

        self.invalidate_recipients_cache(&user_ids).await?;

        for (user_id, notification_id) in user_ids.iter().zip(notification_ids.iter()) {
            let ws_payload = json!({
                "notificationId": notification_id.to_string(),
                "activityId": activity.id.to_string(),
                "kind": activity.kind,
                "title": activity.title,
                "summary": activity.summary,
                "payload": activity.payload,
                "entityType": activity.entity_type,
                "entityId": activity.entity_id.map(|id| id.to_string()),
                "createdAt": activity.created_at.to_rfc3339(),
            });
            let channel = format!("user:{user_id}");
            let _ = self
                .outbox
                .publish(
                    &channel,
                    "notification.created",
                    ws_payload,
                    None,
                    Some(*user_id),
                    true,
                )
                .await;
        }

        Ok(activity)
    }

    pub async fn list_notifications(
        &self,
        user_id: Uuid,
        filters: NotificationFilterDto,
    ) -> Result<PaginationResult<NotificationItem>, AppError> {
        let cache_key = keys::notifications_inbox(&user_id, &keys::filter_hash(&filters));
        get_or_set(
            &*self.cache,
            &cache_key,
            keys::ttl::NOTIFICATIONS,
            || async {
                self.repo.list_notifications(user_id, &filters).await
            },
        )
        .await
    }

    pub async fn unread_count(&self, user_id: Uuid) -> Result<UnreadCountDto, AppError> {
        let cache_key = keys::notifications_unread(&user_id);
        get_or_set(
            &*self.cache,
            &cache_key,
            keys::ttl::NOTIFICATIONS,
            || async {
                let count = self.repo.unread_count(user_id).await?;
                Ok(UnreadCountDto { count })
            },
        )
        .await
    }

    pub async fn mark_read(&self, notification_id: Uuid, user_id: Uuid) -> Result<bool, AppError> {
        let updated = self.repo.mark_read(notification_id, user_id).await?;
        if updated {
            self.invalidate_user_cache(user_id).await?;
        }
        Ok(updated)
    }

    pub async fn mark_all_read(&self, user_id: Uuid) -> Result<i64, AppError> {
        let count = self.repo.mark_all_read(user_id).await?;
        if count > 0 {
            self.invalidate_user_cache(user_id).await?;
        }
        Ok(count)
    }

    pub async fn list_activity_log(
        &self,
        user_id: Uuid,
        is_admin: bool,
        filters: ActivityLogFilterDto,
    ) -> Result<PaginationResult<ActivityLog>, AppError> {
        self.repo
            .list_activity_log(user_id, is_admin, &filters)
            .await
    }

    pub async fn record_approval_requested(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        title: &str,
        payload: Value,
        actor_user_id: Option<Uuid>,
    ) -> Result<ActivityLog, AppError> {
        self.record(RecordNotification {
            kind: crate::models::activity_kind::APPROVAL_REQUESTED.to_string(),
            title: title.to_string(),
            summary: Some("Awaiting admin approval".to_string()),
            payload,
            actor_user_id,
            entity_type: Some(entity_type.to_string()),
            entity_id: Some(entity_id),
            recipients: Recipients::AllAdmins,
        })
        .await
    }

    pub async fn record_approval_decided(
        &self,
        entity_type: &str,
        entity_id: Uuid,
        status: &str,
        title: &str,
        payload: Value,
        requester_id: Uuid,
        actor_user_id: Option<Uuid>,
    ) -> Result<ActivityLog, AppError> {
        self.record(RecordNotification {
            kind: crate::models::activity_kind::APPROVAL_DECIDED.to_string(),
            title: title.to_string(),
            summary: Some(format!("Status: {status}")),
            payload,
            actor_user_id,
            entity_type: Some(entity_type.to_string()),
            entity_id: Some(entity_id),
            recipients: Recipients::Users(vec![requester_id]),
        })
        .await
    }

    pub async fn build_staff_sale_payload(
        &self,
        transaction: &Transaction,
        actor_id: Uuid,
    ) -> Result<Value, AppError> {
        let staff_name = self.repo.user_display_name(actor_id).await?;
        let customer_name = self.repo.user_display_name(transaction.player_id).await?;
        let payment_label = payment_method_label(&transaction.payment_method);

        Ok(json!({
            "transactionId": transaction.id.to_string(),
            "amount": transaction.amount,
            "paymentMethod": transaction.payment_method,
            "paymentLabel": payment_label,
            "transactionType": transaction.transaction_type,
            "staffId": actor_id.to_string(),
            "staffName": staff_name,
            "customerId": transaction.player_id.to_string(),
            "customerName": customer_name,
        }))
    }

    pub async fn record_staff_sale(
        &self,
        transaction: &Transaction,
        actor_id: Uuid,
    ) -> Result<ActivityLog, AppError> {
        let payload = self.build_staff_sale_payload(transaction, actor_id).await?;
        let staff_name = payload
            .get("staffName")
            .and_then(|v| v.as_str())
            .unwrap_or("Staff");
        let customer_name = payload
            .get("customerName")
            .and_then(|v| v.as_str())
            .unwrap_or("Customer");
        let payment_label = payload
            .get("paymentLabel")
            .and_then(|v| v.as_str())
            .unwrap_or("Payment");

        let transaction_type = transaction.transaction_type.as_str();
        let kind = if transaction_type == "plan_purchase" {
            crate::models::activity_kind::PLAN_SALE
        } else {
            crate::models::activity_kind::TRANSACTION_SALE
        };

        let sale_label = if transaction_type == "plan_purchase" {
            "Plan sale"
        } else {
            "Product sale"
        };

        self.record(RecordNotification {
            kind: kind.to_string(),
            title: format!("{sale_label} · ₹{:.2}", transaction.amount),
            summary: Some(format!(
                "Sold by {staff_name} to {customer_name} · {payment_label}"
            )),
            payload,
            actor_user_id: Some(actor_id),
            entity_type: Some("transaction".to_string()),
            entity_id: Some(transaction.id),
            recipients: Recipients::AdminAndUsers(vec![actor_id]),
        })
        .await
    }
}

fn payment_method_label(method: &str) -> &'static str {
    match method {
        "cash" => "Cash",
        "online" => "Online",
        "split_payment" => "Split payment",
        "credit" => "Credit",
        _ => "Payment",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recipients_users_passthrough() {
        let ids = vec![Uuid::new_v4()];
        match Recipients::Users(ids.clone()) {
            Recipients::Users(v) => assert_eq!(v, ids),
            _ => panic!("expected Users variant"),
        }
    }

    #[test]
    fn payment_method_labels() {
        assert_eq!(payment_method_label("cash"), "Cash");
        assert_eq!(payment_method_label("credit"), "Credit");
    }
}
