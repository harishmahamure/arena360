use axum::{
    extract::{Path, State},
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::middleware::{AdminUser, AuthUser};
use crate::realtime::rooms::{AddMemberDto, CreateRoomDto, Room};

#[utoipa::path(
    get,
    path = "/realtime/rooms",
    responses(
        (status = 200, description = "List rooms the caller belongs to", body = Vec<Room>),
        (status = 401, description = "Unauthorized"),
    ),
    security(("bearer_auth" = [])),
    tag = "realtime"
)]
pub async fn list_rooms(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult<Vec<Room>> {
    let user_id = claims
        .user_id_uuid()
        .ok_or_else(|| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let rooms = state.rooms.list_for_user(user_id).await?;
    ok(rooms)
}

#[utoipa::path(
    post,
    path = "/realtime/rooms",
    request_body = CreateRoomDto,
    responses(
        (status = 201, description = "Room created", body = Room),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden – admin only"),
        (status = 409, description = "Room name conflict"),
    ),
    security(("bearer_auth" = [])),
    tag = "realtime"
)]
pub async fn create_room(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateRoomDto>,
) -> ApiResult<Room> {
    let admin_id = claims
        .user_id_uuid()
        .ok_or_else(|| crate::error::AppError::BadRequest("Invalid user ID".to_string()))?;
    let room = state.rooms.create(dto, admin_id).await?;
    created(room)
}

#[utoipa::path(
    post,
    path = "/realtime/rooms/{id}/members",
    request_body = AddMemberDto,
    params(
        ("id" = Uuid, Path, description = "Room ID"),
    ),
    responses(
        (status = 200, description = "Member added"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden – admin only"),
        (status = 404, description = "Room not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "realtime"
)]
pub async fn add_member(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(room_id): Path<Uuid>,
    Json(dto): Json<AddMemberDto>,
) -> ApiResult<serde_json::Value> {
    if !state.rooms.room_exists(room_id).await? {
        return Err(crate::error::AppError::NotFound(
            "Room not found".to_string(),
        ));
    }
    state.rooms.add_member(room_id, dto.user_id).await?;
    ok(serde_json::json!({ "message": "Member added" }))
}

#[utoipa::path(
    delete,
    path = "/realtime/rooms/{id}/members/{user_id}",
    params(
        ("id" = Uuid, Path, description = "Room ID"),
        ("user_id" = Uuid, Path, description = "User ID to remove"),
    ),
    responses(
        (status = 200, description = "Member removed"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden – admin only"),
        (status = 404, description = "Member or room not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "realtime"
)]
pub async fn remove_member(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path((room_id, user_id)): Path<(Uuid, Uuid)>,
) -> ApiResult<serde_json::Value> {
    state.rooms.remove_member(room_id, user_id).await?;
    ok(serde_json::json!({ "message": "Member removed" }))
}
