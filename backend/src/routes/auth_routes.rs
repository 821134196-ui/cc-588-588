use crate::app::{AppError, AppResult, AppState};
use crate::auth::{create_session, hash_password, User};
use axum::extract::State;
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[derive(Deserialize)]
pub struct LoginReq {
    username: String,
    password: String,
}

pub async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginReq>,
) -> AppResult<Json<Value>> {
    let row = sqlx::query_as::<_, (String, String, String, String, String)>(
        "SELECT id, username, password_salt, password_hash, role FROM users WHERE username = ?",
    )
    .bind(&req.username)
    .fetch_optional(state.pool())
    .await?;

    // 统一错误信息，避免枚举用户名
    let (id, _username, salt, stored_hash, role) =
        row.ok_or_else(|| AppError::unauthorized("用户名或密码错误"))?;

    let input_hash = hash_password(&salt, &req.password);
    if !constant_time_eq(input_hash.as_bytes(), stored_hash.as_bytes()) {
        return Err(AppError::unauthorized("用户名或密码错误"));
    }

    let token = create_session(&state, &id).await?;
    let display_name: String =
        sqlx::query_scalar("SELECT display_name FROM users WHERE id = ?")
            .bind(&id)
            .fetch_one(state.pool())
            .await?;

    Ok(Json(json!({
        "token": token,
        "user": { "id": id, "username": req.username, "role": role, "display_name": display_name }
    })))
}

pub async fn me(user: User) -> Json<Value> {
    Json(json!({
        "id": user.id,
        "username": user.username,
        "role": user.role,
        "display_name": user.display_name,
    }))
}

#[derive(Deserialize)]
pub struct LogoutReq {
    token: Option<String>,
}

pub async fn logout(
    State(state): State<AppState>,
    user: User,
    Json(req): Json<LogoutReq>,
) -> AppResult<Json<Value>> {
    if let Some(token) = req.token {
        let mut hasher = Sha256::new();
        hasher.update(b"petcare-session:");
        hasher.update(token.as_bytes());
        let hash = hex::encode(hasher.finalize());
        sqlx::query("DELETE FROM sessions WHERE token_hash = ? AND user_id = ?")
            .bind(&hash)
            .bind(&user.id)
            .execute(state.pool())
            .await?;
    }
    Ok(Json(json!({ "ok": true })))
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        diff |= x ^ y;
    }
    diff == 0
}
