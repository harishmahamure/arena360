use sqlx::PgPool;
use uuid::Uuid;

use super::channel::ChannelId;
use crate::dto::JwtUserClaims;
use crate::error::AppError;

pub fn can_subscribe(claims: &JwtUserClaims, channel: &ChannelId) -> Result<(), AppError> {
    match channel {
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
            } else {
                Err(AppError::Forbidden(
                    "Cannot subscribe to another user's channel".to_string(),
                ))
            }
        }
        ChannelId::Device(_) => {
            if claims.is_admin_or_staff() {
                Ok(())
            } else {
                Err(AppError::Forbidden(
                    "Only admin or staff can subscribe to device channels".to_string(),
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
        ChannelId::Public | ChannelId::Admin | ChannelId::Staff | ChannelId::Device(_) => {
            Err(AppError::Forbidden(
                "Only the system can publish to this channel".to_string(),
            ))
        }
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

pub async fn is_room_member(pool: &PgPool, room_name: &str, user_id: Uuid) -> Result<bool, AppError> {
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
