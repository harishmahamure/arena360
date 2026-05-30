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

    /// Kiosk/client-facing error with ErrorCode string in `message` and optional structured details.
    #[error("{code}")]
    Api {
        code: String,
        status: StatusCode,
        details: Option<serde_json::Value>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ErrorBody {
    status_code: u16,
    message: String,
    error: String,
    timestamp: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    details: Option<serde_json::Value>,
}

impl AppError {
    pub fn unauthorized_code(code: &str) -> Self {
        Self::Api {
            code: code.to_string(),
            status: StatusCode::UNAUTHORIZED,
            details: None,
        }
    }

    pub fn forbidden_code(code: &str) -> Self {
        Self::Api {
            code: code.to_string(),
            status: StatusCode::FORBIDDEN,
            details: None,
        }
    }

    pub fn conflict_code(code: &str, details: Option<serde_json::Value>) -> Self {
        Self::Api {
            code: code.to_string(),
            status: StatusCode::CONFLICT,
            details,
        }
    }

    fn status(&self) -> StatusCode {
        match self {
            Self::NotFound(_) => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Unauthorized(_) | Self::Jwt(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::TooManyRequests(_) => StatusCode::TOO_MANY_REQUESTS,
            Self::Internal(_) | Self::Database(_) => StatusCode::INTERNAL_SERVER_ERROR,
            Self::Api { status, .. } => *status,
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
            Self::Api { status, .. } => match *status {
                StatusCode::UNAUTHORIZED => "Unauthorized",
                StatusCode::FORBIDDEN => "Forbidden",
                StatusCode::CONFLICT => "Conflict",
                _ => "Error",
            },
        }
    }

    fn message(&self) -> String {
        match self {
            Self::Api { code, .. } => code.clone(),
            _ => self.to_string(),
        }
    }

    fn details(&self) -> Option<serde_json::Value> {
        match self {
            Self::Api { details, .. } => details.clone(),
            _ => None,
        }
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let status = self.status();
        let body = ErrorBody {
            status_code: status.as_u16(),
            message: self.message(),
            error: self.error_label().to_string(),
            timestamp: Utc::now().to_rfc3339(),
            details: self.details(),
        };

        (status, Json(body)).into_response()
    }
}
