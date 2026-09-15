//! 成績（grades）entity：每筆選課對應一列成績，含期中／期末／加權總分。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "grades")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub grade_id: i32,
    /// 對應的選課紀錄（FK → enrollments.enrollment_id），唯一
    #[sea_orm(unique)]
    pub enrollment_id: i32,
    /// 期中成績（未登錄為 NULL）
    pub midterm_score: Option<f64>,
    /// 期末成績
    pub final_score: Option<f64>,
    /// 加權總分 = 期中×0.4 + 期末×0.6
    pub total_score: Option<f64>,
    /// 教師是否已鎖定送交（送交後不可再改）
    pub is_submitted: bool,
    pub updated_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    /// 成績隸屬於某筆選課
    #[sea_orm(
        belongs_to = "super::enrollments::Entity",
        from = "Column::EnrollmentId",
        to = "super::enrollments::Column::EnrollmentId"
    )]
    Enrollment,
}

impl Related<super::enrollments::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Enrollment.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
