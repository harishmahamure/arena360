use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use std::sync::Arc;
use uuid::Uuid;

use crate::app::AppState;
use crate::dto::{created, ok, ApiResult};
use crate::middleware::AdminUser;
use crate::models::{CreateGameDto, Game, GameFilterDto, UpdateGameDto};
use crate::openapi::responses::{ErrorEnvelope, GameEnvelope, GamePaginationEnvelope};

#[utoipa::path(
    get,
    path = "/games",
    params(GameFilterDto),
    responses(
        (status = 200, description = "List games", body = GamePaginationEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 500, description = "Internal server error", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "games"
)]
pub async fn list_games(
    State(state): State<Arc<AppState>>,
    Query(filters): Query<GameFilterDto>,
) -> ApiResult<crate::dto::PaginationResult<Game>> {
    let result = state.games.list(filters).await?;
    ok(result)
}

#[utoipa::path(
    get,
    path = "/games/{id}",
    params(("id" = Uuid, Path, description = "Game ID")),
    responses(
        (status = 200, description = "Get game", body = GameEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "games"
)]
pub async fn get_game(
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> ApiResult<Game> {
    let game = state.games.get_by_id(id).await?;
    ok(game)
}

#[utoipa::path(
    post,
    path = "/games",
    request_body = CreateGameDto,
    responses(
        (status = 201, description = "Create game", body = GameEnvelope),
        (status = 400, description = "Bad request", body = ErrorEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "games"
)]
pub async fn create_game(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CreateGameDto>,
) -> ApiResult<Game> {
    let game = state.games.create(dto, claims.user_id_uuid()).await?;
    created(game)
}

#[utoipa::path(
    patch,
    path = "/games/{id}",
    params(("id" = Uuid, Path, description = "Game ID")),
    request_body = UpdateGameDto,
    responses(
        (status = 200, description = "Update game", body = GameEnvelope),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "games"
)]
pub async fn update_game(
    AdminUser(claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
    Json(dto): Json<UpdateGameDto>,
) -> ApiResult<Game> {
    let game = state.games.update(id, dto, claims.user_id_uuid()).await?;
    ok(game)
}

#[utoipa::path(
    delete,
    path = "/games/{id}",
    params(("id" = Uuid, Path, description = "Game ID")),
    responses(
        (status = 204, description = "Soft-deleted"),
        (status = 401, description = "Unauthorized", body = ErrorEnvelope),
        (status = 403, description = "Forbidden", body = ErrorEnvelope),
        (status = 404, description = "Not found", body = ErrorEnvelope),
    ),
    security(("bearer_auth" = [])),
    tag = "games"
)]
pub async fn delete_game(
    AdminUser(_claims): AdminUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<Uuid>,
) -> Result<StatusCode, crate::error::AppError> {
    state.games.delete(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
