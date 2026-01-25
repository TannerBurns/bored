use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

/// API error codes for client handling
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ErrorCode {
    NotFound,
    BadRequest,
    Unauthorized,
    Conflict,
    DatabaseError,
    InternalError,
    QueueEmpty,
    LockExpired,
    ValidationError,
}

/// Standard API error response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiError {
    pub error: String,
    pub code: ErrorCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            error: message.into(),
            code,
            details: None,
        }
    }

    pub fn not_found(resource: &str) -> Self {
        Self::new(ErrorCode::NotFound, format!("{} not found", resource))
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::BadRequest, message)
    }

    pub fn unauthorized() -> Self {
        Self::new(ErrorCode::Unauthorized, "Authentication required")
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, message)
    }

    pub fn database(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::DatabaseError, message)
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::InternalError, message)
    }

    pub fn queue_empty() -> Self {
        Self::new(ErrorCode::QueueEmpty, "No tickets available in queue")
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::ValidationError, message)
    }
}

/// Wrapper for API results
pub type ApiResult<T> = Result<T, AppError>;

/// Application error that converts to HTTP responses
#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub body: ApiError,
}

impl AppError {
    pub fn new(status: StatusCode, body: ApiError) -> Self {
        Self { status, body }
    }

    pub fn not_found(resource: &str) -> Self {
        Self::new(StatusCode::NOT_FOUND, ApiError::not_found(resource))
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, ApiError::bad_request(message))
    }

    pub fn unauthorized() -> Self {
        Self::new(StatusCode::UNAUTHORIZED, ApiError::unauthorized())
    }

    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, ApiError::conflict(message))
    }

    pub fn database(err: impl std::fmt::Display) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::database(err.to_string()),
        )
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            ApiError::internal(message),
        )
    }

    pub fn queue_empty() -> Self {
        Self::new(StatusCode::NOT_FOUND, ApiError::queue_empty())
    }

    pub fn validation(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, ApiError::validation(message))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        (self.status, Json(self.body)).into_response()
    }
}

impl From<crate::db::DbError> for AppError {
    fn from(err: crate::db::DbError) -> Self {
        match &err {
            crate::db::DbError::NotFound(msg) => Self::not_found(msg),
            crate::db::DbError::Validation(msg) => Self::validation(msg.clone()),
            _ => Self::database(err),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::DbError;

    #[test]
    fn api_error_not_found() {
        let err = ApiError::not_found("Ticket");
        assert_eq!(err.error, "Ticket not found");
        assert!(matches!(err.code, ErrorCode::NotFound));
    }

    #[test]
    fn api_error_validation() {
        let err = ApiError::validation("Title cannot be empty");
        assert_eq!(err.error, "Title cannot be empty");
        assert!(matches!(err.code, ErrorCode::ValidationError));
    }

    #[test]
    fn app_error_status_codes() {
        assert_eq!(AppError::not_found("x").status, StatusCode::NOT_FOUND);
        assert_eq!(AppError::bad_request("x").status, StatusCode::BAD_REQUEST);
        assert_eq!(AppError::unauthorized().status, StatusCode::UNAUTHORIZED);
        assert_eq!(AppError::conflict("x").status, StatusCode::CONFLICT);
        assert_eq!(AppError::queue_empty().status, StatusCode::NOT_FOUND);
        assert_eq!(AppError::validation("x").status, StatusCode::BAD_REQUEST);
    }

    #[test]
    fn from_db_error_not_found() {
        let db_err = DbError::NotFound("Ticket abc".to_string());
        let app_err: AppError = db_err.into();
        assert_eq!(app_err.status, StatusCode::NOT_FOUND);
        assert!(app_err.body.error.contains("Ticket abc"));
    }

    #[test]
    fn from_db_error_validation() {
        let db_err = DbError::Validation("Invalid input".to_string());
        let app_err: AppError = db_err.into();
        assert_eq!(app_err.status, StatusCode::BAD_REQUEST);
        assert!(matches!(app_err.body.code, ErrorCode::ValidationError));
    }

    #[test]
    fn from_db_error_sqlite() {
        let db_err = DbError::Sqlite(rusqlite::Error::InvalidQuery);
        let app_err: AppError = db_err.into();
        assert_eq!(app_err.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(matches!(app_err.body.code, ErrorCode::DatabaseError));
    }
}
