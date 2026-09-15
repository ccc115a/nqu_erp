//! 校務系統（NQU-ERP）後端程式進入點。
//!
//! 負責建立 DB 連線、執行 migration、組裝 Axum Router，並啟動 HTTP server。

mod config;
mod db;
mod entities;
mod errors;
mod handlers;
mod middleware;

use axum::{
    http::HeaderValue,
    routing::{get, post, put},
    Router,
};
use std::net::SocketAddr;
use tower_http::cors::{Any, CorsLayer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// 全域共享狀態：透過 `State(state): State<AppState>` 注入到每個 handler。
#[derive(Clone)]
pub struct AppState {
    /// 資料庫連線（SQLite / PostgreSQL 皆可，由 `DATABASE_URL` 決定）
    pub db: sea_orm::DatabaseConnection,
    /// 簽發與驗證 JWT 用的密鑰
    pub jwt_secret: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 初始化 tracing 日誌
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    // 讀取環境變數設定（DATABASE_URL / JWT_SECRET / SERVER_ADDR）
    let config = config::Config::from_env();

    // 建立 DB 連線，並在啟動前先執行所有 migration（失敗即退出，不留半初始化狀態）
    let db = db::establish_connection(&config).await?;
    db::run_migrations(&db).await?;

    tracing::info!("Database migrations completed");

    let state = AppState {
        db,
        jwt_secret: config.jwt_secret.clone(),
    };

    // CORS：僅允許前端 dev server（http://localhost:5173）跨來源呼叫
    let cors = CorsLayer::new()
        .allow_origin(HeaderValue::from_static("http://localhost:5173"))
        .allow_methods(Any)
        .allow_headers(Any);

    // 路由表：集中定義所有 API 端點與對應的 handler
    let app = Router::new()
        // 健康檢查，供部署探測使用
        .route("/health", get(|| async { "OK" }))
        // 認證：登入取得 JWT
        .route("/api/v1/auth/login", post(handlers::auth::login))
        // 課程查詢（依學年/學期篩選）
        .route("/api/v1/courses", get(handlers::course::list_courses))
        // 學生加退選
        .route(
            "/api/v1/enrollments",
            post(handlers::enrollment::enroll_course).delete(handlers::enrollment::drop_course),
        )
        // 學生個人課表
        .route(
            "/api/v1/students/me/schedule",
            get(handlers::enrollment::get_student_schedule),
        )
        // 學生歷年成績
        .route(
            "/api/v1/students/me/grades",
            get(handlers::grade::get_student_grades),
        )
        // 教師授課清單
        .route(
            "/api/v1/teachers/me/courses",
            get(handlers::course::get_teacher_courses),
        )
        // 教師查看某課程的學生名冊
        .route(
            "/api/v1/teachers/me/courses/{course_id}/students",
            get(handlers::grade::get_course_roster),
        )
        // 教師批次登錄成績
        .route(
            "/api/v1/grades/batch",
            put(handlers::grade::batch_update_grades),
        )
        // 教師鎖定送交成績
        .route(
            "/api/v1/grades/submit",
            post(handlers::grade::submit_grades),
        )
        // 管理員開設課程
        .route(
            "/api/v1/admin/courses",
            post(handlers::admin::create_course),
        )
        // 管理員修改 / 刪除課程
        .route(
            "/api/v1/admin/courses/{course_id}",
            put(handlers::admin::update_course).delete(handlers::admin::delete_course),
        )
        // 管理員帳號列表 / 建立帳號
        .route(
            "/api/v1/admin/users",
            get(handlers::admin::list_users).post(handlers::admin::create_user),
        )
        // 管理員修改 / 刪除帳號
        .route(
            "/api/v1/admin/users/{user_id}",
            put(handlers::admin::update_user).delete(handlers::admin::delete_user),
        )
        // 輔助下拉清單：教師（開課用）
        .route(
            "/api/v1/admin/teachers",
            get(handlers::admin::list_teachers),
        )
        // 輔助下拉清單：科系（表單用）
        .route(
            "/api/v1/admin/departments",
            get(handlers::admin::list_departments),
        )
        // 注入共享狀態
        .with_state(state)
        // 套用 CORS
        .layer(cors);

    let addr: SocketAddr = config.server_addr.parse()?;
    tracing::info!("Server starting on http://{}", addr);

    // 綁定監聽位址並啟動 HTTP 服務
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}
