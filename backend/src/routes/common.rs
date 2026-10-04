use crate::app::{AppError, AppResult, AppState};
use crate::auth::{new_id, now_iso, Staff};
use axum::extract::{Multipart, State};
use axum::Json;
use serde_json::{json, Value};
use std::path::Path;
use uuid::Uuid;

/// 写入一条本地模拟通知。通知同时打印到服务端日志，模拟推送给主人。
pub async fn notify(
    state: &AppState,
    owner_id: &str,
    kind: &str,
    title: &str,
    body: &str,
    pet_id: Option<&str>,
    incident_id: Option<&str>,
) -> AppResult<String> {
    let id = new_id();
    sqlx::query(
        "INSERT INTO notifications
            (id, owner_id, kind, title, body, pet_id, incident_id, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(owner_id)
    .bind(kind)
    .bind(title)
    .bind(body)
    .bind(pet_id)
    .bind(incident_id)
    .bind(now_iso())
    .execute(state.pool())
    .await?;

    tracing::info!(
        "[本地模拟通知->主人 {}] {} | {} ({})",
        owner_id,
        title,
        body,
        kind
    );
    Ok(id)
}

/// 查询宠物主人
pub async fn pet_owner_id(state: &AppState, pet_id: &str) -> AppResult<String> {
    sqlx::query_scalar("SELECT owner_id FROM pets WHERE id = ?")
        .bind(pet_id)
        .fetch_optional(state.pool())
        .await?
        .flatten()
        .ok_or_else(|| AppError::not_found("宠物不存在"))
}

/// 照片上传：multipart 字段 file，保存到本地 uploads，返回可访问 URL
pub async fn upload_photo(
    State(state): State<AppState>,
    _staff: Staff,
    mut multipart: Multipart,
) -> AppResult<Json<Value>> {
    let mut saved: Option<String> = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| AppError::bad_request(&format!("上传数据解析失败: {e}")))?
    {
        let name = field.name().unwrap_or("").to_string();
        if name != "file" {
            continue;
        }
        let original = field.file_name().unwrap_or("photo").to_string();
        let ext = Path::new(&original)
            .extension()
            .and_then(|e| e.to_str())
            .filter(|e| e.chars().all(|c| c.is_ascii_alphanumeric()) && e.len() <= 6)
            .unwrap_or("jpg");
        let filename = format!("{}.{}", Uuid::new_v4().simple(), ext);
        let path = state.inner.upload_dir.join(&filename);
        let data = field
            .bytes()
            .await
            .map_err(|e| AppError::bad_request(&format!("读取照片失败: {e}")))?;
        if data.len() > 8 * 1024 * 1024 {
            return Err(AppError::bad_request("照片不能超过 8MB"));
        }
        tokio::fs::write(&path, &data).await?;
        saved = Some(filename);
    }
    let filename = saved.ok_or_else(|| AppError::bad_request("缺少 file 字段"))?;
    Ok(Json(json!({ "path": filename, "url": format!("/uploads/{filename}") })))
}
