//! 系統整合（Web API）測試：以 in-process Router（tower oneshot）模擬完整 HTTP 流程，
//! 使用一次性暫存 SQLite 檔，驗證登入 → 選課 → 成績 → 管理端 CRUD 的端到端行為。
//!
//! 與原始碼內各模組的單元測試分開：單元測試跑 `cargo test --lib`，
//! 本測試檔跑 `cargo test --test system_tests`（或 `cargo test` 一併執行）。

use axum::body::Body;
use axum::http::{Request, StatusCode};
use axum::Router;
use http_body_util::BodyExt;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set};
use serde_json::{json, Value};
use tower::ServiceExt;
use tower_http::cors::CorsLayer;

use nqu_erp::config::Config;
use nqu_erp::db;
use nqu_erp::entities::{class_schedules, courses, departments, enrollments, users};
use nqu_erp::middleware::create_token;
use nqu_erp::AppState;

const SECRET: &str = "test-jwt-secret";
const PASSWORD: &str = "pw123456";

struct TestFixture {
    app: Router,
    db: DatabaseConnection,
    admin_id: i32,
    student_id: i32,
    student2_id: i32,
    teacher_id: i32,
    teacher2_id: i32,
    dept_id: i32,
    /// 常態加選流程用課（T001，週一 1~2 節，名額 2）
    course_id: i32,
    /// 與 course_id 衝突的課（同為週一，2~3 節）
    conflict_course_id: i32,
    /// 名額 1 的課（測滿額）
    capacity_course_id: i32,
    /// T002 的課（測「非授課教師不可查看名冊」）
    teacher2_course_id: i32,
    token_student: String,
    token_student2: String,
    token_teacher: String,
    token_teacher2: String,
    token_admin: String,
    db_path: std::path::PathBuf,
}

impl Drop for TestFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.db_path);
    }
}

async fn init_fixture() -> TestFixture {
    let db_path = std::env::temp_dir().join(format!("nqu-erp-test-{}.db", uuid::Uuid::new_v4()));
    let config = Config {
        database_url: format!("sqlite://{}?mode=rwc", db_path.display()),
        jwt_secret: SECRET.to_string(),
        server_addr: "0.0.0.0:0".to_string(),
        cors_origin: "*".to_string(),
    };

    let db_conn = db::establish_connection(&config).await.expect("connect db");
    db::run_migrations(&db_conn).await.expect("run migrations");

    let password_hash = bcrypt::hash(PASSWORD, 4).expect("hash password");

    let dept = seed_department(&db_conn).await;
    let dept_id = dept.dept_id;

    let admin = seed_user(
        &db_conn,
        &password_hash,
        "admin",
        "ADMIN",
        "管理員",
        "admin@t",
    )
    .await;
    let teacher = seed_user(
        &db_conn,
        &password_hash,
        "T001",
        "TEACHER",
        "教師一",
        "t001@t",
    )
    .await;
    let teacher2 = seed_user(
        &db_conn,
        &password_hash,
        "T002",
        "TEACHER",
        "教師二",
        "t002@t",
    )
    .await;
    let student = seed_user(
        &db_conn,
        &password_hash,
        "11303001",
        "STUDENT",
        "學生一",
        "s001@t",
    )
    .await;
    let student2 = seed_user(
        &db_conn,
        &password_hash,
        "11303002",
        "STUDENT",
        "學生二",
        "s002@t",
    )
    .await;

    let course = seed_course(&db_conn, "TEST001", "系統測試學", teacher.user_id, 2).await;
    let conflict_course = seed_course(&db_conn, "TEST002", "衝突學", teacher.user_id, 10).await;
    let capacity_course = seed_course(&db_conn, "TEST003", "滿額學", teacher.user_id, 1).await;
    let teacher2_course = seed_course(&db_conn, "TEST004", "教二學", teacher2.user_id, 10).await;

    seed_schedule(&db_conn, course.course_id, 1, 1, 2).await;
    seed_schedule(&db_conn, conflict_course.course_id, 1, 2, 3).await;
    seed_schedule(&db_conn, capacity_course.course_id, 2, 1, 2).await;
    seed_schedule(&db_conn, teacher2_course.course_id, 3, 4, 5).await;

    let state = AppState {
        db: db_conn.clone(),
        jwt_secret: SECRET.to_string(),
    };
    let app = nqu_erp::build_router(state, CorsLayer::new());

    TestFixture {
        app,
        db: db_conn,
        admin_id: admin.user_id,
        student_id: student.user_id,
        student2_id: student2.user_id,
        teacher_id: teacher.user_id,
        teacher2_id: teacher2.user_id,
        dept_id,
        course_id: course.course_id,
        conflict_course_id: conflict_course.course_id,
        capacity_course_id: capacity_course.course_id,
        teacher2_course_id: teacher2_course.course_id,
        token_student: create_token(student.user_id, &student.username, "STUDENT", SECRET).unwrap(),
        token_student2: create_token(student2.user_id, &student2.username, "STUDENT", SECRET)
            .unwrap(),
        token_teacher: create_token(teacher.user_id, &teacher.username, "TEACHER", SECRET).unwrap(),
        token_teacher2: create_token(teacher2.user_id, &teacher2.username, "TEACHER", SECRET)
            .unwrap(),
        token_admin: create_token(admin.user_id, &admin.username, "ADMIN", SECRET).unwrap(),
        db_path,
    }
}

