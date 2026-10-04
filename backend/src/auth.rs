use crate::app::{AppError, AppResult, AppState};
use axum::async_trait;
use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use chrono::{Duration, Utc};
use hex::encode;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub username: String,
    pub role: String, // staff | owner
    pub display_name: String,
}

pub fn hash_password(salt: &str, password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"petcare-v1:");
    hasher.update(salt.as_bytes());
    hasher.update(password.as_bytes());
    encode(hasher.finalize())
}

fn hash_token(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(b"petcare-session:");
    hasher.update(token.as_bytes());
    encode(hasher.finalize())
}

pub fn new_id() -> String {
    Uuid::new_v4().to_string()
}

pub fn now_iso() -> String {
    Utc::now().to_rfc3339()
}

pub fn future_iso(hours: i64) -> String {
    (Utc::now() + Duration::hours(hours)).to_rfc3339()
}

/// 登录成功后创建会话，返回明文 token（仅这一次返回）
pub async fn create_session(state: &AppState, user_id: &str) -> AppResult<String> {
    let token = Uuid::new_v4().to_string().replace('-', "");
    let token_hash = hash_token(&token);
    sqlx::query(
        "INSERT INTO sessions (token_hash, user_id, expires_at, created_at) VALUES (?, ?, ?, ?)",
    )
    .bind(&token_hash)
    .bind(user_id)
    .bind(future_iso(72))
    .bind(now_iso())
    .execute(state.pool())
    .await?;
    Ok(token)
}

/// 从 Authorization: Bearer <token> 或 X-Auth-Token 解析当前用户
async fn user_from_parts(parts: &Parts, state: &AppState) -> AppResult<User> {
    let token = parts
        .headers
        .get("authorization")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .or_else(|| {
            parts
                .headers
                .get("x-auth-token")
                .and_then(|v| v.to_str().ok())
        })
        .ok_or_else(|| AppError::unauthorized("缺少登录凭证"))?;

    let token_hash = hash_token(token);
    let row: SqliteRow = sqlx::query(
        "SELECT u.id, u.username, u.role, u.display_name
         FROM sessions s JOIN users u ON u.id = s.user_id
         WHERE s.token_hash = ? AND s.expires_at > ?",
    )
    .bind(&token_hash)
    .bind(now_iso())
    .fetch_optional(state.pool())
    .await?
    .ok_or_else(|| AppError::unauthorized("登录已失效，请重新登录"))?;

    Ok(User {
        id: row.get("id"),
        username: row.get("username"),
        role: row.get("role"),
        display_name: row.get("display_name"),
    })
}

/// 已登录用户提取器
#[async_trait]
impl FromRequestParts<AppState> for User {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        user_from_parts(parts, state).await
    }
}

/// 门店员工提取器（owner 直接 403）
pub struct Staff(pub User);

#[async_trait]
impl FromRequestParts<AppState> for Staff {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let user = user_from_parts(parts, state).await?;
        if user.role != "staff" {
            return Err(AppError::forbidden("仅门店员工可执行该操作"));
        }
        Ok(Staff(user))
    }
}

impl Staff {
    pub fn id(&self) -> &str {
        &self.0.id
    }
    pub fn user(&self) -> &User {
        &self.0
    }
}

/// 宠物主人提取器
pub struct Owner(pub User);

#[async_trait]
impl FromRequestParts<AppState> for Owner {
    type Rejection = AppError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let user = user_from_parts(parts, state).await?;
        if user.role != "owner" {
            return Err(AppError::forbidden("仅宠物主人可执行该操作"));
        }
        Ok(Owner(user))
    }
}

/// 校验当前用户有权访问该宠物：主人只能访问自己的，员工均可
pub async fn require_pet_access(state: &AppState, user: &User, pet_id: &str) -> AppResult<()> {
    let owner_id: Option<String> = sqlx::query_scalar("SELECT owner_id FROM pets WHERE id = ?")
        .bind(pet_id)
        .fetch_optional(state.pool())
        .await?
        .flatten();
    let owner_id = owner_id.ok_or_else(|| AppError::not_found("宠物不存在"))?;
    if user.role == "owner" && owner_id != user.id {
        // 主人查看他人宠物：按不存在处理，避免泄露
        return Err(AppError::not_found("宠物不存在"));
    }
    Ok(())
}
