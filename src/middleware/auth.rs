//! 認證基礎設施：JWT 簽發／驗證，以及從 HTTP 標頭解析目前使用者的 extractor。

use axum::{
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap},
};
use jsonwebtoken::{decode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::errors::AppError;
use crate::AppState;

/// JWT payload：`sub` = 使用者 id、`role` = 角色、`exp` = 過期時間（Unix 秒）
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Claims {
    pub sub: i32,
    pub username: String,
    pub role: String,
    pub exp: usize,
}

/// 已驗證身分的使用者，作為 handler 第二個參數自動注入
#[derive(Clone)]
pub struct AuthUser {
    pub user_id: i32,
    pub username: String,
    pub role: String,
}

impl AuthUser {
    /// 是否為學生角色
    pub fn is_student(&self) -> bool {
        self.role == "STUDENT"
    }

    /// 是否為教師角色
    pub fn is_teacher(&self) -> bool {
        self.role == "TEACHER"
    }

    /// 是否為管理員角色
    pub fn is_admin(&self) -> bool {
        self.role == "ADMIN"
    }
}

/// 登入成功後簽發 JWT（有效 24 小時）
pub fn create_token(
    user_id: i32,
    username: &str,
    role: &str,
    secret: &str,
) -> Result<String, jsonwebtoken::errors::Error> {
    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(24))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        sub: user_id,
        username: username.to_string(),
        role: role.to_string(),
        exp,
    };

    encode(secret, &claims)
}

/// 依現有 claims 簽發新 token（預設 24 小時）
pub fn encode(secret: &str, claims: &Claims) -> Result<String, jsonwebtoken::errors::Error> {
    encode_with_duration(secret, claims, 24)
}

/// 以指定有效時數簽發 token（供測試彈性調整過期時間）
pub fn encode_with_duration(
    secret: &str,
    claims: &Claims,
    hours: i64,
) -> Result<String, jsonwebtoken::errors::Error> {
    let exp = chrono::Utc::now()
        .checked_add_signed(chrono::Duration::hours(hours))
        .expect("valid timestamp")
        .timestamp() as usize;

    let claims = Claims {
        exp,
        ..claims.clone()
    };

    jsonwebtoken::encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(secret.as_bytes()),
    )
}

/// 驗證 token 簽名與過期時間，成功回傳 claims
pub fn decode_token(token: &str, secret: &str) -> Result<Claims, jsonwebtoken::errors::Error> {
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(secret.as_bytes()),
        &Validation::default(),
    )?;
    Ok(token_data.claims)
}

/// Extract 實作：每個需要登入的 handler 宣告 `auth: AuthUser` 即可自動解析標頭
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = AppError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let headers: &HeaderMap = &parts.headers;

        // 1. 取得 Authorization 標頭
        let auth_header = headers
            .get("Authorization")
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| AppError::unauthorized("缺少 Authorization 標頭"))?;

        // 2. 剝除 "Bearer " 前綴取出 token
        let token = auth_header
            .strip_prefix("Bearer ")
            .ok_or_else(|| AppError::unauthorized("Authorization 格式錯誤，應為 Bearer <token>"))?;

        // 3. 驗證簽名與期限，失敗視為未授權
        let claims = decode_token(token, &state.jwt_secret)
            .map_err(|e| AppError::unauthorized(format!("Token 無效: {}", e)))?;

        Ok(AuthUser {
            user_id: claims.sub,
            username: claims.username,
            role: claims.role,
        })
    }
}
