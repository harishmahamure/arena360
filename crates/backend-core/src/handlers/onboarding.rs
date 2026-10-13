use crate::{
    app::AppState,
    dto::{ok, ApiResult},
    error::AppError,
    middleware::AdminOrStaff,
    models::onboarding::*,
    repositories::TenantOnboardingRepository,
};
use axum::{extract::State, Json};
use std::sync::Arc;

#[utoipa::path(get, path="/onboarding", responses((status=200, body=crate::openapi::responses::OnboardingEnvelope), (status=403, body=crate::openapi::responses::ErrorEnvelope)), security(("bearer_auth"=[])), tag="onboarding")]
pub async fn progress(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
) -> ApiResult<OnboardingSnapshot> {
    let actor = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user".into()))?;
    ok(
        TenantOnboardingRepository::new(state.business_db(&claims).await?)
            .snapshot(actor)
            .await?,
    )
}

#[utoipa::path(put, path="/onboarding", request_body=SaveOnboardingDto, responses((status=200, body=crate::openapi::responses::OnboardingEnvelope), (status=409, body=crate::openapi::responses::ErrorEnvelope)), security(("bearer_auth"=[])), tag="onboarding")]
pub async fn save_progress(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<SaveOnboardingDto>,
) -> ApiResult<OnboardingSnapshot> {
    let actor = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user".into()))?;
    ok(
        TenantOnboardingRepository::new(state.business_db(&claims).await?)
            .save(actor, dto)
            .await?,
    )
}

#[utoipa::path(post, path="/onboarding/complete", request_body=CompleteOnboardingDto, responses((status=200, body=crate::openapi::responses::OnboardingEnvelope), (status=409, body=crate::openapi::responses::ErrorEnvelope)), security(("bearer_auth"=[])), tag="onboarding")]
pub async fn complete(
    AdminOrStaff(claims): AdminOrStaff,
    State(state): State<Arc<AppState>>,
    Json(dto): Json<CompleteOnboardingDto>,
) -> ApiResult<OnboardingSnapshot> {
    let actor = claims
        .user_id_uuid()
        .ok_or_else(|| AppError::Unauthorized("Invalid user".into()))?;
    ok(
        TenantOnboardingRepository::new(state.business_db(&claims).await?)
            .complete(actor, dto)
            .await?,
    )
}
