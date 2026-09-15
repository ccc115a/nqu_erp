//! 課程（courses）entity：一門課的基本資料、授課教師、且額與目前選課人數。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "courses")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub course_id: i32,
    /// 課號（如 CSIE301）
    #[sea_orm(length = 20)]
    pub course_code: String,
    /// 開課學年（如 113 = 113 學年度）
    pub academic_year: i32,
    /// 開課學期（1 或 2）
    pub semester: i32,
    /// 課程名稱
    #[sea_orm(length = 100)]
    pub course_name: String,
    /// 授課教師（FK → users.user_id）
    pub teacher_id: i32,
    /// 學分數
    pub credits: i32,
    /// 開放名額上限
    pub capacity: i32,
    /// 目前已選人數（選課時原子增減）
    pub enrolled_count: i32,
    /// 開課科系（FK → departments.dept_id）
    pub dept_id: i32,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    /// 課程的授課教師
    #[sea_orm(
        belongs_to = "super::users::Entity",
        from = "Column::TeacherId",
        to = "super::users::Column::UserId"
    )]
    Teacher,
    /// 課程歸屬於開課科系
    #[sea_orm(
        belongs_to = "super::departments::Entity",
        from = "Column::DeptId",
        to = "super::departments::Column::DeptId"
    )]
    Department,
    /// 課程可能有多個上課時段
    #[sea_orm(has_many = "super::class_schedules::Entity")]
    Schedules,
    /// 課程被多筆選課紀錄引用
    #[sea_orm(has_many = "super::enrollments::Entity")]
    Enrollments,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Teacher.def()
    }
}

impl Related<super::departments::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Department.def()
    }
}

impl Related<super::class_schedules::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Schedules.def()
    }
}

impl Related<super::enrollments::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Enrollments.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
