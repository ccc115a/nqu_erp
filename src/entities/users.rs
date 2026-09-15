//! 使用者（users）entity：學生／教師／管理員皆存放在此表，以 `role` 區分。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub user_id: i32,
    /// 登入帳號（學號為 113XXXXXX、教師為 T001、管理員為 admin）
    #[sea_orm(unique, length = 20)]
    pub username: String,
    /// bcrypt 雜湊後的密碼，絕不存明碼
    #[sea_orm(length = 255)]
    pub password_hash: String,
    /// 中文姓名
    #[sea_orm(length = 50)]
    pub full_name: String,
    /// 角色：STUDENT / TEACHER / ADMIN
    #[sea_orm(length = 10)]
    pub role: String,
    /// 所屬科系（FK → departments.dept_id）
    pub dept_id: i32,
    /// 電子郵件，唯一
    #[sea_orm(unique, length = 100)]
    pub email: String,
    pub created_at: DateTimeUtc,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    /// 使用者歸屬於某科系
    #[sea_orm(
        belongs_to = "super::departments::Entity",
        from = "Column::DeptId",
        to = "super::departments::Column::DeptId"
    )]
    Department,
    /// 教師開設的多門課程
    #[sea_orm(has_many = "super::courses::Entity")]
    CoursesAsTeacher,
    /// 學生的選課紀錄
    #[sea_orm(has_many = "super::enrollments::Entity")]
    Enrollments,
}

impl Related<super::departments::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Department.def()
    }
}

impl Related<super::courses::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::CoursesAsTeacher.def()
    }
}

impl Related<super::enrollments::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Enrollments.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
