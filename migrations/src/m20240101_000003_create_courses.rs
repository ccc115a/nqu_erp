//! Migration 0003：建立課程（courses）表，含學年學期複合索引。

use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::pk_auto;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// 建立表格：額額 capacity、已選人數 enrolled_count 預設 0，並建立索引
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Courses::Table)
                    .if_not_exists()
                    .col(pk_auto(Courses::CourseId))
                    .col(ColumnDef::new(Courses::CourseCode).string().not_null())
                    .col(ColumnDef::new(Courses::AcademicYear).integer().not_null())
                    .col(ColumnDef::new(Courses::Semester).integer().not_null())
                    .col(ColumnDef::new(Courses::CourseName).string().not_null())
                    .col(ColumnDef::new(Courses::TeacherId).integer())
                    .col(ColumnDef::new(Courses::Credits).integer().not_null())
                    .col(ColumnDef::new(Courses::Capacity).integer().not_null())
                    .col(ColumnDef::new(Courses::EnrolledCount).integer().default(0))
                    .col(ColumnDef::new(Courses::DeptId).integer())
                    // 授課教師外鍵
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_courses_teacher")
                            .from(Courses::Table, Courses::TeacherId)
                            .to(Users::Table, Users::UserId),
                    )
                    // 開課科系外鍵
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_courses_dept")
                            .from(Courses::Table, Courses::DeptId)
                            .to(Departments::Table, Departments::DeptId),
                    )
                    .to_owned(),
            )
            .await?;

        // 依學年＋學期查詢課程是主要查詢路徑，建立複合索引加速
        manager
            .create_index(
                Index::create()
                    .name("idx_courses_year_sem")
                    .table(Courses::Table)
                    .col(Courses::AcademicYear)
                    .col(Courses::Semester)
                    .to_owned(),
            )
            .await
    }

    /// 反向：先刪索引再刪表
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(Index::drop().name("idx_courses_year_sem").to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Courses::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Courses {
    Table,
    CourseId,
    CourseCode,
    AcademicYear,
    Semester,
    CourseName,
    TeacherId,
    Credits,
    Capacity,
    EnrolledCount,
    DeptId,
}

#[derive(DeriveIden)]
enum Users {
    Table,
    UserId,
}

#[derive(DeriveIden)]
enum Departments {
    Table,
    DeptId,
}
