use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::PgPool;
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

#[derive(Clone)]
pub struct RoomService {
    pool: PgPool,
}

impl RoomService {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn create(&self, dto: CreateRoomDto, created_by: Uuid) -> Result<Room, AppError> {
        let row: (Uuid, String, Option<String>, Option<Uuid>, DateTime<Utc>) = sqlx::query_as(
            r#"INSERT INTO realtime_rooms (name, description, created_by)
               VALUES ($1, $2, $3)
               RETURNING id, name, description, created_by, created_at"#,
        )
        .bind(&dto.name)
        .bind(&dto.description)
        .bind(created_by)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| match e {
            sqlx::Error::Database(ref db_err)
                if db_err.constraint() == Some("realtime_rooms_name_key") =>
            {
                AppError::Conflict(format!("Room '{}' already exists", dto.name))
            }
            other => AppError::from(other),
        })?;

        Ok(Room {
            id: row.0,
            name: row.1,
            description: row.2,
            created_by: row.3,
            created_at: row.4,
        })
    }

    pub async fn list_for_user(&self, user_id: Uuid) -> Result<Vec<Room>, AppError> {
        let rows: Vec<(Uuid, String, Option<String>, Option<Uuid>, DateTime<Utc>)> =
            sqlx::query_as(
                r#"SELECT r.id, r.name, r.description, r.created_by, r.created_at
               FROM realtime_rooms r
               JOIN realtime_room_members m ON m.room_id = r.id
               WHERE m.user_id = $1
               ORDER BY r.name"#,
            )
            .bind(user_id)
            .fetch_all(&self.pool)
            .await?;

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

    pub async fn add_member(&self, room_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        sqlx::query(
            r#"INSERT INTO realtime_room_members (room_id, user_id)
               VALUES ($1, $2)
               ON CONFLICT (room_id, user_id) DO NOTHING"#,
        )
        .bind(room_id)
        .bind(user_id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn remove_member(&self, room_id: Uuid, user_id: Uuid) -> Result<(), AppError> {
        let result =
            sqlx::query(r#"DELETE FROM realtime_room_members WHERE room_id = $1 AND user_id = $2"#)
                .bind(room_id)
                .bind(user_id)
                .execute(&self.pool)
                .await?;

        if result.rows_affected() == 0 {
            return Err(AppError::NotFound("Member not found in room".to_string()));
        }
        Ok(())
    }

    pub async fn room_exists(&self, room_id: Uuid) -> Result<bool, AppError> {
        let row: Option<(i64,)> =
            sqlx::query_as(r#"SELECT 1::bigint FROM realtime_rooms WHERE id = $1"#)
                .bind(room_id)
                .fetch_optional(&self.pool)
                .await?;
        Ok(row.is_some())
    }
}
