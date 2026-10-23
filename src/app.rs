//! 應用程式組裝：`AppState` 共享狀態與 API Router（main 與系統測試共用同一份路由定義）。

use axum::routing::{get, post, put};
use axum::Router;
use tower_http::cors::CorsLayer;

use crate::handlers;

/// 全域共享狀態：透過 `State(state): State<AppState>` 注入到每個 handler。
#[derive(Clone)]
pub struct AppState {
    /// 資料庫連線（SQLite / PostgreSQL 皆可，由 `DATABASE_URL` 決定）
    pub db: sea_orm::DatabaseConnection,
    /// 簽發與驗證 JWT 用的密鑰
    pub jwt_secret: String,
}

/// 組裝 API Router（路由表集中於此）
pub fn build_router(state: AppState, cors: CorsLayer) -> Router {
    Router::new()
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
        // 注入共享狀態，套用 CORS
        .with_state(state)
        .layer(cors)
}