fn req(method: &str, uri: &str, token: Option<&str>, body: Option<Value>) -> Request<Body> {
    let mut builder = Request::builder().method(method).uri(uri);
    if let Some(t) = token {
        builder = builder.header("Authorization", format!("Bearer {t}"));
    }
    if body.is_some() {
        builder = builder.header("Content-Type", "application/json");
    }
    let payload = body.map(|b| b.to_string()).unwrap_or_default();
    builder.body(Body::from(payload)).expect("build request")
}

/// 傳送請求並回傳 (status, body)。非 JSON body 以字串值回傳（如 /health）
async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.expect("oneshot");
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let json = serde_json::from_slice(&bytes)
        .unwrap_or(Value::String(String::from_utf8_lossy(&bytes).into_owned()));
    (status, json)
}

async fn send_json(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let (status, body) = send(app, request).await;
    assert!(!body.is_null(), "expected JSON body, got: {body}");
    (status, body)
}

async fn enroll(app: &Router, token: &str, course_id: i32) -> (StatusCode, Value) {
    send_json(
        app,
        req(
            "POST",
            "/api/v1/enrollments",
            Some(token),
            Some(json!({ "course_id": course_id })),
        ),
    )
    .await
}

async fn get_course(app: &Router, token: &str, uri: &str) -> (StatusCode, Value) {
    send_json(app, req("GET", uri, Some(token), None)).await
}

// ─── 認證 ───────────────────────────────────────────────────────────────────

#[tokio::test]
async fn health_check_ok() {
    let f = init_fixture().await;
    let (status, body) = send(&f.app, req("GET", "/health", None, None)).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, Value::String("OK".into()));
}

