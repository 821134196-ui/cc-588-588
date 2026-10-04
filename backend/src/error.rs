use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;

#[derive(Debug)]
pub enum ApiError {
    BadRequest(String),
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    Conflict(String),
    Sqlx(sqlx::Error),
    Other(String),
}

impl ApiError {
    pub fn bad_request(msg: impl Into<String>) -> Self {
        ApiError::BadRequest(msg.into())
    }
    pub fn unauthorized(msg: impl Into<String>) -> Self {
        ApiError::Unauthorized(msg.into())
    }
    pub fn forbidden(msg: impl Into<String>) -> Self {
        ApiError::Forbidden(msg.into())
    }
    pub fn not_found(msg: impl Into<String>) -> Self {
        ApiError::NotFound(msg.into())
    }
    pub fn conflict(msg: impl Into<String>) -> Self {
        ApiError::Conflict(msg.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, code, message) = match self {
            ApiError::BadRequest(m) => (StatusCode::BAD_REQUEST, "bad_request", m),
            ApiError::Unauthorized(m) => (StatusCode::UNAUTHORIZED, "unauthorized", m),
            ApiError::Forbidden(m) => (StatusCode::FORBIDDEN, "forbidden", m),
            ApiError::NotFound(m) => (StatusCode::NOT_FOUND, "not_found", m),
            ApiError::Conflict(m) => (StatusCode::CONFLICT, "conflict", m),
            ApiError::Sqlx(e) => {
                tracing::error!("数据库错误: {e}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "数据库错误".to_string(),
                )
            }
            ApiError::Other(m) => {
                tracing::error!("内部错误: {m}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal",
                    "服务器内部错误".to_string(),
                )
            }
        };
        (status, Json(json!({ "error": code, "message": message }))).into_response()
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        // 唯一约束冲突（如重复提交执行记录）
        if let sqlx::error::Error::Database(db) = &e {
            if db.message().contains("UNIQUE") {
                return ApiError::Conflict("该任务已存在执行记录，请勿重复提交".into());
            }
        }
        ApiError::Sqlx(e)
    }
}

impl From<std::io::Error> for ApiError {
    fn from(e: std::io::Error) -> Self {
        ApiError::Other(e.to_string())
    }
}
