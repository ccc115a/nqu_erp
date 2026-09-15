//! 管理員 handler：帳號與課程的 CRUD，以及親助用的教師／科系清單。
//! 所有端點皆需 ADMIN 角色。

use axum::{extract::Path, extract::State, Json};
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde::{Deserialize, Serialize};

use crate::entities::{class_schedules, courses, departments, enrollments, grades, users};
use crate::errors::AppError;
use crate::middleware::AuthUser;
use crate::AppState;

/// POST /api/v1/admin/courses 請求體：開設課程所需欄位
#[derive(Deserialize)]
pub struct CreateCourseRequest {
    pub course_code: String,
    pub academic_year: i32,
    pub semester: i32,
    pub course_name: String,
    pub teacher_id: i32,
    pub credits: i32,
    pub capacity: i32,
    pub dept_id: i32,
}

/// 管理端操作統一回應格式
#[derive(Serialize)]
pub struct AdminResponse {
    pub success: bool,
    pub message: String,
}

/// POST /api/v1/admin/courses：開設新課程（名額從 0 起算）
pub async fn create_course(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CreateCourseRequest>,
) -> Result<Json<AdminResponse>, AppError> {
    // 僅管理員
    if !auth.is_admin() {
        return Err(AppError::forbidden("只有管理員可以開課"));
    }

    let course = courses::ActiveModel {
        course_code: Set(payload.course_code),
        academic_year: Set(payload.academic_year),
        semester: Set(payload.semester),
        course_name: Set(payload.course_name),
        teacher_id: Set(payload.teacher_id),
        credits: Set(payload.credits),
        capacity: Set(payload.capacity),
        enrolled_count: Set(0),
        dept_id: Set(payload.dept_id),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;

    Ok(Json(AdminResponse {
        success: true,
        message: format!("課程建立成功，ID: {}", course.course_id),
    }))
}

/// POST /api/v1/admin/users 請求體：建立帳號所需欄位
#[derive(Deserialize)]
pub struct CreateUserRequest {
    pub username: String,
    pub password: String,
    pub full_name: String,
    pub role: String,
    pub dept_id: i32,
    pub email: String,
}

/// POST /api/v1/admin/users：建立學生／教師／管理員帳號
pub async fn create_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<CreateUserRequest>,
) -> Result<Json<AdminResponse>, AppError> {
    // 僅管理員
    if !auth.is_admin() {
        return Err(AppError::forbidden("只有管理員可以建立帳號"));
    }

    // 密碼以 bcrypt 雜湊後才存庫（cost 10）
    let password_hash =
        bcrypt::hash(&payload.password, 10).map_err(|_| AppError::internal("密碼雜湊失敗"))?;

    let user = users::ActiveModel {
        username: Set(payload.username),
        password_hash: Set(password_hash),
        full_name: Set(payload.full_name),
        role: Set(payload.role),
        dept_id: Set(payload.dept_id),
        email: Set(payload.email),
        ..Default::default()
    }
    .insert(&state.db)
    .await?;

    Ok(Json(AdminResponse {
        success: true,
        message: format!("帳號建立成功，ID: {}", user.user_id),
    }))
}

/// 帳號列表項目（含科系名稱，不回傳密碼）
#[derive(Serialize)]
pub struct UserListItem {
    pub user_id: i32,
    pub username: String,
    pub full_name: String,
    pub role: String,
    pub dept_id: i32,
    pub dept_name: String,
    pub email: String,
}

