use axum::{http::StatusCode, response::IntoResponse, Json};
use chrono::Utc;
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SuccessResponse<T: Serialize> {
    pub success: bool,
    pub status_code: u16,
    pub timestamp: String,
    pub data: T,
}

impl<T: Serialize> SuccessResponse<T> {
    pub fn new(status: StatusCode, data: T) -> Self {
        Self {
            success: true,
            status_code: status.as_u16(),
            timestamp: Utc::now().to_rfc3339(),
            data,
        }
    }
}

pub type ApiResult<T> = Result<Json<SuccessResponse<T>>, crate::error::AppError>;

pub fn ok<T: Serialize>(data: T) -> ApiResult<T> {
    Ok(Json(SuccessResponse::new(StatusCode::OK, data)))
}

pub fn created<T: Serialize>(data: T) -> ApiResult<T> {
    Ok(Json(SuccessResponse::new(StatusCode::CREATED, data)))
}

impl<T: Serialize> IntoResponse for SuccessResponse<T> {
    fn into_response(self) -> axum::response::Response {
        Json(self).into_response()
    }
}
