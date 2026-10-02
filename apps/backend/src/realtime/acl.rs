use sqlx::PgPool;
use uuid::Uuid;

use super::channel::ChannelId;
use crate::dto::JwtUserClaims;
use crate::error::AppError;

pub fn can_subscribe(claims: &JwtUserClaims, channel: &ChannelId) -> Result<(), AppError> {
    match channel {
        ChannelId::Configuration => {
            if claims.is_admin_or_staff()
                && claims
                    .permissions
                    .iter()
                    .any(|p| p == "settings:read" || p == "rules:read")
            {
                Ok(())
            } else {
                Err(AppError::Forbidden(
                    "Configuration read access required".into(),
                ))
            }
        }
        ChannelId::Public => Ok(()),
        ChannelId::Admin => {
            if claims.is_admin() {
                Ok(())
            } else {
                Err(AppError::Forbidden(
                    "Only admins can subscribe to the admin channel".to_string(),
                ))
            }
        }
        ChannelId::Staff => {
            if claims.is_admin_or_staff() {
                Ok(())
            } else {
                Err(AppError::Forbidden(
                    "Only admin or staff can subscribe to the staff channel".to_string(),
                ))
            }
        }
        ChannelId::User(user_id) => {
            let caller_id = claims.user_id_uuid();
            if caller_id == Some(*user_id) || claims.is_admin() {
                Ok(())
            } else if claims.is_device() {
                // DB session check happens in connection handler (async)
                Ok(())
            } else {
                Err(AppError::Forbidden(
                    "Cannot subscribe to another user's channel".to_string(),
                ))
            }
        }
        ChannelId::Device(device_id) => {
            if claims.is_admin_or_staff() {
                Ok(())
            } else if claims.is_device() {
                let own_id = claims.device_id_uuid();
                if own_id == Some(*device_id) {
                    Ok(())
                } else {
                    Err(AppError::Forbidden(
                        "Device token can only subscribe to its own device channel".to_string(),
                    ))
                }
            } else {
                Err(AppError::Forbidden(
                    "Cannot subscribe to device channel".to_string(),
                ))
            }
        }
        ChannelId::Room(_) => {
            // Room membership is checked at the DB level in the handler
            Ok(())
        }
    }
}

pub fn can_publish(claims: &JwtUserClaims, channel: &ChannelId) -> Result<(), AppError> {
    match channel {
        ChannelId::Configuration
        | ChannelId::Public
        | ChannelId::Admin
        | ChannelId::Staff
        | ChannelId::Device(_) => Err(AppError::Forbidden(
            "Only the system can publish to this channel".to_string(),
        )),
        ChannelId::User(user_id) => {
            let caller_id = claims.user_id_uuid();
            if caller_id == Some(*user_id) || claims.is_admin() {
                Ok(())
            } else {
                Err(AppError::Forbidden(
                    "Cannot publish to another user's channel without admin role".to_string(),
                ))
            }
        }
        ChannelId::Room(_) => {
            // Room membership is checked at the DB level
            Ok(())
        }
    }
}

pub async fn device_has_player_session(
    pool: &PgPool,
    device_id: Uuid,
    player_id: Uuid,
) -> Result<bool, AppError> {
    let row: Option<(i64,)> = sqlx::query_as(
        r#"
        SELECT 1::bigint
        FROM usage_sessions s
        INNER JOIN player_plan_balances b ON b.id = s."balanceId" AND b."deletedAt" IS NULL
        WHERE s."deviceId" = $1
          AND b."playerId" = $2
          AND s."endTime" IS NULL
          AND s."deletedAt" IS NULL
        LIMIT 1
        "#,
    )
    .bind(device_id)
    .bind(player_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
}

pub async fn is_room_member(
    pool: &PgPool,
    room_name: &str,
    user_id: Uuid,
) -> Result<bool, AppError> {
    let row: Option<(i64,)> = sqlx::query_as(
        r#"SELECT 1::bigint FROM realtime_room_members rm
           JOIN realtime_rooms r ON r.id = rm.room_id
           WHERE r.name = $1 AND rm.user_id = $2"#,
    )
    .bind(room_name)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.is_some())
}

/// Apply audience restrictions to both live delivery and durable replay.
pub fn event_matches_claims(claims: &JwtUserClaims, row: &super::outbox::OutboxRow) -> bool {
    if row
        .audience_role
        .as_ref()
        .is_some_and(|role| !claims.roles.contains(role))
    {
        return false;
    }
    if row
        .audience_user_id
        .is_some_and(|id| claims.user_id_uuid() != Some(id))
    {
        return false;
    }
    if row.channel == "configuration" {
        let permission = if row.event_type == "pricing.rules.changed" {
            "rules:read"
        } else {
            "settings:read"
        };
        return claims.is_admin_or_staff()
            && row.payload.get("organizationId").and_then(|id| id.as_str())
                == Some(claims.tenantId.as_str())
            && claims.permissions.iter().any(|p| p == permission);
    }
    true
}

#[cfg(test)]
mod configuration_tests {
    use super::*;
    use crate::realtime::outbox::OutboxRow;
    fn claims() -> JwtUserClaims {
        serde_json::from_value(serde_json::json!({
            "sub": "00000000-0000-4000-8000-000000000001", "userId": "00000000-0000-4000-8000-000000000001",
            "roles": ["staff"], "tenantId": "org-one", "orgIds": ["org-one"], "allowedTenants": ["org-one"],
            "permissions": ["settings:read"], "iss": "gamezone", "aud": "gamezone", "appId": "test"
        })).unwrap()
    }
    fn event() -> OutboxRow {
        OutboxRow {
            id: 1,
            channel: "configuration".into(),
            event_type: "configuration.changed".into(),
            payload: serde_json::json!({"organizationId": "org-one"}),
            audience_role: None,
            audience_user_id: None,
            audience_room_id: None,
            durable: true,
            created_at: chrono::Utc::now(),
        }
    }
    #[test]
    fn configuration_requires_read_access_and_cannot_be_client_published() {
        let mut claims = claims();
        assert!(can_subscribe(&claims, &ChannelId::Configuration).is_ok());
        assert!(can_publish(&claims, &ChannelId::Configuration).is_err());
        claims.permissions.clear();
        assert!(can_subscribe(&claims, &ChannelId::Configuration).is_err());
    }
    #[test]
    fn live_and_replayed_events_enforce_organization_role_and_permission() {
        let claims = claims();
        let mut row = event();
        assert!(event_matches_claims(&claims, &row));
        row.payload = serde_json::json!({"organizationId": "org-other"});
        assert!(!event_matches_claims(&claims, &row));
        row = event();
        row.event_type = "pricing.rules.changed".into();
        assert!(!event_matches_claims(&claims, &row));
        row = event();
        row.audience_role = Some("admin".into());
        assert!(!event_matches_claims(&claims, &row));
        row = event();
        row.payload = serde_json::json!({});
        assert!(!event_matches_claims(&claims, &row));
    }
}