/// GET /api/v1/admin/users：列出所有帳號
pub async fn list_users(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<UserListItem>>, AppError> {
    // 僅管理員
    if !auth.is_admin() {
        return Err(AppError::forbidden("只有管理員可以查看帳號"));
    }

    let all_users = users::Entity::find().all(&state.db).await?;
    let mut items = Vec::new();

    // 逐一補上科系名稱
    for u in &all_users {
        let dept = departments::Entity::find_by_id(u.dept_id)
            .one(&state.db)
            .await?;
        items.push(UserListItem {
            user_id: u.user_id,
            username: u.username.clone(),
            full_name: u.full_name.clone(),
            role: u.role.clone(),
            dept_id: u.dept_id,
            dept_name: dept.map(|d| d.dept_name).unwrap_or_default(),
            email: u.email.clone(),
        });
    }

    Ok(Json(items))
}

/// 教師下拉選項（供開課表單選授課教師）
#[derive(Serialize)]
pub struct TeacherListItem {
    pub user_id: i32,
    pub username: String,
    pub full_name: String,
    pub dept_name: String,
}

/// GET /api/v1/admin/teachers：列出所有教師（角色 = TEACHER）
pub async fn list_teachers(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<TeacherListItem>>, AppError> {
    // 僅管理員
    if !auth.is_admin() {
        return Err(AppError::forbidden("只有管理員可以查看教師清單"));
    }

    let all_users = users::Entity::find()
        .filter(users::Column::Role.eq("TEACHER"))
        .all(&state.db)
        .await?;
    let mut items = Vec::new();

    for u in &all_users {
        let dept = departments::Entity::find_by_id(u.dept_id)
            .one(&state.db)
            .await?;
        items.push(TeacherListItem {
            user_id: u.user_id,
            username: u.username.clone(),
            full_name: u.full_name.clone(),
            dept_name: dept.map(|d| d.dept_name).unwrap_or_default(),
        });
    }

    Ok(Json(items))
}

/// 科系下拉選項
#[derive(Serialize)]
pub struct DepartmentItem {
    pub dept_id: i32,
    pub dept_code: String,
    pub dept_name: String,
}

/// GET /api/v1/admin/departments：列出所有科系
pub async fn list_departments(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<DepartmentItem>>, AppError> {
    // 僅管理員
    if !auth.is_admin() {
        return Err(AppError::forbidden("只有管理員可以查看科系清單"));
    }

    let depts = departments::Entity::find().all(&state.db).await?;
    Ok(Json(
        depts
            .into_iter()
            .map(|d| DepartmentItem {
                dept_id: d.dept_id,
                dept_code: d.dept_code,
                dept_name: d.dept_name,
            })
            .collect(),
    ))
}

/// PUT /api/v1/admin/users/{user_id} 請求體：password 為可選（留空不修改）
#[derive(Deserialize)]
pub struct UpdateUserRequest {
    pub password: Option<String>,
    pub full_name: String,
    pub role: String,
    pub dept_id: i32,
    pub email: String,
}

/// PUT /api/v1/admin/users/{user_id}：修改帳號（不允許改 username）
pub async fn update_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(user_id): Path<i32>,
    Json(payload): Json<UpdateUserRequest>,
) -> Result<Json<AdminResponse>, AppError> {
    // 僅管理員
    if !auth.is_admin() {
        return Err(AppError::forbidden("只有管理員可以修改帳號"));
    }

    let user = users::Entity::find_by_id(user_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("帳號不存在"))?;

    // 以現有資料為基底做部分更新
    let mut model: users::ActiveModel = user.into();
    // 密碼為可選欄位：有提供且非空字串才重新雜湊
    if let Some(password) = payload.password {
        if !password.is_empty() {
            let hash =
                bcrypt::hash(&password, 10).map_err(|_| AppError::internal("密碼雜湊失敗"))?;
            model.password_hash = Set(hash);
        }
    }
    model.full_name = Set(payload.full_name);
    model.role = Set(payload.role);
    model.dept_id = Set(payload.dept_id);
    model.email = Set(payload.email);
    model.update(&state.db).await?;

    Ok(Json(AdminResponse {
        success: true,
        message: "帳號更新成功".to_string(),
    }))
}

/// DELETE /api/v1/admin/users/{user_id}：刪除帳號
///
/// 安全防護：不可刪除自己；教師有授課或學生有選課紀錄時拒絕刪除。
pub async fn delete_user(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(user_id): Path<i32>,
) -> Result<Json<AdminResponse>, AppError> {
    // 僅管理員
    if !auth.is_admin() {
        return Err(AppError::forbidden("只有管理員可以刪除帳號"));
    }
    // 避免管理員誤刪自己的帳號
    if user_id == auth.user_id {
        return Err(AppError::bad_request("不能刪除自己"));
    }

    let user = users::Entity::find_by_id(user_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("帳號不存在"))?;

    // 若為教師且身上掛有授課課程 → 拒絕（避免課程失去授課者）
    let has_courses = !courses::Entity::find()
        .filter(courses::Column::TeacherId.eq(user_id))
        .all(&state.db)
        .await?
        .is_empty();
    if has_courses {
        return Err(AppError::bad_request("該教師尚有授課課程，無法刪除"));
    }

    // 若為學生且有選課紀錄 → 拒絕（避免選課／成績資料斷裂）
    let has_enrollments = !enrollments::Entity::find()
        .filter(enrollments::Column::StudentId.eq(user_id))
        .all(&state.db)
        .await?
        .is_empty();
    if has_enrollments {
        return Err(AppError::bad_request("該學生尚有選課紀錄，無法刪除"));
    }

    users::Entity::delete_by_id(user_id).exec(&state.db).await?;

    Ok(Json(AdminResponse {
        success: true,
        message: format!("帳號 {} 刪除成功", user.username),
    }))
}

/// PUT /api/v1/admin/courses/{course_id} 請求體：與建立課程欄位一致
#[derive(Deserialize)]
pub struct UpdateCourseRequest {
    pub course_code: String,
    pub academic_year: i32,
    pub semester: i32,
    pub course_name: String,
    pub teacher_id: i32,
    pub credits: i32,
    pub capacity: i32,
    pub dept_id: i32,
}

/// PUT /api/v1/admin/courses/{course_id}：修改課程基本資料
pub async fn update_course(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(course_id): Path<i32>,
    Json(payload): Json<UpdateCourseRequest>,
) -> Result<Json<AdminResponse>, AppError> {
    // 僅管理員
    if !auth.is_admin() {
        return Err(AppError::forbidden("只有管理員可以修改課程"));
    }

    let course = courses::Entity::find_by_id(course_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("課程不存在"))?;

    // 全欄位覆蓋更新（enrolled_count 保留不動）
    let mut model: courses::ActiveModel = course.into();
    model.course_code = Set(payload.course_code);
    model.academic_year = Set(payload.academic_year);
    model.semester = Set(payload.semester);
    model.course_name = Set(payload.course_name);
    model.teacher_id = Set(payload.teacher_id);
    model.credits = Set(payload.credits);
    model.capacity = Set(payload.capacity);
    model.dept_id = Set(payload.dept_id);
    model.update(&state.db).await?;

    Ok(Json(AdminResponse {
        success: true,
        message: "課程更新成功".to_string(),
    }))
}

/// DELETE /api/v1/admin/courses/{course_id}：刪除課程
///
/// 使用 Transaction 級聯清理：成績 → 選課 → 上課時間 → 課程本身。
pub async fn delete_course(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(course_id): Path<i32>,
) -> Result<Json<AdminResponse>, AppError> {
    // 僅管理員
    if !auth.is_admin() {
        return Err(AppError::forbidden("只有管理員可以刪除課程"));
    }

    let course = courses::Entity::find_by_id(course_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("課程不存在"))?;

    let tx = state.db.begin().await?;
    // 1. 刪除該課程所有選課所對應的成績
    let enrollments = enrollments::Entity::find()
        .filter(enrollments::Column::CourseId.eq(course_id))
        .all(&tx)
        .await?;
    let enrollment_ids: Vec<i32> = enrollments.iter().map(|e| e.enrollment_id).collect();
    if !enrollment_ids.is_empty() {
        grades::Entity::delete_many()
            .filter(grades::Column::EnrollmentId.is_in(enrollment_ids))
            .exec(&tx)
            .await?;
    }
    // 2. 刪除選課紀錄
    enrollments::Entity::delete_many()
        .filter(enrollments::Column::CourseId.eq(course_id))
        .exec(&tx)
        .await?;
    // 3. 刪除上課時間
    class_schedules::Entity::delete_many()
        .filter(class_schedules::Column::CourseId.eq(course_id))
        .exec(&tx)
        .await?;
    // 4. 最後刪除課程本身
    courses::Entity::delete_by_id(course_id).exec(&tx).await?;
    tx.commit().await?;

    Ok(Json(AdminResponse {
        success: true,
        message: format!("課程 {} 刪除成功", course.course_name),
    }))
}
