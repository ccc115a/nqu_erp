//! 應用程式設定：從環境變數（或 `.env`）讀取資料庫連線、JWT 密鑰與監聽位址。

use std::env;

/// 應用程式設定，全部從環境變數讀取（`.env` 已 gitignore）
#[derive(Debug, Clone)]
pub struct Config {
    /// 資料庫連線字串（`sqlite://dev.db?mode=rwc` 或 PostgreSQL URL）
    pub database_url: String,
    /// 簽發/驗證 JWT 用的密鑰
    pub jwt_secret: String,
    /// HTTP server 監聽位址，例如 `0.0.0.0:8080`
    pub server_addr: String,
}

impl Config {
    /// 從 `.env`／環境變數建立設定，缺省時使用開發預設值
    pub fn from_env() -> Self {
        // 載入 `.env`（若存在），不影響已設置的系統環境變數
        dotenvy::dotenv().ok();

        Self {
            database_url: env::var("DATABASE_URL")
                .unwrap_or_else(|_| "sqlite://dev.db?mode=rwc".into()),
            jwt_secret: env::var("JWT_SECRET")
                .unwrap_or_else(|_| "nqu-erp-secret-key-change-in-production".into()),
            server_addr: env::var("SERVER_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".into()),
        }
    }
}
