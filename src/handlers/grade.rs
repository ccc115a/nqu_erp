//! 成績管理 handler：教師登錄／送交成績、查看名冊；學生查看自己的成績。

use axum::extract::{Path, State};
use axum::Json;
use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, TransactionTrait};
use serde::{Deserialize, Serialize};

use crate::entities::{courses, enrollments, grades, users};
use crate::errors::AppError;
use crate::middleware::AuthUser;
use crate::AppState;

/// 單一學生的成績輸入項
#[derive(Deserialize)]
pub struct GradeEntry {
    pub enrollment_id: i32,
    pub midterm_score: Option<f64>,
    pub final_score: Option<f64>,
}

/// 批次成績登錄請求：一門課一次送出多筆
#[derive(Deserialize)]
pub struct BatchGradeRequest {
    pub course_id: i32,
    pub grades: Vec<GradeEntry>,
}

/// 成績操作的統一回應格式
#[derive(Serialize)]
pub struct GradeResponse {
    pub success: bool,
    pub message: String,
}

/// 送交（鎖定）成績請求
#[derive(Deserialize)]
pub struct SubmitRequest {
    pub course_id: i32,
}

/// PUT /api/v1/grades/batch：教師批次登錄某課程的成績
pub async fn batch_update_grades(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<BatchGradeRequest>,
) -> Result<Json<GradeResponse>, AppError> {
    // 僅教師角色
    if !auth.is_teacher() {
        return Err(AppError::forbidden("只有教師可以登錄成績"));
    }

    let tx = state.db.begin().await?;

    // 逐筆檢查選課紀錄、計算加權總分並寫入
    for entry in &payload.grades {
        let enrollment = enrollments::Entity::find_by_id(entry.enrollment_id)
            .one(&tx)
            .await?
            .ok_or_else(|| AppError::not_found("選課紀錄不存在"))?;

        // 安全性檢查：選課紀錄必須屬於此次送出的課程
        if enrollment.course_id != payload.course_id {
            return Err(AppError::bad_request("選課紀錄不屬於此課程"));
        }

        let existing_grade = grades::Entity::find()
            .filter(grades::Column::EnrollmentId.eq(entry.enrollment_id))
            .one(&tx)
            .await?;

        // 加權總分 = 期中 40% + 期末 60%
        let midterm = entry.midterm_score.unwrap_or(0.0);
        let final_ = entry.final_score.unwrap_or(0.0);
        let total = midterm * 0.4 + final_ * 0.6;

        if let Some(g) = existing_grade {
            // 已送交的成績不可再修改
            if g.is_submitted {
                return Err(AppError::forbidden("此課程成績已送交，無法修改"));
            }

            grades::Entity::update(grades::ActiveModel {
                grade_id: Set(g.grade_id),
                midterm_score: Set(Some(midterm)),
                final_score: Set(Some(final_)),
                total_score: Set(Some(total)),
                updated_at: Set(chrono::Utc::now()),
                ..Default::default()
            })
            .exec(&tx)
            .await?;
        } else {
            // 平常加選時已建立成績列，理論上不會走到這裡；保險起見仍可新增
            grades::ActiveModel {
                enrollment_id: Set(entry.enrollment_id),
                midterm_score: Set(Some(midterm)),
                final_score: Set(Some(final_)),
                total_score: Set(Some(total)),
                ..Default::default()
            }
            .insert(&tx)
            .await?;
        }
    }

    tx.commit().await?;

    Ok(Json(GradeResponse {
        success: true,
        message: "成績登錄成功".into(),
    }))
}

/// POST /api/v1/grades/submit：教師將某課程全部成績鎖定送交（不可再改）
pub async fn submit_grades(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<SubmitRequest>,
) -> Result<Json<GradeResponse>, AppError> {
    // 僅教師角色
    if !auth.is_teacher() {
        return Err(AppError::forbidden("只有教師可以送交成績"));
    }

    // 找出該課程所有選課紀錄
    let enrollments = enrollments::Entity::find()
        .filter(enrollments::Column::CourseId.eq(payload.course_id))
        .all(&state.db)
        .await?;

    // 逐一將 is_submitted 設為 true
    for e in &enrollments {
        let grade = grades::Entity::find()
            .filter(grades::Column::EnrollmentId.eq(e.enrollment_id))
            .one(&state.db)
            .await?;

        if let Some(g) = grade {
            grades::Entity::update(grades::ActiveModel {
                grade_id: Set(g.grade_id),
                is_submitted: Set(true),
                updated_at: Set(chrono::Utc::now()),
                ..Default::default()
            })
            .exec(&state.db)
            .await?;
        }
    }

    Ok(Json(GradeResponse {
        success: true,
        message: "成績已鎖定送交".into(),
    }))
}

