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
            timestamp: crate::time::utc_timestamp(&Utc::now()),
            data,
        }
    }
}

pub type ApiResult<T> = Result<SuccessResponse<T>, crate::error::AppError>;

pub fn ok<T: Serialize>(data: T) -> ApiResult<T> {
    Ok(SuccessResponse::new(StatusCode::OK, data))
}

pub fn created<T: Serialize>(data: T) -> ApiResult<T> {
    Ok(SuccessResponse::new(StatusCode::CREATED, data))
}

impl<T: Serialize> IntoResponse for SuccessResponse<T> {
    fn into_response(self) -> axum::response::Response {
        let status = StatusCode::from_u16(self.status_code).unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);
        (status, Json(self)).into_response()
    }
}
