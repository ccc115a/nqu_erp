//! 統一錯誤處理：定義 `AppError` 型別，將業務／資料庫／JWT 錯誤轉成 HTTP 回應。

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// 應用程式統一錯誤型別：HTTP status + 中文錯誤訊息
pub struct AppError {
    pub status: StatusCode,
    pub message: String,
}

impl AppError {
    /// 直接以 status＋訊息建立錯誤
    pub fn new(status: StatusCode, message: impl Into<String>) -> Self {
        Self {
            status,
            message: message.into(),
        }
    }

    /// 400 參數/業務錯誤（如課程時間衝突）
    pub fn bad_request(message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, message)
    }

    /// 401 未登入或登入失敗
    pub fn unauthorized(message: impl Into<String>) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, message)
    }

    /// 403 沒有權限執行此操作（角色不符）
    pub fn forbidden(message: impl Into<String>) -> Self {
        Self::new(StatusCode::FORBIDDEN, message)
    }

    /// 404 資源不存在
    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, message)
    }

    /// 409 資源衝突（目前保留備用）
    #[allow(dead_code)]
    pub fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, message)
    }

    /// 500 內部錯誤（不直接向使用者洩漏細節）
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, message)
    }
}

/// 將 AppError 轉為 HTTP 回應，body 格式：`{ "error": "..." }`
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let body = json!({
            "error": self.message,
        });
        (self.status, axum::Json(body)).into_response()
    }
}

/// 資料庫錯誤一律轉為 500「資料庫錯誤」，細節僅寫入日誌
impl From<sea_orm::DbErr> for AppError {
    fn from(err: sea_orm::DbErr) -> Self {
        tracing::error!("Database error: {:?}", err);
        Self::internal("資料庫錯誤")
    }
}

/// JWT 解析錯誤視為未授權，回傳原始錯誤訊息
impl From<jsonwebtoken::errors::Error> for AppError {
    fn from(err: jsonwebtoken::errors::Error) -> Self {
        Self::unauthorized(format!("JWT 錯誤: {}", err))
    }
}
