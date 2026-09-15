//! Migration 0002：建立使用者（users）表。

use sea_orm_migration::prelude::*;
use sea_orm_migration::schema::pk_auto;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    /// 建立表格：帳號／email 唯一、密碼只存雜湊、created_at 預設目前時間
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(Users::Table)
                    .if_not_exists()
                    .col(pk_auto(Users::UserId))
                    .col(ColumnDef::new(Users::Username).string().not_null().unique_key())
                    .col(ColumnDef::new(Users::PasswordHash).string().not_null())
                    .col(ColumnDef::new(Users::FullName).string().not_null())
                    .col(ColumnDef::new(Users::Role).string().not_null())
                    .col(ColumnDef::new(Users::DeptId).integer())
                    .col(ColumnDef::new(Users::Email).string().not_null().unique_key())
                    .col(ColumnDef::new(Users::CreatedAt).timestamp_with_time_zone().default(Expr::current_timestamp()))
                    // 外鍵：使用者隸屬某科系
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_users_dept")
                            .from(Users::Table, Users::DeptId)
                            .to(Departments::Table, Departments::DeptId),
                    )
                    .to_owned(),
            )
            .await
    }

    /// 反向：刪除表格
    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(Users::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum Users {
    Table,
    UserId,
    Username,
    PasswordHash,
    FullName,
    Role,
    DeptId,
    Email,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Departments {
    Table,
    DeptId,
}
