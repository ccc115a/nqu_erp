//! 選課／退選／個人課表 handler。加退選皆使用 Transaction 確保多表一致性。

use axum::{extract::State, Json};
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, ModelTrait, QueryFilter, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize};

use crate::entities::{class_schedules, courses, enrollments, grades};
use crate::errors::AppError;
use crate::middleware::AuthUser;
use crate::AppState;

/// 加選請求：POST /api/v1/enrollments
#[derive(Deserialize)]
pub struct EnrollRequest {
    pub course_id: i32,
}

/// 加退選的回應格式（成功與失敗共用，靠 `success` 欄位區分）
#[derive(Serialize)]
pub struct EnrollResponse {
    pub success: bool,
    pub message: String,
}

/// 退選請求：DELETE /api/v1/enrollments
#[derive(Deserialize)]
pub struct DropRequest {
    pub course_id: i32,
}

/// POST /api/v1/enrollments：學生加選課程
///
/// 一連串檢查皆須通過才寫入：重複選課 → 時間衝突 → 名額是否額滿。
pub async fn enroll_course(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<EnrollRequest>,
) -> Result<Json<EnrollResponse>, AppError> {
    // 僅學生角色可選課
    if !auth.is_student() {
        return Err(AppError::forbidden("只有學生可以選課"));
    }

    // 開啟交易：後續寫入（選課＋名額＋成績）保持原子性
    let tx = state.db.begin().await?;

    // 1. 檢查是否已選修過此課程
    let already_enrolled = enrollments::Entity::find()
        .filter(enrollments::Column::StudentId.eq(auth.user_id))
        .filter(enrollments::Column::CourseId.eq(payload.course_id))
        .one(&tx)
        .await?;

    if let Some(e) = already_enrolled {
        // 「已退選不可重選」：DROPPED 狀態也不允許再次加選
        if e.status == "ENROLLED" {
            return Ok(Json(EnrollResponse {
                success: false,
                message: "您已選修過此課程".into(),
            }));
        }
        if e.status == "DROPPED" {
            return Ok(Json(EnrollResponse {
                success: false,
                message: "已退選課程，請聯繫教務處重新選課".into(),
            }));
        }
    }

    // 2. 檢查與已選課程的時間衝突（同一天、節次範圍重疊即衝突）
    let target_schedules = class_schedules::Entity::find()
        .filter(class_schedules::Column::CourseId.eq(payload.course_id))
        .all(&tx)
        .await?;

    let current_enrollments = enrollments::Entity::find()
        .filter(enrollments::Column::StudentId.eq(auth.user_id))
        .filter(enrollments::Column::Status.eq("ENROLLED"))
        .all(&tx)
        .await?;

    for current in &current_enrollments {
        let current_schedules = class_schedules::Entity::find()
            .filter(class_schedules::Column::CourseId.eq(current.course_id))
            .all(&tx)
            .await?;

        for cs in &current_schedules {
            for ts in &target_schedules {
                if cs.day_of_week == ts.day_of_week
                    && cs.start_period <= ts.end_period
                    && ts.start_period <= cs.end_period
                {
                    return Ok(Json(EnrollResponse {
                        success: false,
                        message: format!(
                            "課程時間衝突！與已選課程在星期 {} 第 {}~{} 節重疊",
                            cs.day_of_week, cs.start_period, cs.end_period
                        ),
                    }));
                }
            }
        }
    }

    // 3. 檢查名額：選滿即拒絕
    let course = courses::Entity::find_by_id(payload.course_id)
        .one(&tx)
        .await?
        .ok_or_else(|| AppError::not_found("課程不存在"))?;

    if course.enrolled_count >= course.capacity {
        return Ok(Json(EnrollResponse {
            success: false,
            message: "選課失敗：課程名額已滿".into(),
        }));
    }

    // 4. 名額 +1
    courses::Entity::update(courses::ActiveModel {
        course_id: Set(course.course_id),
        enrolled_count: Set(course.enrolled_count + 1),
        ..Default::default()
    })
    .exec(&tx)
    .await?;

    // 5. 寫入選課紀錄（狀態 ENROLLED）
    let enrollment = enrollments::ActiveModel {
        student_id: Set(auth.user_id),
        course_id: Set(payload.course_id),
        status: Set("ENROLLED".into()),
        ..Default::default()
    }
    .insert(&tx)
    .await?;

    // 6. 同時建立一筆空的成績紀錄，供教師日後登錄
    grades::ActiveModel {
        enrollment_id: Set(enrollment.enrollment_id),
        ..Default::default()
    }
    .insert(&tx)
    .await?;

    // 全部成功才一起提交
    tx.commit().await?;

    Ok(Json(EnrollResponse {
        success: true,
        message: "加選成功！".into(),
    }))
}

