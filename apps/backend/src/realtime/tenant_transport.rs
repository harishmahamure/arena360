//! Durable delivery and replay on the owning cell's tenant database.
use super::{acl, channel::ChannelId, outbox::OutboxRow};
use crate::{dto::JwtUserClaims, error::AppError, tenancy::TenantDb};
use chrono::{DateTime, Utc};
use serde_json::Value;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct TenantTransportRow {
    pub event: OutboxRow,
    pub location_id: Option<Uuid>,
}
#[derive(sqlx::FromRow)]
struct StoredRow {
    id: i64,
    channel: String,
    event_type: String,
    payload: String,
    audience_role: Option<String>,
    audience_user_id: Option<Uuid>,
    audience_room_id: Option<Uuid>,
    durable: bool,
    created_at: DateTime<Utc>,
    location_id: Option<Uuid>,
}
const SELECT:&str="SELECT o.id,o.channel,o.event_type,o.payload,o.audience_role,unhex(replace(o.audience_user_id,'-','')) audience_user_id,unhex(replace(o.audience_room_id,'-','')) audience_room_id,o.durable,o.created_at,unhex(replace(o.location_id,'-','')) location_id FROM realtime_outbox o";
impl StoredRow {
    fn into_event(self, tenant: Uuid) -> Result<TenantTransportRow, AppError> {
        let mut payload: Value =
            serde_json::from_str(&self.payload).map_err(|e| AppError::Internal(e.to_string()))?;
        if let Some(fields) = payload.as_object_mut() {
            fields.insert("sourceTenantId".into(), serde_json::json!(tenant));
        }
        Ok(TenantTransportRow {
            event: OutboxRow {
                id: self.id,
                channel: self.channel,
                event_type: self.event_type,
                payload,
                audience_role: self.audience_role,
                audience_user_id: self.audience_user_id,
                audience_room_id: self.audience_room_id,
                durable: self.durable,
                created_at: self.created_at,
                source_tenant_id: Some(tenant),
            },
            location_id: self.location_id,
        })
    }
}
#[derive(Clone)]
pub struct TenantTransport {
    db: Arc<TenantDb>,
}
impl TenantTransport {
    pub fn new(db: Arc<TenantDb>) -> Self {
        Self { db }
    }
    /// A persistent cursor makes projection restartable without deleting analytics events.
    pub async fn project_pending(&self) -> Result<(), AppError> {
        let pool = self.db.background_read_pool()?;
        let cursor: i64 =
            sqlx::query_scalar("SELECT sequence FROM realtime_projection_cursor WHERE singleton=1")
                .fetch_one(&pool)
                .await?;
        let events:Vec<(i64,String,String,Option<String>,String)>=sqlx::query_as("SELECT sequence,event_type,payload,location_id,occurred_at FROM outbox_events WHERE sequence>? ORDER BY sequence LIMIT 100")
            .bind(cursor).fetch_all(&pool).await?;
        for (sequence, kind, payload, location, at) in events {
            let payload = serde_json::from_str(&payload)
                .map_err(|e| AppError::Internal(format!("Invalid tenant outbox payload: {e}")))?;
            let projections = super::dispatcher::tenant_projections(
                self.db.tenant_id(),
                &kind,
                payload,
                Some(pool.clone()),
            )
            .await?;
            self.db.with_immediate_writer(move |c|Box::pin(async move {
                let current:i64=sqlx::query_scalar("SELECT sequence FROM realtime_projection_cursor WHERE singleton=1").fetch_one(&mut *c).await?;
                if current>=sequence {return Ok(());}
                for (index,p) in projections.into_iter().enumerate() {
                    sqlx::query("INSERT INTO realtime_outbox(source_sequence,projection_index,channel,event_type,payload,audience_role,audience_user_id,location_id,durable,created_at) VALUES(?,?,?,?,?,?,?,?,?,?) ON CONFLICT(source_sequence,projection_index) DO NOTHING")
                        .bind(sequence).bind(index as i64).bind(p.channel).bind(p.event_type.unwrap_or_else(||kind.clone()))
                        .bind(serde_json::to_string(&p.payload).map_err(|e|AppError::Internal(e.to_string()))?)
                        .bind(p.audience_role).bind(p.audience_user_id.map(|id|id.to_string())).bind(&location).bind(p.durable).bind(&at).execute(&mut *c).await?;
                }
                sqlx::query("UPDATE realtime_projection_cursor SET sequence=? WHERE singleton=1").bind(sequence).execute(c).await?;
                Ok(())
            })).await?;
        }
        Ok(())
    }
    pub async fn pending(&self) -> Result<Vec<TenantTransportRow>, AppError> {
        let rows: Vec<StoredRow> = sqlx::query_as(&format!(
            "{SELECT} WHERE o.dispatched_at IS NULL ORDER BY o.id LIMIT 100"
        ))
        .fetch_all(&self.db.background_read_pool()?)
        .await?;
        rows.into_iter()
            .map(|r| r.into_event(self.db.tenant_id()))
            .collect()
    }
    pub async fn record_deliveries(&self, id: i64, users: Vec<Uuid>) -> Result<(), AppError> {
        if users.is_empty() {
            return Ok(());
        }
        let at = timestamp(Utc::now())?;
        self.db.with_immediate_writer(move|c|Box::pin(async move {
            for user in users {
                sqlx::query("INSERT INTO realtime_deliveries(outbox_id,subscriber_id,delivered_at) VALUES(?,?,?) ON CONFLICT DO NOTHING")
                    .bind(id).bind(user.to_string()).bind(&at).execute(&mut *c).await?;
            }
            Ok(())
        })).await
    }
    pub async fn mark_dispatched(&self, id: i64) -> Result<(), AppError> {
        let at = timestamp(Utc::now())?;
        self.db.with_immediate_writer(move|c|Box::pin(async move {
            sqlx::query("UPDATE realtime_outbox SET dispatched_at=? WHERE id=? AND dispatched_at IS NULL").bind(at).bind(id).execute(c).await?;
            Ok(())
        })).await
    }
    pub async fn replay(&self, user: Uuid) -> Result<Vec<TenantTransportRow>, AppError> {
        self.replay_after(user, 0).await
    }
    pub async fn replay_after(
        &self,
        user: Uuid,
        after: i64,
    ) -> Result<Vec<TenantTransportRow>, AppError> {
        let rows:Vec<StoredRow>=sqlx::query_as(&format!("{SELECT} JOIN realtime_deliveries d ON d.outbox_id=o.id WHERE d.subscriber_id=? AND d.ack_at IS NULL AND o.id>? ORDER BY o.id LIMIT 500"))
            .bind(user.to_string()).bind(after).fetch_all(&self.db.read_pool()?).await?;
        rows.into_iter()
            .map(|r| r.into_event(self.db.tenant_id()))
            .collect()
    }
    pub async fn ack(&self, id: i64, user: Uuid) -> Result<(), AppError> {
        let at = timestamp(Utc::now())?;
        self.db.with_immediate_writer(move|c|Box::pin(async move {
            sqlx::query("UPDATE realtime_deliveries SET ack_at=? WHERE outbox_id=? AND subscriber_id=? AND ack_at IS NULL")
                .bind(at).bind(id).bind(user.to_string()).execute(c).await?;
            Ok(())
        })).await
    }
    pub async fn cleanup(&self, days: i64) -> Result<u64, AppError> {
        let cutoff = timestamp(Utc::now() - chrono::Duration::days(days))?;
        let ids: Vec<i64> = sqlx::query_scalar(
            "SELECT id FROM realtime_outbox WHERE created_at<? ORDER BY id LIMIT 5000",
        )
        .bind(cutoff)
        .fetch_all(&self.db.background_read_pool()?)
        .await?;
        if ids.is_empty() {
            return Ok(0);
        }
        self.db
            .with_immediate_writer(move |c| {
                Box::pin(async move {
                    let mut count = 0;
                    for id in ids {
                        count += sqlx::query("DELETE FROM realtime_outbox WHERE id=?")
                            .bind(id)
                            .execute(&mut *c)
                            .await?
                            .rows_affected();
                    }
                    Ok(count)
                })
            })
            .await
    }
    pub async fn publish_chat(
        &self,
        claims: &JwtUserClaims,
        channel: String,
        mut payload: Value,
    ) -> Result<(), AppError> {
        let current = current_claims(self.db.clone(), claims).await?;
        let ch = ChannelId::parse(&channel)
            .ok_or_else(|| AppError::BadRequest("Unknown channel".into()))?;
        acl::can_publish(&current, &ch)?;
        check_channel(self.db.clone(), &current, &ch).await?;
        let sender = current
            .user_id_uuid()
            .ok_or_else(|| AppError::Unauthorized("Invalid identity".into()))?;
        let fields = payload
            .as_object_mut()
            .ok_or_else(|| AppError::BadRequest("Chat payload must be an object".into()))?;
        fields.insert("sender_id".into(), serde_json::json!(sender));
        fields.insert(
            "sourceTenantId".into(),
            serde_json::json!(self.db.tenant_id()),
        );
        self.db.with_immediate_writer(move|c|Box::pin(async move {
            // Membership can be removed between request validation and commit.
            if let ChannelId::Room(name)=ch {
                let valid:bool=sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM realtime_room_members m JOIN realtime_rooms r ON r.id=m.room_id JOIN users u ON u.id=m.user_id WHERE r.name=? AND m.user_id=? AND u.is_active=1 AND u.deleted_at IS NULL)")
                    .bind(name).bind(sender.to_string()).fetch_one(&mut *c).await?;
                if !valid {return Err(AppError::Forbidden("Room membership required".into()));}
            }
            crate::tenancy::write_outbox_event_on_connection(c,crate::tenancy::NewOutboxEvent {
                aggregate_type:"realtime_chat".into(),aggregate_id:Uuid::now_v7(),event_type:"realtime.chat_message".into(),
                payload:serde_json::json!({"channel":channel,"message":payload}),location_id:None,schema_version:1,deleted:false
            }).await?;
            Ok(())
        })).await
    }
}
fn timestamp(at: DateTime<Utc>) -> Result<String, AppError> {
    crate::time::format_sqlite_timestamp(&at).map_err(|e| AppError::Internal(e.to_string()))
}