/// 學生成績清單項目（依每日選課紀錄展開為一筆）
#[derive(Serialize)]
pub struct StudentGradeItem {
    pub course_code: String,
    pub course_name: String,
    pub credits: i32,
    pub midterm_score: Option<f64>,
    pub final_score: Option<f64>,
    pub total_score: Option<f64>,
    pub is_submitted: bool,
}

/// GET /api/v1/students/me/grades：學生查看自己的所有成績
pub async fn get_student_grades(
    State(state): State<AppState>,
    auth: AuthUser,
) -> Result<Json<Vec<StudentGradeItem>>, AppError> {
    // 僅學生角色
    if !auth.is_student() {
        return Err(AppError::forbidden("只有學生可以查看成績"));
    }

    // 取得該學生的全部選課紀錄（含已退選？），逐一補上課程與成績
    let enrollments = enrollments::Entity::find()
        .filter(enrollments::Column::StudentId.eq(auth.user_id))
        .all(&state.db)
        .await?;

    let mut items = Vec::new();

    for e in &enrollments {
        let course = crate::entities::courses::Entity::find_by_id(e.course_id)
            .one(&state.db)
            .await?;

        let grade = grades::Entity::find()
            .filter(grades::Column::EnrollmentId.eq(e.enrollment_id))
            .one(&state.db)
            .await?;

        if let Some(c) = course {
            items.push(StudentGradeItem {
                course_code: c.course_code,
                course_name: c.course_name,
                credits: c.credits,
                midterm_score: grade.as_ref().and_then(|g| g.midterm_score),
                final_score: grade.as_ref().and_then(|g| g.final_score),
                total_score: grade.as_ref().and_then(|g| g.total_score),
                is_submitted: grade.as_ref().map(|g| g.is_submitted).unwrap_or(false),
            });
        }
    }

    Ok(Json(items))
}

/// 教師端名冊項目：名冊列 + 成績輸入框綁定的 enrollment_id
#[derive(Serialize)]
pub struct RosterItem {
    pub enrollment_id: i32,
    pub student_id: i32,
    pub student_number: String,
    pub full_name: String,
    pub midterm_score: Option<f64>,
    pub final_score: Option<f64>,
    pub total_score: Option<f64>,
    pub is_submitted: bool,
}

/// GET /api/v1/teachers/me/courses/{course_id}/students：教師查看課程學生名冊
pub async fn get_course_roster(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(course_id): Path<i32>,
) -> Result<Json<Vec<RosterItem>>, AppError> {
    // 僅教師角色
    if !auth.is_teacher() {
        return Err(AppError::forbidden("只有教師可以查看學生名冊"));
    }

    let course = courses::Entity::find_by_id(course_id)
        .one(&state.db)
        .await?
        .ok_or_else(|| AppError::not_found("課程不存在"))?;

    // 教師只能查看自己授課的課程名冊
    if course.teacher_id != auth.user_id {
        return Err(AppError::forbidden("非您授課的課程"));
    }

    // 找出該課程 ENROLLED 的學生
    let enrollments = enrollments::Entity::find()
        .filter(enrollments::Column::CourseId.eq(course_id))
        .filter(enrollments::Column::Status.eq("ENROLLED"))
        .all(&state.db)
        .await?;

    let mut items = Vec::new();

    // 每筆選課補上學生基本資料與成績狀態
    for e in &enrollments {
        let student = users::Entity::find_by_id(e.student_id)
            .one(&state.db)
            .await?;
        let grade = grades::Entity::find()
            .filter(grades::Column::EnrollmentId.eq(e.enrollment_id))
            .one(&state.db)
            .await?;

        items.push(RosterItem {
            enrollment_id: e.enrollment_id,
            student_id: e.student_id,
            student_number: student
                .as_ref()
                .map(|s| s.username.clone())
                .unwrap_or_default(),
            full_name: student.map(|s| s.full_name).unwrap_or_default(),
            midterm_score: grade.as_ref().and_then(|g| g.midterm_score),
            final_score: grade.as_ref().and_then(|g| g.final_score),
            total_score: grade.as_ref().and_then(|g| g.total_score),
            is_submitted: grade.as_ref().map(|g| g.is_submitted).unwrap_or(false),
        });
    }

    Ok(Json(items))
}
