//! 資料庫連線與 migration：負責建立連線並把資料表結構升級到最新版本。

use sea_orm::{Database, DatabaseConnection};
use sea_orm_migration::prelude::*;

use crate::config::Config;

/// 依照設定建立資料庫連線（不負責建立資料表）
pub async fn establish_connection(config: &Config) -> Result<DatabaseConnection, sea_orm::DbErr> {
    let db = Database::connect(&config.database_url).await?;
    Ok(db)
}

/// 執行所有尚未執行的 migration，確保資料表結構為最新版本
pub async fn run_migrations(db: &DatabaseConnection) -> Result<(), sea_orm::DbErr> {
    use migrations::Migrator;
    Migrator::up(db, None).await?;
    Ok(())
}