#[tokio::test]
async fn login_success_returns_token_and_user_info() {
    let f = init_fixture().await;
    let (status, body) = send_json(
        &f.app,
        req(
            "POST",
            "/api/v1/auth/login",
            None,
            Some(json!({ "username": "11303001", "password": PASSWORD })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["username"], "11303001");
    assert_eq!(body["full_name"], "學生一");
    assert_eq!(body["role"], "STUDENT");
    assert!(body["token"].as_str().unwrap().len() > 20);
}

#[tokio::test]
async fn login_wrong_password_is_unauthorized() {
    let f = init_fixture().await;
    let (status, body) = send_json(
        &f.app,
        req(
            "POST",
            "/api/v1/auth/login",
            None,
            Some(json!({ "username": "11303001", "password": "bad" })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "帳號或密碼錯誤");
}

#[tokio::test]
async fn login_unknown_account_is_unauthorized() {
    let f = init_fixture().await;
    let (status, _) = send_json(
        &f.app,
        req(
            "POST",
            "/api/v1/auth/login",
            None,
            Some(json!({ "username": "nobody", "password": PASSWORD })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn request_without_token_is_unauthorized() {
    let f = init_fixture().await;
    let (status, body) = send_json(
        &f.app,
        req("GET", "/api/v1/courses?year=113&semester=1", None, None),
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"], "缺少 Authorization 標頭");
}

#[tokio::test]
async fn request_with_invalid_token_is_unauthorized() {
    let f = init_fixture().await;
    let (status, body) = get_course(
        &f.app,
        "garbage-token",
        "/api/v1/courses?year=113&semester=1",
    )
    .await;

    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(body["error"].as_str().unwrap().contains("Token 無效"));
}

// ─── 角色權限 ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn teacher_cannot_access_admin_endpoints() {
    let f = init_fixture().await;
    let (status, body) = send_json(
        &f.app,
        req("GET", "/api/v1/admin/users", Some(&f.token_teacher), None),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "只有管理員可以查看帳號");
}

#[tokio::test]
async fn teacher_cannot_enroll() {
    let f = init_fixture().await;
    let (status, body) = enroll(&f.app, &f.token_teacher, f.course_id).await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "只有學生可以選課");
}

#[tokio::test]
async fn student_cannot_enter_grade() {
    let f = init_fixture().await;
    let (status, body) = send_json(
        &f.app,
        req(
            "PUT",
            "/api/v1/grades/batch",
            Some(&f.token_student),
            Some(json!({ "course_id": f.course_id, "grades": [] })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "只有教師可以登錄成績");
}

// ─── 課程查詢 ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn list_courses_filters_by_year_and_semester() {
    let f = init_fixture().await;

    let (status, body) = get_course(
        &f.app,
        &f.token_student,
        "/api/v1/courses?year=113&semester=1",
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body.as_array().unwrap().len(), 4);
    assert_eq!(body[0]["teacher_name"], "教師一");

    let (_, body) = get_course(
        &f.app,
        &f.token_student,
        "/api/v1/courses?year=113&semester=2",
    )
    .await;
    assert_eq!(body.as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn teacher_can_list_own_courses() {
    let f = init_fixture().await;
    let (status, body) = get_course(&f.app, &f.token_teacher2, "/api/v1/teachers/me/courses").await;

    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["course_code"], "TEST004");
    assert_eq!(items[0]["course_id"], f.teacher2_course_id);
    assert_eq!(items[0]["teacher_id"], f.teacher2_id);
}

// ─── 加退選 ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn enroll_success_then_duplicate_fails() {
    let f = init_fixture().await;

    let (status, body) = enroll(&f.app, &f.token_student, f.course_id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    assert_eq!(body["message"], "加選成功！");

    let (_, body) = enroll(&f.app, &f.token_student, f.course_id).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["message"], "您已選修過此課程");

    let (_, body) = get_course(&f.app, &f.token_student, "/api/v1/students/me/schedule").await;
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["course_code"], "TEST001");
    assert_eq!(items[0]["day_of_week"], 1);
    assert_eq!(items[0]["teacher_name"], "教師一");

    let (_, body) = get_course(&f.app, &f.token_student, "/api/v1/students/me/grades").await;
    assert_eq!(body[0]["course_code"], "TEST001");
}

#[tokio::test]
async fn enroll_into_conflicting_period_fails() {
    let f = init_fixture().await;

    let (_, body) = enroll(&f.app, &f.token_student, f.course_id).await;
    assert_eq!(body["success"], true);

    let (_, body) = enroll(&f.app, &f.token_student, f.conflict_course_id).await;
    assert_eq!(body["success"], false);
    assert!(body["message"].as_str().unwrap().contains("課程時間衝突"));
}

#[tokio::test]
async fn drop_then_reenroll_is_rejected() {
    let f = init_fixture().await;

    let (_, body) = enroll(&f.app, &f.token_student, f.course_id).await;
    assert_eq!(body["success"], true);

    let (_, body) = send_json(
        &f.app,
        req(
            "DELETE",
            "/api/v1/enrollments",
            Some(&f.token_student),
            Some(json!({ "course_id": f.course_id })),
        ),
    )
    .await;
    assert_eq!(body["success"], true);
    assert_eq!(body["message"], "退選成功！");

    let (_, body) = enroll(&f.app, &f.token_student, f.course_id).await;
    assert_eq!(body["success"], false);
    assert!(body["message"]
        .as_str()
        .unwrap()
        .contains("已退選課程，請聯繫教務處重新選課"));
}

#[tokio::test]
async fn drop_without_enrollment_is_not_found() {
    let f = init_fixture().await;
    let (status, _) = send_json(
        &f.app,
        req(
            "DELETE",
            "/api/v1/enrollments",
            Some(&f.token_student),
            Some(json!({ "course_id": f.course_id })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn enroll_missing_course_is_not_found() {
    let f = init_fixture().await;
    let (status, _) = enroll(&f.app, &f.token_student, 9999).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn enroll_fails_when_capacity_is_full() {
    let f = init_fixture().await;

    let (_, body) = enroll(&f.app, &f.token_student, f.capacity_course_id).await;
    assert_eq!(body["success"], true);

    let (_, body) = enroll(&f.app, &f.token_student2, f.capacity_course_id).await;
    assert_eq!(body["success"], false);
    assert_eq!(body["message"], "選課失敗：課程名額已滿");
}

#[tokio::test]
async fn schedule_only_contains_current_semester() {
    let f = init_fixture().await;
    let (_, body) = enroll(&f.app, &f.token_student, f.course_id).await;
    assert_eq!(body["success"], true);

    let (_, body) = get_course(
        &f.app,
        &f.token_student,
        "/api/v1/students/me/schedule?year=112&semester=2",
    )
    .await;
    assert_eq!(body.as_array().unwrap().len(), 0);

    let (_, body) = get_course(
        &f.app,
        &f.token_student,
        "/api/v1/students/me/schedule?year=113&semester=1",
    )
    .await;
    assert_eq!(body.as_array().unwrap().len(), 1);
}

// ─── 名冊與成績 ────────────────────────────────────────────────────────────

#[tokio::test]
async fn teacher_sees_roster_of_own_course_only() {
    let f = init_fixture().await;
    let (_, body) = enroll(&f.app, &f.token_student, f.course_id).await;
    assert_eq!(body["success"], true);

    let (status, body) = get_course(
        &f.app,
        &f.token_teacher,
        &format!("/api/v1/teachers/me/courses/{}/students", f.course_id),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["student_number"], "11303001");
    assert_eq!(items[0]["full_name"], "學生一");
    assert_eq!(items[0]["is_submitted"], false);

    // 非授課教師
    let (status, body) = get_course(
        &f.app,
        &f.token_teacher2,
        &format!("/api/v1/teachers/me/courses/{}/students", f.course_id),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "非您授課的課程");

    // 學生不能查看名冊
    let (status, _) = get_course(
        &f.app,
        &f.token_student,
        &format!("/api/v1/teachers/me/courses/{}/students", f.course_id),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn grade_batch_locks_after_submit() {
    let f = init_fixture().await;
    let (_, body) = enroll(&f.app, &f.token_student, f.course_id).await;
    assert_eq!(body["success"], true);

    let (_, roster) = get_course(
        &f.app,
        &f.token_teacher,
        &format!("/api/v1/teachers/me/courses/{}/students", f.course_id),
    )
    .await;
    let enrollment_id = roster[0]["enrollment_id"].as_i64().unwrap();

    let (status, body) = send_json(
        &f.app,
        req(
            "PUT",
            "/api/v1/grades/batch",
            Some(&f.token_teacher),
            Some(json!({
                "course_id": f.course_id,
                "grades": [
                    { "enrollment_id": enrollment_id, "midterm_score": 80, "final_score": 90 }
                ]
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    assert_eq!(body["message"], "成績登錄成功");

    // 名冊顯示加權總分 80*0.4 + 90*0.6 = 86
    let (_, roster) = get_course(
        &f.app,
        &f.token_teacher,
        &format!("/api/v1/teachers/me/courses/{}/students", f.course_id),
    )
    .await;
    assert_eq!(roster[0]["total_score"].as_f64().unwrap(), 86.0);

    // 送交後鎖定
    let (status, body) = send_json(
        &f.app,
        req(
            "POST",
            "/api/v1/grades/submit",
            Some(&f.token_teacher),
            Some(json!({ "course_id": f.course_id })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], "成績已鎖定送交");

    let (status, body) = send_json(
        &f.app,
        req(
            "PUT",
            "/api/v1/grades/batch",
            Some(&f.token_teacher),
            Some(json!({
                "course_id": f.course_id,
                "grades": [
                    { "enrollment_id": enrollment_id, "midterm_score": 60, "final_score": 60 }
                ]
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "此課程成績已送交，無法修改");

    // 學生視角看到送交成績
    let (_, body) = get_course(&f.app, &f.token_student, "/api/v1/students/me/grades").await;
    assert_eq!(body[0]["is_submitted"], true);
    assert_eq!(body[0]["total_score"].as_f64().unwrap(), 86.0);
}

#[tokio::test]
async fn grade_entry_must_belong_to_course() {
    let f = init_fixture().await;
    let (_, body) = enroll(&f.app, &f.token_student, f.course_id).await;
    assert_eq!(body["success"], true);
    let (_, body) = enroll(&f.app, &f.token_student, f.capacity_course_id).await;
    assert_eq!(body["success"], true);

    // 找出「滿額學」課程的選課紀錄
    let other_enrollment = enrollments::Entity::find()
        .filter(enrollments::Column::CourseId.eq(f.capacity_course_id))
        .one(&f.db)
        .await
        .expect("query enrollment")
        .expect("enrollment exists");

    let (status, body) = send_json(
        &f.app,
        req(
            "PUT",
            "/api/v1/grades/batch",
            Some(&f.token_teacher),
            Some(json!({
                "course_id": f.course_id,
                "grades": [
                    { "enrollment_id": other_enrollment.enrollment_id, "midterm_score": 80, "final_score": 90 }
                ]
            })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "選課紀錄不屬於此課程");
}

// ─── 管理端 ────────────────────────────────────────────────────────────────

#[tokio::test]
async fn admin_create_user_then_login() {
    let f = init_fixture().await;
    let (status, body) = send_json(
        &f.app,
        req(
            "POST",
            "/api/v1/admin/users",
            Some(&f.token_admin),
            Some(json!({
                "username": "11399999",
                "password": "newpass",
                "full_name": "新生",
                "role": "STUDENT",
                "dept_id": f.dept_id,
                "email": "new@t"
            })),
        ),
    )
    .await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["success"], true);
    assert!(body["message"].as_str().unwrap().contains("帳號建立成功"));

    let (status, _) = send_json(
        &f.app,
        req(
            "POST",
            "/api/v1/auth/login",
            None,
            Some(json!({ "username": "11399999", "password": "newpass" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn admin_update_user_password() {
    let f = init_fixture().await;
    send_json(
        &f.app,
        req(
            "POST",
            "/api/v1/admin/users",
            Some(&f.token_admin),
            Some(json!({
                "username": "11388888",
                "password": "oldpass",
                "full_name": "更新生",
                "role": "STUDENT",
                "dept_id": f.dept_id,
                "email": "upd@t"
            })),
        ),
    )
    .await;

    // 從帳號列表找出新帳號的 id
    let (_, users) = send_json(
        &f.app,
        req("GET", "/api/v1/admin/users", Some(&f.token_admin), None),
    )
    .await;
    let target = users
        .as_array()
        .unwrap()
        .iter()
        .find(|u| u["username"] == "11388888")
        .expect("new user in list")
        .clone();
    assert_eq!(target["dept_name"], "資訊工程學系");

    let (status, body) = send_json(
        &f.app,
        req(
            "PUT",
            &format!("/api/v1/admin/users/{}", target["user_id"]),
            Some(&f.token_admin),
            Some(json!({
                "password": "newpass",
                "full_name": "更新生",
                "role": "STUDENT",
                "dept_id": f.dept_id,
                "email": "upd@t"
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], "帳號更新成功");

    // 新密碼可登入，舊密碼不行
    let (status, _) = send_json(
        &f.app,
        req(
            "POST",
            "/api/v1/auth/login",
            None,
            Some(json!({ "username": "11388888", "password": "newpass" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    let (status, _) = send_json(
        &f.app,
        req(
            "POST",
            "/api/v1/auth/login",
            None,
            Some(json!({ "username": "11388888", "password": "oldpass" })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn admin_delete_user_guards() {
    let f = init_fixture().await;
    let (_, body) = enroll(&f.app, &f.token_student, f.course_id).await;
    assert_eq!(body["success"], true);

    // 有選課紀錄的學生不可刪除
    let (status, body) = send_json(
        &f.app,
        req(
            "DELETE",
            &format!("/api/v1/admin/users/{}", f.student_id),
            Some(&f.token_admin),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "該學生尚有選課紀錄，無法刪除");

    // 有課程的教師不可刪除
    let (status, body) = send_json(
        &f.app,
        req(
            "DELETE",
            &format!("/api/v1/admin/users/{}", f.teacher_id),
            Some(&f.token_admin),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "該教師尚有授課課程，無法刪除");

    // 不可刪除自己
    let (status, body) = send_json(
        &f.app,
        req(
            "DELETE",
            &format!("/api/v1/admin/users/{}", f.admin_id),
            Some(&f.token_admin),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"], "不能刪除自己");

    // 無選課紀錄的學生可刪除
    let (status, body) = send_json(
        &f.app,
        req(
            "DELETE",
            &format!("/api/v1/admin/users/{}", f.student2_id),
            Some(&f.token_admin),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["message"].as_str().unwrap().contains("刪除成功"));
}

#[tokio::test]
async fn admin_course_crud_with_cascade() {
    let f = init_fixture().await;
    let (_, body) = enroll(&f.app, &f.token_student, f.course_id).await;
    assert_eq!(body["success"], true);

    // 建立
    let (status, body) = send_json(
        &f.app,
        req(
            "POST",
            "/api/v1/admin/courses",
            Some(&f.token_admin),
            Some(json!({
                "course_code": "TEST999",
                "academic_year": 114,
                "semester": 2,
                "course_name": "新課",
                "teacher_id": f.teacher_id,
                "credits": 2,
                "capacity": 5,
                "dept_id": f.dept_id
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["message"].as_str().unwrap().contains("課程建立成功"));

    let (_, body) = get_course(
        &f.app,
        &f.token_admin,
        "/api/v1/courses?year=114&semester=2",
    )
    .await;
    assert_eq!(body[0]["course_code"], "TEST999");
    assert_eq!(body[0]["enrolled_count"], 0);

    // 修改
    let (status, body) = send_json(
        &f.app,
        req(
            "PUT",
            &format!("/api/v1/admin/courses/{}", f.course_id),
            Some(&f.token_admin),
            Some(json!({
                "course_code": "TEST001",
                "academic_year": 113,
                "semester": 1,
                "course_name": "改名課",
                "teacher_id": f.teacher_id,
                "credits": 3,
                "capacity": 4,
                "dept_id": f.dept_id
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["message"], "課程更新成功");

    let (_, body) = get_course(
        &f.app,
        &f.token_student,
        "/api/v1/courses?year=113&semester=1",
    )
    .await;
    let renamed = body
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["course_id"] == f.course_id)
        .expect("course still listed");
    assert_eq!(renamed["course_name"], "改名課");

    // 刪除（級聯清選與成績）
    let (status, body) = send_json(
        &f.app,
        req(
            "DELETE",
            &format!("/api/v1/admin/courses/{}", f.course_id),
            Some(&f.token_admin),
            None,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(body["message"].as_str().unwrap().contains("刪除成功"));

    let (_, body) = get_course(&f.app, &f.token_student, "/api/v1/students/me/schedule").await;
    assert_eq!(body.as_array().unwrap().len(), 0, "enrollment cascaded");

    // 非管理員不可開課
    let (status, body) = send_json(
        &f.app,
        req(
            "POST",
            "/api/v1/admin/courses",
            Some(&f.token_teacher),
            Some(json!({
                "course_code": "TEST888", "academic_year": 113, "semester": 1,
                "course_name": "x", "teacher_id": f.teacher_id, "credits": 1,
                "capacity": 1, "dept_id": f.dept_id
            })),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(body["error"], "只有管理員可以開課");
}

#[tokio::test]
async fn admin_list_teachers_and_departments() {
    let f = init_fixture().await;

    let (_, body) = send_json(
        &f.app,
        req("GET", "/api/v1/admin/teachers", Some(&f.token_admin), None),
    )
    .await;
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|t| t["user_id"] != Value::Null));

    let (_, body) = send_json(
        &f.app,
        req(
            "GET",
            "/api/v1/admin/departments",
            Some(&f.token_admin),
            None,
        ),
    )
    .await;
    let items = body.as_array().unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0]["dept_code"], "CS");
}

async fn seed_department(db: &DatabaseConnection) -> departments::Model {
    departments::ActiveModel {
        dept_code: Set("CS".into()),
        dept_name: Set("資訊工程學系".into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("seed dept")
}

async fn seed_user(
    db: &DatabaseConnection,
    password_hash: &str,
    username: &str,
    role: &str,
    full_name: &str,
    email: &str,
) -> users::Model {
    users::ActiveModel {
        username: Set(username.into()),
        password_hash: Set(password_hash.into()),
        full_name: Set(full_name.into()),
        role: Set(role.into()),
        dept_id: Set(1),
        email: Set(email.into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("seed user")
}

async fn seed_course(
    db: &DatabaseConnection,
    code: &str,
    name: &str,
    teacher_id: i32,
    capacity: i32,
) -> courses::Model {
    courses::ActiveModel {
        course_code: Set(code.into()),
        academic_year: Set(113),
        semester: Set(1),
        course_name: Set(name.into()),
        teacher_id: Set(teacher_id),
        credits: Set(3),
        capacity: Set(capacity),
        enrolled_count: Set(0),
        dept_id: Set(1),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("seed course")
}

async fn seed_schedule(
    db: &DatabaseConnection,
    course_id: i32,
    day: i32,
    start: i32,
    end: i32,
) -> class_schedules::Model {
    class_schedules::ActiveModel {
        course_id: Set(course_id),
        day_of_week: Set(day),
        start_period: Set(start),
        end_period: Set(end),
        location: Set("A101".into()),
        ..Default::default()
    }
    .insert(db)
    .await
    .expect("seed schedule")
}
