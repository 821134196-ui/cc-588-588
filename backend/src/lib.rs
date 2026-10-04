pub mod app;
pub mod auth;
pub mod routes;
pub mod schedule;
pub mod seed;

use axum::extract::State;
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;

use crate::app::AppState;

/// 构建完整应用（API + 本地照片 + 前端静态资源），供 main 与集成测试复用
pub async fn build_app(
    database_url: &str,
    upload_dir: std::path::PathBuf,
    static_dir: std::path::PathBuf,
) -> anyhow::Result<Router> {
    let state = app::build_state(database_url, upload_dir, static_dir.clone()).await?;
    seed::seed_if_empty(&state)
        .await
        .map_err(|e| anyhow::anyhow!("seed failed: {}", e.message))?;

    let app = Router::new()
        .nest("/api", routes::router())
        .fallback(spa_fallback)
        .nest_service("/assets", ServeDir::new(static_dir.join("assets")))
        .layer(CorsLayer::permissive())
        .with_state(state);
    Ok(app)
}

/// 未知路由：/api/* 返回 JSON 404；其余路径回退 index.html（SPA，200）
async fn spa_fallback(
    State(state): State<AppState>,
    uri: axum::http::Uri,
) -> Response {
    if uri.path().starts_with("/api/") {
        return (
            StatusCode::NOT_FOUND,
            [(header::CONTENT_TYPE, HeaderValue::from_static("application/json"))],
            r#"{"error":"not_found","message":"接口不存在"}"#,
        )
            .into_response();
    }
    if uri.path().starts_with("/uploads/") {
        return StatusCode::NOT_FOUND.into_response();
    }

    match tokio::fs::read(state.inner.static_dir.join("index.html")).await {
        Ok(bytes) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, HeaderValue::from_static("text/html; charset=utf-8"))],
            bytes,
        )
            .into_response(),
        Err(_) => (
            StatusCode::SERVICE_UNAVAILABLE,
            "前端尚未构建，请先运行 ./run.sh",
        )
            .into_response(),
    }
}
