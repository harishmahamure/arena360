use axum::{http::StatusCode, response::IntoResponse, Json};
use chrono::Utc;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Not found: {0}")]
    NotFound(String),

    #[error("{0}")]
    BadRequest(String),

    #[error("{0}")]
    Unauthorized(String),

    #[error("{0}")]
    Forbidden(String),

    #[error("{0}")]
    Conflict(String),

    #[error("{0}")]
    TooManyRequests(String),

    #[error("Internal error: {0}")]
    Internal(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("JWT error: {0}")]
    Jwt(#[from] jsonwebtoken::errors::Error),
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    status_code: u16,
    message: String,
    error: String,
    timestamp: String,
}

impl AppError {
    fn status(&self) -> StatusCode {
        match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) | Self::Jwt(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::Internal(_) | Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    fn error_label(&self) -> &'static str {
        match self {
            Self::NotFound(_) => "Not Found",
            Self::BadRequest(_) => "Bad Request",
            Self::Unauthorized(_) | Self::Jwt(_) => "Unauthorized",
            Self::Forbidden(_) => "Forbidden",
            Self::Conflict(_) => "Conflict",
            Self::TooManyRequests(_) => "Too Many Requests",
            Self::Internal(_) | Self::Database(_) => "Internal Server Error",
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status();
        let body = ErrorBody {
            status_code: status.as_u16(),
            message: self.to_string(),
            error: self.error_label().to_string(),
            timestamp: Utc::now().to_rfc3339(),
        };

        (status, Json(body)).into_response()
    }
}
