use std::net::SocketAddr;
use std::path::PathBuf;

use petcare_backend::build_app;
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "petcare_backend=info,tower_http=warn".into()),
        )
        .init();

    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:data/petcare.db?mode=rwc".to_string());
    let upload_dir = PathBuf::from(std::env::var("UPLOAD_DIR").unwrap_or_else(|_| "data/uploads".into()));
    let static_dir =
        PathBuf::from(std::env::var("STATIC_DIR").unwrap_or_else(|_| "../frontend/dist".into()));
    let port: u16 = std::env::var("PORT").unwrap_or_else(|_| "8080".into()).parse()?;

    let uploads_dir = upload_dir.clone();
    let mut app = build_app(&database_url, upload_dir, static_dir).await?;
    // 本地照片目录
    app = app.nest_service("/uploads", ServeDir::new(&uploads_dir));
    app = app.layer(TraceLayer::new_for_http());

    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    tracing::info!("宠物寄养照护系统已启动: http://localhost:{}", port);
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
