mod db;
mod error;
mod handlers;
mod models;

use axum::http::{header, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::Router;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use std::{net::SocketAddr, str::FromStr, sync::Arc};
use tower_http::cors::CorsLayer;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub pool: sqlx::SqlitePool,
    pub uploads_dir: Arc<String>,
    pub index_html: Arc<String>,
}

pub type ApiResult<T> = Result<T, error::ApiError>;

#[tokio::main]
async fn main() -> anyhow_lite::Result {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "petcare=info,tower_http=warn".into()),
        )
        .init();

    let db_url = std::env::var("DATABASE_URL").unwrap_or_else(|_| "data/petcare.db".into());
    let port: u16 = std::env::var("PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(8080);
    let host = std::env::var("HOST").unwrap_or_else(|_| "127.0.0.1".into());
    let static_dir = std::env::var("STATIC_DIR").unwrap_or_else(|_| "frontend/dist".into());
    let uploads_dir = std::env::var("UPLOADS_DIR").unwrap_or_else(|_| "data/uploads".into());

    tokio::fs::create_dir_all("data").await.ok();
    tokio::fs::create_dir_all(&uploads_dir).await.ok();

    let options = SqliteConnectOptions::from_str(&db_url)?
        .create_if_missing(true)
        .foreign_keys(true);
    let pool = SqlitePoolOptions::new()
        .max_connections(5)
        .connect_with(options)
        .await?;

    db::migrate(&pool).await?;
    db::seed(&pool).await?;

    let index_html = tokio::fs::read_to_string(format!("{static_dir}/index.html"))
        .await
        .unwrap_or_else(|_| {
            tracing::warn!("未找到 {static_dir}/index.html，请先构建前端");
            String::new()
        });

    let state = AppState {
        pool,
        uploads_dir: Arc::new(uploads_dir.clone()),
        index_html: Arc::new(index_html),
    };

    let api = handlers::router();
    let app = Router::new()
        .nest("/api", api)
        .nest_service("/uploads", ServeDir::new(&uploads_dir))
        .nest_service("/assets", ServeDir::new(format!("{static_dir}/assets")))
        .fallback(spa_fallback)
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr: SocketAddr = format!("{host}:{port}").parse()?;
    tracing::info!("🐾 宠物寄养照护交接系统已启动: http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

// 极简 anyhow 替代，避免额外依赖
mod anyhow_lite {
    pub type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;
}

/// 非 API、非静态资源的路径一律回退到前端入口（SPA），保持 200
async fn spa_fallback(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        Html((*state.index_html).clone()),
    )
}
