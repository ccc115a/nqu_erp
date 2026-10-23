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

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    async fn body_json(response: Response) -> serde_json::Value {
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("collect body")
            .to_bytes();
        serde_json::from_slice(&bytes).expect("invalid json body")
    }

    #[tokio::test]
    async fn builder_sets_expected_status() {
        assert_eq!(AppError::bad_request("x").status, StatusCode::BAD_REQUEST);
        assert_eq!(AppError::unauthorized("x").status, StatusCode::UNAUTHORIZED);
        assert_eq!(AppError::forbidden("x").status, StatusCode::FORBIDDEN);
        assert_eq!(AppError::not_found("x").status, StatusCode::NOT_FOUND);
        assert_eq!(AppError::conflict("x").status, StatusCode::CONFLICT);
        assert_eq!(
            AppError::internal("x").status,
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }

    #[tokio::test]
    async fn into_response_wraps_error_in_json_body() {
        let response = AppError::bad_request("課程時間衝突！").into_response();

        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let json = body_json(response).await;
        assert_eq!(json["error"], "課程時間衝突！");
    }

    #[tokio::test]
    async fn db_error_becomes_internal_error() {
        let err = AppError::from(sea_orm::DbErr::Custom("broken".to_owned()));

        assert_eq!(err.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(err.message, "資料庫錯誤");
    }

    #[tokio::test]
    async fn jwt_error_becomes_unauthorized() {
        let jwt_err = crate::middleware::auth::decode_token("garbage", "secret").unwrap_err();
        let err = AppError::from(jwt_err);

        assert_eq!(err.status, StatusCode::UNAUTHORIZED);
        assert!(err.message.starts_with("JWT 錯誤"));
    }
}
