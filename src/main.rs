//! 校務系統（NQU-ERP）後端程式進入點。
//!
//! 負責建立 DB 連線、執行 migration、組裝 Axum Router，並啟動 HTTP server。

use std::net::SocketAddr;

use axum::http::HeaderValue;
use nqu_erp::{build_router, config::Config, db, AppState};
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 tracing 日誌
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 讀取環境變數設定（DATABASE_URL / JWT_SECRET / SERVER_ADDR）
    let config = Config::from_env();

    // 建立 DB 連線，並在啟動前先執行所有 migration（失敗即退出，不留半初始化狀態）
    let db = db::establish_connection(&config).await?;
    db::run_migrations(&db).await?;

    tracing::info!("Database migrations completed");

    let state = AppState {
        db,
        jwt_secret: config.jwt_secret.clone(),
    };

    // CORS：依設定允許跨來源呼叫。缺省開放前端 dev server；設為 `*` 則全部放行（docker 同源部署用）
    let cors_origin = config.cors_origin.clone();
    let cors = if cors_origin == "*" {
        CorsLayer::new()
            .allow_origin(Any)
            .allow_methods(Any)
            .allow_headers(Any)
    } else {
        let origin = HeaderValue::from_str(&cors_origin)
            .unwrap_or(HeaderValue::from_static("http://localhost:5173"));
        CorsLayer::new()
            .allow_origin(origin)
            .allow_methods(Any)
            .allow_headers(Any)
    };

    let app = build_router(state, cors);

    let addr: SocketAddr = config.server_addr.parse()?;
    tracing::info!("Server starting on http://{}", addr);

    // 綁定監聽位址並啟動 HTTP 服務
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