/// Refresh authorization from local state for each live delivery and replay.
pub async fn current_claims(
    db: Arc<TenantDb>,
    claims: &JwtUserClaims,
) -> Result<JwtUserClaims, AppError> {
    db.ensure_current_owner()?;
    if claims.tenantId != db.tenant_id().to_string()
        || claims.exp.is_none_or(|exp| exp <= Utc::now().timestamp())
    {
        return Err(AppError::Unauthorized(
            "Session expired or tenant changed".into(),
        ));
    }
    let mut current = claims.clone();
    let user = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid identity".into()))?;
    if claims.is_admin_or_staff() {
        let repo = crate::repositories::TenantSettingsRepository::new(db);
        let membership = repo
            .membership_context(user)
            .await?
            .filter(|m| claims.roles.contains(&m.role))
            .ok_or_else(|| AppError::Unauthorized("Tenant membership changed".into()))?;
        current.permissions = repo.effective_permissions(user).await?;
        current.roles = vec![membership.role];
    } else if claims.is_device() {
        if claims.device_id_uuid() != Some(user) {
            return Err(AppError::Unauthorized("Device identity mismatch".into()));
        }
        current.permissions.clear();
        current.roles = vec!["device".into()];
        let device = crate::repositories::TenantDeviceRepository::new(db)
            .find_by_id(user)
            .await?
            .ok_or_else(|| AppError::Unauthorized("Device no longer registered".into()))?;
        if claims.locationId != Some(device.location_id.to_string())
            || device.registration_status != "registered"
        {
            return Err(AppError::Unauthorized("Device registration changed".into()));
        }
    } else {
        current.permissions.clear();
        current.roles = vec!["player".into()];
        let local = crate::repositories::TenantUserRepository::new(db)
            .find_by_id(user)
            .await?
            .filter(|u| u.is_active && u.role.as_deref() == Some("player"))
            .ok_or_else(|| AppError::Unauthorized("Player access changed".into()))?;
        if !claims.roles.iter().any(|r| r == "player") || local.id != user {
            return Err(AppError::Unauthorized("Player identity required".into()));
        }
    }
    Ok(current)
}
pub async fn check_channel(
    db: Arc<TenantDb>,
    claims: &JwtUserClaims,
    ch: &ChannelId,
) -> Result<(), AppError> {
    acl::can_subscribe(claims, ch)?;
    let user = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid identity".into()))?;
    let pool = db.read_pool()?;
    let allowed=match ch {
        ChannelId::Room(name)=>sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM realtime_room_members m JOIN realtime_rooms r ON r.id=m.room_id JOIN users u ON u.id=m.user_id WHERE r.name=? AND m.user_id=? AND u.is_active=1 AND u.deleted_at IS NULL)")
            .bind(name).bind(user.to_string()).fetch_one(&pool).await?,
        ChannelId::User(player) if claims.is_device()=>sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM usage_sessions WHERE device_id=? AND player_id=? AND end_time IS NULL AND deleted_at IS NULL)")
            .bind(user.to_string()).bind(player.to_string()).fetch_one(&pool).await?,
        ChannelId::Device(id) if claims.is_admin_or_staff()=>{
            let device=crate::repositories::TenantDeviceRepository::new(db.clone()).find_by_id(*id).await?
                .ok_or_else(||AppError::NotFound("Device not found".into()))?;
            crate::repositories::TenantSettingsRepository::new(db.clone()).ensure_location_permission(db.tenant_id(),device.location_id,user,"devices:read").await?;true
        },
        _=>true,
    };
    if allowed {
        Ok(())
    } else {
        Err(AppError::Forbidden(
            "Current channel membership required".into(),
        ))
    }
}
pub async fn can_receive(
    db: Arc<TenantDb>,
    claims: &JwtUserClaims,
    row: &TenantTransportRow,
) -> Result<bool, AppError> {
    let claims = match current_claims(db.clone(), claims).await {
        Ok(claims) => claims,
        Err(error) if denied(&error) => return Ok(false),
        Err(error) => return Err(error),
    };
    let Some(channel) = ChannelId::parse(&row.event.channel) else {
        return Ok(false);
    };
    if !acl::event_matches_claims(&claims, &row.event) {
        return Ok(false);
    }
    if let Err(error) = check_channel(db.clone(), &claims, &channel).await {
        return if denied(&error) {
            Ok(false)
        } else {
            Err(error)
        };
    }
    if row.event.event_type == "notification.created"
        && row
            .event
            .payload
            .get("kind")
            .and_then(serde_json::Value::as_str)
            == Some("kiosk_order_placed")
        && row.location_id.is_none()
    {
        return Ok(false);
    }
    if claims.is_admin_or_staff() {
        if let Some(location) = row.location_id {
            let permission = match channel {
                ChannelId::Admin => Some("events:admin"),
                ChannelId::Staff => Some("events:staff"),
                ChannelId::Kitchen => Some("kitchen:read"),
                ChannelId::Device(_) => Some("devices:read"),
                ChannelId::User(_) if row.event.event_type == "notification.created" => {
                    Some("notifications:read")
                }
                ChannelId::Configuration => {
                    Some(if row.event.event_type == "pricing.rules.changed" {
                        "rules:read"
                    } else {
                        "settings:read"
                    })
                }
                _ => None,
            };
            if let Some(permission) = permission {
                if let Err(error) = crate::repositories::TenantSettingsRepository::new(db.clone())
                    .ensure_location_permission(
                        db.tenant_id(),
                        location,
                        claims.user_id_uuid().unwrap(),
                        permission,
                    )
                    .await
                {
                    return if denied(&error) {
                        Ok(false)
                    } else {
                        Err(error)
                    };
                }
            }
        }
    }
    Ok(true)
}

fn denied(error: &AppError) -> bool {
    matches!(
        error,
        AppError::Unauthorized(_) | AppError::Forbidden(_) | AppError::NotFound(_)
    ) || matches!(error,AppError::Api{status,..} if matches!(status.as_u16(),401|403|404))
}
