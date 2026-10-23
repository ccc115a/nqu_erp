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
    /// CORS 允許的來源，缺省為前端 dev server；設為 `*` 表示全部放行（docker 同源部署用）
    pub cors_origin: String,
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
            cors_origin: env::var("CORS_ORIGIN").unwrap_or_else(|_| "http://localhost:5173".into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 環境變數為 process 全域狀態，用 Mutex 序列化會讀寫環境變數的測試
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn from_env_uses_overridden_values() {
        let _guard = ENV_LOCK.lock().unwrap();

        std::env::set_var("DATABASE_URL", "postgres://test/test_db");
        std::env::set_var("JWT_SECRET", "overridden-secret");
        std::env::set_var("SERVER_ADDR", "127.0.0.1:9999");
        std::env::set_var("CORS_ORIGIN", "http://test.example");

        let config = Config::from_env();

        assert_eq!(config.database_url, "postgres://test/test_db");
        assert_eq!(config.jwt_secret, "overridden-secret");
        assert_eq!(config.server_addr, "127.0.0.1:9999");
        assert_eq!(config.cors_origin, "http://test.example");

        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("JWT_SECRET");
        std::env::remove_var("SERVER_ADDR");
        std::env::remove_var("CORS_ORIGIN");
    }

    #[test]
    fn from_env_falls_back_to_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();

        std::env::remove_var("DATABASE_URL");
        std::env::remove_var("JWT_SECRET");
        std::env::remove_var("SERVER_ADDR");
        std::env::remove_var("CORS_ORIGIN");

        let config = Config::from_env();

        assert_eq!(config.database_url, "sqlite://dev.db?mode=rwc");
        assert!(!config.jwt_secret.is_empty());
        assert_eq!(config.server_addr, "0.0.0.0:8080");
        assert_eq!(config.cors_origin, "http://localhost:5173");
    }
}