/// DELETE /api/v1/enrollments：學生退選課程
pub async fn drop_course(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(payload): Json<DropRequest>,
) -> Result<Json<EnrollResponse>, AppError> {
    // 僅學生角色可退選
    if !auth.is_student() {
        return Err(AppError::forbidden("只有學生可以退選"));
    }

    let tx = state.db.begin().await?;

    // 找出 ENROLLED 的選課紀錄，不存在則報錯
    let enrollment = enrollments::Entity::find()
        .filter(enrollments::Column::StudentId.eq(auth.user_id))
        .filter(enrollments::Column::CourseId.eq(payload.course_id))
        .filter(enrollments::Column::Status.eq("ENROLLED"))
        .one(&tx)
        .await?
        .ok_or_else(|| AppError::not_found("未找到選課紀錄"))?;

    // 1. 將狀態改為 DROPPED（保留歷史紀錄，用以阻止事後重選）
    enrollments::ActiveModel {
        enrollment_id: Set(enrollment.enrollment_id),
        status: Set("DROPPED".into()),
        ..Default::default()
    }
    .update(&tx)
    .await?;

    // 2. 課程名額 -1
    let course = courses::Entity::find_by_id(payload.course_id)
        .one(&tx)
        .await?
        .ok_or_else(|| AppError::not_found("課程不存在"))?;

    if course.enrolled_count > 0 {
        courses::Entity::update(courses::ActiveModel {
            course_id: Set(course.course_id),
            enrolled_count: Set(course.enrolled_count - 1),
            ..Default::default()
        })
        .exec(&tx)
        .await?;
    }

    // 3. 一併刪除對應的成績紀錄
    if let Some(g) = grades::Entity::find()
        .filter(grades::Column::EnrollmentId.eq(enrollment.enrollment_id))
        .one(&tx)
        .await?
    {
        g.delete(&tx).await?;
    }

    tx.commit().await?;

    Ok(Json(EnrollResponse {
        success: true,
        message: "退選成功！".into(),
    }))
}

/// 課表項目：一門課的一筆時段（跨節次課程可能有多筆）
#[derive(Serialize)]
pub struct ScheduleItem {
    pub course_code: String,
    pub course_name: String,
    pub teacher_name: String,
    pub day_of_week: i32,
    pub start_period: i32,
    pub end_period: i32,
    pub location: String,
}

/// GET /api/v1/students/me/schedule：回傳學生指定學期的個人課表
pub async fn get_student_schedule(
    State(state): State<AppState>,
    auth: AuthUser,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> Result<Json<Vec<ScheduleItem>>, AppError> {
    // 僅學生角色可查看自己的課表
    if !auth.is_student() {
        return Err(AppError::forbidden("只有學生可以查看課表"));
    }

    // 學年／學期可選參數，缺省為 113 學年第 1 學期
    let year: i32 = params
        .get("year")
        .and_then(|v| v.parse().ok())
        .unwrap_or(113);
    let semester: i32 = params
        .get("semester")
        .and_then(|v| v.parse().ok())
        .unwrap_or(1);

    // 取得目前全部 ENROLLED 的選課
    let enrollments = enrollments::Entity::find()
        .filter(enrollments::Column::StudentId.eq(auth.user_id))
        .filter(enrollments::Column::Status.eq("ENROLLED"))
        .all(&state.db)
        .await?;

    let mut items = Vec::new();

    // 每門課展開成一筆筆時段項目，並補上教師名
    for e in &enrollments {
        let course = courses::Entity::find_by_id(e.course_id)
            .one(&state.db)
            .await?;

        if let Some(c) = course {
            // 只保留指定學期的課程
            if c.academic_year != year || c.semester != semester {
                continue;
            }

            let teacher = crate::entities::users::Entity::find_by_id(c.teacher_id)
                .one(&state.db)
                .await?;

            let schedules = class_schedules::Entity::find()
                .filter(class_schedules::Column::CourseId.eq(c.course_id))
                .all(&state.db)
                .await?;

            for s in schedules {
                items.push(ScheduleItem {
                    course_code: c.course_code.clone(),
                    course_name: c.course_name.clone(),
                    teacher_name: teacher
                        .as_ref()
                        .map(|t| t.full_name.clone())
                        .unwrap_or_default(),
                    day_of_week: s.day_of_week,
                    start_period: s.start_period,
                    end_period: s.end_period,
                    location: s.location.clone(),
                });
            }
        }
    }

    // 依星期、節次排序，方便前端直接渲染週課表
    items.sort_by(|a, b| {
        a.day_of_week
            .cmp(&b.day_of_week)
            .then(a.start_period.cmp(&b.start_period))
    });

    Ok(Json(items))
}
