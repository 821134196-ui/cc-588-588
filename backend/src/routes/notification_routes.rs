use crate::app::{AppError, AppResult, AppState};
use crate::auth::{User};
use axum::extract::{Path, State};
use axum::Json;
use serde_json::{json, Value};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

/// 通知列表：主人看自己的；员工看不到主人通知
pub async fn list_notifications(
    State(state): State<AppState>,
    user: User,
) -> AppResult<Json<Value>> {
    if user.role != "owner" {
        return Ok(Json(json!({ "notifications": [] })));
    }
    let rows = sqlx::query(
        "SELECT * FROM notifications WHERE owner_id = ?
         ORDER BY created_at DESC LIMIT 200",
    )
    .bind(&user.id)
    .fetch_all(state.pool())
    .await?;

    let unread = rows.iter().filter(|r| r.try_get::<Option<String>, _>("read_at").ok().flatten().is_none()).count();
    let items: Vec<Value> = rows.iter().map(notif_json).collect();
    Ok(Json(json!({ "notifications": items, "unread": unread })))
}

fn notif_json(r: &SqliteRow) -> Value {
    json!({
        "id": r.get::<String,_>("id"),
        "kind": r.get::<String,_>("kind"),
        "title": r.get::<String,_>("title"),
        "body": r.get::<String,_>("body"),
        "pet_id": r.get::<Option<String>,_>("pet_id"),
        "incident_id": r.get::<Option<String>,_>("incident_id"),
        "read_at": r.get::<Option<String>,_>("read_at"),
        "created_at": r.get::<String,_>("created_at"),
    })
}

pub async fn mark_read(
    State(state): State<AppState>,
    user: User,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    // 只能标记自己的通知为已读
    let res = sqlx::query(
        "UPDATE notifications SET read_at = ?
         WHERE id = ? AND owner_id = ? AND read_at IS NULL",
    )
    .bind(crate::auth::now_iso())
    .bind(&id)
    .bind(&user.id)
    .execute(state.pool())
    .await?;
    if res.rows_affected() == 0 {
        // 可能已读或不属于自己；不属于自己按 404
        let owner: Option<String> =
            sqlx::query_scalar("SELECT owner_id FROM notifications WHERE id = ?")
                .bind(&id)
                .fetch_optional(state.pool())
                .await?
                .flatten();
        if owner.as_deref() != Some(user.id.as_str()) {
            return Err(AppError::not_found("通知不存在"));
        }
    }
    Ok(Json(json!({ "ok": true })))
}

pub async fn read_all(
    State(state): State<AppState>,
    user: User,
) -> AppResult<Json<Value>> {
    sqlx::query(
        "UPDATE notifications SET read_at = ?
         WHERE owner_id = ? AND read_at IS NULL",
    )
    .bind(crate::auth::now_iso())
    .bind(&user.id)
    .execute(state.pool())
    .await?;
    Ok(Json(json!({ "ok": true })))
}
