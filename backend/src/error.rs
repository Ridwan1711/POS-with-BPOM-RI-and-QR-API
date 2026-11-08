use axum::{
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use serde_json::{json, Value};

pub type AppResult<T> = Result<T, AppError>;

/// Every failure the API can return. Rendered as:
/// `{"error": {"code": "...", "message": "...", "details": ...}}`
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    BadRequest(String),
    #[error("{0}")]
    Unauthorized(String),
    #[error("{0}")]
    Forbidden(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{message}")]
    Conflict {
        code: &'static str,
        message: String,
        details: Value,
    },
    #[error("{message}")]
    Unprocessable {
        code: &'static str,
        message: String,
        details: Value,
    },
    #[error("request validation failed")]
    Validation { message: String, details: Value },
    #[error("too many requests")]
    RateLimited { retry_after_secs: u64 },
    #[error("database error")]
    Database(#[from] sqlx::Error),
    #[error("internal error: {0}")]
    Internal(String),
}

impl AppError {
    pub fn conflict(code: &'static str, message: impl Into<String>) -> Self {
        Self::Conflict {
            code,
            message: message.into(),
            details: Value::Null,
        }
    }

    pub fn unprocessable(code: &'static str, message: impl Into<String>, details: Value) -> Self {
        Self::Unprocessable {
            code,
            message: message.into(),
            details,
        }
    }
}

/// Builds a single-field validation error, e.g. `field_error("name", "must not be blank")`.
pub fn field_error(field: &str, message: &str) -> AppError {
    AppError::Validation {
        message: format!("{field} {message}"),
        details: json!({ field: [message] }),
    }
}

impl From<validator::ValidationErrors> for AppError {
    fn from(errors: validator::ValidationErrors) -> Self {
        AppError::Validation {
            message: "one or more fields are invalid".into(),
            details: serde_json::to_value(&errors).unwrap_or(Value::Null),
        }
    }
}

/// Runs `validator` rules on a request body.
pub fn validate<T: validator::Validate>(value: &T) -> AppResult<()> {
    value.validate().map_err(AppError::from)
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, code, message, details, retry_after) = match self {
            AppError::BadRequest(m) => (StatusCode::BAD_REQUEST, "bad_request", m, Value::Null, None),
            AppError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, "unauthorized", m, Value::Null, None),
            AppError::Forbidden(m) => (StatusCode::FORBIDDEN, "forbidden", m, Value::Null, None),
            AppError::NotFound(m) => (StatusCode::NOT_FOUND, "not_found", m, Value::Null, None),
            AppError::Conflict { code, message, details } => (StatusCode::CONFLICT, code, message, details, None),
            AppError::Unprocessable { code, message, details } => {
                (StatusCode::UNPROCESSABLE_ENTITY, code, message, details, None)
            }
            AppError::Validation { message, details } => {
                (StatusCode::UNPROCESSABLE_ENTITY, "validation_error", message, details, None)
            }
            AppError::RateLimited { retry_after_secs } => (
                StatusCode::TOO_MANY_REQUESTS,
                "rate_limited",
                format!("Too many requests. Try again in {retry_after_secs} seconds."),
                Value::Null,
                Some(retry_after_secs),
            ),
            AppError::Database(err) => {
                tracing::error!(error = %err, "database error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "An internal error occurred.".into(),
                    Value::Null,
                    None,
                )
            }
            AppError::Internal(err) => {
                tracing::error!(error = %err, "internal error");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    "An internal error occurred.".into(),
                    Value::Null,
                    None,
                )
            }
        };

        let body = Json(json!({
            "error": { "code": code, "message": message, "details": details }
        }));
        let mut response = (status, body).into_response();
        if let Some(seconds) = retry_after {
            if let Ok(value) = HeaderValue::from_str(&seconds.to_string()) {
                response.headers_mut().insert(header::RETRY_AFTER, value);
            }
        }
        response
    }
}
