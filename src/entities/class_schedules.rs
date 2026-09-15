//! 上課時間表（class_schedules）entity：記錄每門課在星期幾、第幾節、何地上課。

use sea_orm::entity::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, DeriveEntityModel, Serialize, Deserialize)]
#[sea_orm(table_name = "class_schedules")]
pub struct Model {
    #[sea_orm(primary_key, auto_increment = true)]
    pub schedule_id: i32,
    /// 所屬課程（FK → courses.course_id）
    pub course_id: i32,
    /// 星期幾（1 = 週一 … 7 = 週日）
    pub day_of_week: i32,
    /// 開始節次（1 起算）
    pub start_period: i32,
    /// 結束節次（含，可能跨多節）
    pub end_period: i32,
    /// 上課地點
    #[sea_orm(length = 50)]
    pub location: String,
}

#[derive(Copy, Clone, Debug, EnumIter, DeriveRelation)]
pub enum Relation {
    /// 每個時段隸屬於一門課程
    #[sea_orm(
        belongs_to = "super::courses::Entity",
        from = "Column::CourseId",
        to = "super::courses::Column::CourseId"
    )]
    Course,
}

impl Related<super::courses::Entity> for Entity {
    fn to() -> RelationDef {
        Relation::Course.def()
    }
}

impl ActiveModelBehavior for ActiveModel {}
