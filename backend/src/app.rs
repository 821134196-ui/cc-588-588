use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde_json::json;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub inner: Arc<AppStateInner>,
}

pub struct AppStateInner {
    pub pool: SqlitePool,
    pub upload_dir: PathBuf,
    pub static_dir: PathBuf,
}

impl AppState {
    pub fn pool(&self) -> &SqlitePool {
        &self.inner.pool
    }
}

pub async fn build_state(database_url: &str, upload_dir: PathBuf, static_dir: PathBuf) -> anyhow::Result<AppState> {
    std::fs::create_dir_all(&upload_dir)?;
    std::fs::create_dir_all(&static_dir).ok();

    // foreign_keys 需要在每个连接上开启（SQLite 是连接级 pragma）
    let options = SqliteConnectOptions::from_str(database_url)?
        .foreign_keys(true)
        .create_if_missing(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    sqlx::migrate!("./migrations").run(&pool).await?;

    Ok(AppState {
        inner: Arc::new(AppStateInner {
            pool,
            upload_dir,
            static_dir,
        }),
    })
}

/// 统一错误类型：业务规则被拒绝时返回明确的 code，前端/测试据此判断
#[derive(Debug)]
pub struct AppError {
    pub status: StatusCode,
    pub code: String,
    pub message: String,
}

impl AppError {
    pub fn new(status: StatusCode, code: &str, message: impl Into<String>) -> Self {
        Self {
            status,
            code: code.to_string(),
            message: message.into(),
        }
    }
    pub fn bad_request(msg: &str) -> Self {
        Self::new(StatusCode::BAD_REQUEST, "bad_request", msg)
    }
    pub fn unauthorized(msg: &str) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, "unauthorized", msg)
    }
    pub fn forbidden(msg: &str) -> Self {
        Self::new(StatusCode::FORBIDDEN, "forbidden", msg)
    }
    pub fn not_found(msg: &str) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", msg)
    }
    /// 409 冲突：重复执行、已撤销取宠码、任务已取消等
    pub fn conflict(code: &str, msg: &str) -> Self {
        Self::new(StatusCode::CONFLICT, code, msg)
    }
    pub fn unprocessable(code: &str, msg: &str) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, code, msg)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = Json(json!({
            "error": self.code,
            "message": self.message,
        }));
        (self.status, body).into_response()
    }
}

// 把底层错误转 500，同时在服务端日志留下原因
impl From<sqlx::Error> for AppError {
    fn from(e: sqlx::Error) -> Self {
        tracing::error!(error = %e, "database error");
        AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "db_error", "数据库错误")
    }
}

impl From<anyhow::Error> for AppError {
    fn from(e: anyhow::Error) -> Self {
        tracing::error!(error = %e, "server error");
        AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "server_error", "服务器错误")
    }
}

impl From<std::io::Error> for AppError {
    fn from(e: std::io::Error) -> Self {
        tracing::error!(error = %e, "io error");
        AppError::new(StatusCode::INTERNAL_SERVER_ERROR, "io_error", "文件读写错误")
    }
}

pub type AppResult<T> = Result<T, AppError>;
