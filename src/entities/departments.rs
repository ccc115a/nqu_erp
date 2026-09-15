//! 科系（departments）entity：記錄開課單位，學生／教師／課程皆歸屬於某科系。

use sea_orm::entity::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq, DeriveEntityModel)]
#[sea_orm(table_name = "departments")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub dept_id: i32,
    /// 科系代碼（如 CSIE），唯一
    #[sea_orm(unique, length = 10)]
    pub dept_code: String,
    /// 科系名稱（如 資訊工程學系）
    #[sea_orm(length = 50)]
    pub dept_name: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    /// 一個科系底下有多位使用者
    #[sea_orm(has_many = "super::users::Entity")]
    Users,
    /// 一個科系開設多門課程
    #[sea_orm(has_many = "super::courses::Entity")]
    Courses,
}

impl Related<super::users::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Users.def()
    }
}

impl Related<super::courses::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Courses.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
