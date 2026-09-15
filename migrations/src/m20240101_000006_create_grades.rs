//! Migration 0006：建立成績（grades）表，選課紀錄刪除時級聯清除成績。

use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::pk_auto;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// 建立表格：enrollment_id 唯一、成績欄位可空、is_submitted 預設 false
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Grades::Table)
                    .if_not_exists()
                    .col(pk_auto(Grades::GradeId))
                    .col(ColumnDef::new(Grades::EnrollmentId).integer().not_null().unique_key())
                    .col(ColumnDef::new(Grades::MidtermScore).double())
                    .col(ColumnDef::new(Grades::FinalScore).double())
                    .col(ColumnDef::new(Grades::TotalScore).double())
                    .col(ColumnDef::new(Grades::IsSubmitted).boolean().default(false))
                    .col(ColumnDef::new(Grades::UpdatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    // 外鍵：退選刪掉選課紀錄時一併刪成績
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_grades_enrollment")
                            .from(Grades::Table, Grades::EnrollmentId)
                            .to(Enrollments::Table, Enrollments::EnrollmentId)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await
    }

    /// 反向：刪除表格
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Grades::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Grades {
    Table,
    GradeId,
    EnrollmentId,
    MidtermScore,
    FinalScore,
    TotalScore,
    IsSubmitted,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Enrollments {
    Table,
    EnrollmentId,
}
