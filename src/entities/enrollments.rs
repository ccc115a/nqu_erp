//! 選課紀錄（enrollments）entity：學生 × 課程的選修關係與狀態。
//!
//! 狀態為 ENROLLED（已選）或 DROPPED（已退選）；退選後保留紀錄以阻止重選。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "enrollments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub enrollment_id: i32,
    /// 學生（FK → users.user_id）
    pub student_id: i32,
    /// 課程（FK → courses.course_id）
    pub course_id: i32,
    /// ENROLLED / DROPPED
    #[sea_orm(length = 15)]
    pub status: String,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    /// 選課者為學生
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::StudentId",
        to = "super::users::Column::UserId"
    )]
    Student,
    /// 選的課程
    #[sea_orm(
        belongs_to = "super::courses::Entity",
        from = "Column::CourseId",
        to = "super::courses::Column::CourseId"
    )]
    Course,
    /// 每筆選課對應一筆成績
    #[sea_orm(has_one = "super::grades::Entity")]
    Grade,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Student.def()
    }
}

impl Related<super::courses::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Course.def()
    }
}

impl Related<super::grades::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Grade.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
