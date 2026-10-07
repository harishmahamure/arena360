use chrono::{DateTime, Utc};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use crate::error::AppError;

#[derive(Debug, Clone, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Room {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Option<Uuid>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, serde::Deserialize, ToSchema)]
pub struct CreateRoomDto {
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, serde::Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AddMemberDto {
    pub user_id: Uuid,
}

/// Rooms and membership belong to the selected tenant database.
#[derive(Clone)]
pub struct TenantRoomService {
    db: std::sync::Arc<crate::tenancy::TenantDb>,
}
impl TenantRoomService {
    pub fn new(db: std::sync::Arc<crate::tenancy::TenantDb>) -> Self {
        Self { db }
    }
    pub async fn create(&self, dto: CreateRoomDto, actor: Uuid) -> Result<Room, AppError> {
        let name = dto.name.trim().to_string();
        if name.is_empty() || name.chars().count() > 120 {
            return Err(AppError::BadRequest(
                "Room name must contain 1 to 120 characters".into(),
            ));
        }
        let room = Room {
            id: Uuid::now_v7(),
            name,
            description: dto.description,
            created_by: Some(actor),
            created_at: Utc::now(),
        };
        let value = serde_json::to_value(&room).map_err(|e| AppError::Internal(e.to_string()))?;
        let id = room.id;
        let name = room.name.clone();
        let description = room.description.clone();
        let at = crate::time::format_sqlite_timestamp(&room.created_at).map_err(|e| AppError::Internal(e.to_string()))?;
        self.db.with_immediate_writer(move |c| Box::pin(async move {
            require_local_user(c,actor).await?;
            sqlx::query("INSERT INTO realtime_rooms(id,name,description,created_by,created_at) VALUES(?,?,?,?,?)")
                .bind(id.to_string()).bind(name).bind(description).bind(actor.to_string()).bind(at).execute(&mut *c).await.map_err(|e|match &e {
                    sqlx::Error::Database(d) if d.is_unique_violation()=>AppError::Conflict("Room name already exists".into()),
                    _=>AppError::from(e)
                })?;
            room_event(c,id,"realtime.room_created",value).await?;
            Ok(())
        })).await?;
        Ok(room)
    }
    pub async fn list_for_user(&self, user: Uuid) -> Result<Vec<Room>, AppError> {
        let rows:Vec<(Uuid,String,Option<String>,Option<Uuid>,DateTime<Utc>)>=sqlx::query_as("SELECT unhex(replace(r.id,'-','')),r.name,r.description,unhex(replace(r.created_by,'-','')),r.created_at FROM realtime_rooms r JOIN realtime_room_members m ON m.room_id=r.id JOIN users u ON u.id=m.user_id WHERE m.user_id=? AND u.is_active=1 AND u.deleted_at IS NULL ORDER BY r.name,r.id")
            .bind(user.to_string()).fetch_all(&self.db.read_pool()?).await?;
        Ok(rows
            .into_iter()
            .map(|r| Room {
                id: r.0,
                name: r.1,
                description: r.2,
                created_by: r.3,
                created_at: r.4,
            })
            .collect())
    }
    pub async fn is_member(&self, room: Uuid, user: Uuid) -> Result<bool, AppError> {
        Ok(sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM realtime_room_members m JOIN users u ON u.id=m.user_id WHERE m.room_id=? AND m.user_id=? AND u.is_active=1 AND u.deleted_at IS NULL)")
            .bind(room.to_string()).bind(user.to_string()).fetch_one(&self.db.read_pool()?).await?)
    }
    pub async fn add_member(&self, room: Uuid, user: Uuid) -> Result<(), AppError> {
        self.db.with_immediate_writer(move |c|Box::pin(async move {
            require_room(c,room).await?; require_local_user(c,user).await?;
            let result=sqlx::query("INSERT INTO realtime_room_members(room_id,user_id) VALUES(?,?) ON CONFLICT DO NOTHING")
                .bind(room.to_string()).bind(user.to_string()).execute(&mut *c).await?;
            if result.rows_affected()>0 {room_event(c,room,"realtime.room_member_added",serde_json::json!({"roomId":room,"userId":user})).await?;}
            Ok(())
        })).await
    }
    pub async fn remove_member(&self, room: Uuid, user: Uuid) -> Result<(), AppError> {
        self.db
            .with_immediate_writer(move |c| {
                Box::pin(async move {
                    require_room(c, room).await?;
                    let result = sqlx::query(
                        "DELETE FROM realtime_room_members WHERE room_id=? AND user_id=?",
                    )
                    .bind(room.to_string())
                    .bind(user.to_string())
                    .execute(&mut *c)
                    .await?;
                    if result.rows_affected() == 0 {
                        return Err(AppError::NotFound("Member not found in room".into()));
                    }
                    room_event(
                        c,
                        room,
                        "realtime.room_member_removed",
                        serde_json::json!({"roomId":room,"userId":user}),
                    )
                    .await?;
                    Ok(())
                })
            })
            .await
    }
}
async fn require_room(c: &mut sqlx::SqliteConnection, id: Uuid) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM realtime_rooms WHERE id=?)")
        .bind(id.to_string())
        .fetch_one(c)
        .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound("Room not found".into()))
    }
}
async fn require_local_user(c: &mut sqlx::SqliteConnection, id: Uuid) -> Result<(), AppError> {
    let exists: bool = sqlx::query_scalar(
        "SELECT EXISTS(SELECT 1 FROM users WHERE id=? AND is_active=1 AND deleted_at IS NULL)",
    )
    .bind(id.to_string())
    .fetch_one(c)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(AppError::NotFound("Active tenant member not found".into()))
    }
}
async fn room_event(
    c: &mut sqlx::SqliteConnection,
    id: Uuid,
    kind: &str,
    payload: serde_json::Value,
) -> Result<(), AppError> {
    crate::tenancy::write_outbox_event_on_connection(
        c,
        crate::tenancy::NewOutboxEvent {
            aggregate_type: "realtime_room".into(),
            aggregate_id: id,
            event_type: kind.into(),
            payload,
            location_id: None,
            schema_version: 1,
            deleted: false,
        },
    )
    .await?;
    Ok(())
}
