use crate::app::{AppError, AppResult, AppState};
use crate::auth::{new_id, now_iso, require_pet_access, Owner, Staff, User};
use axum::extract::{Path, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use uuid::Uuid;

fn hash_code(code: &str) -> String {
    let mut h = Sha256::new();
    h.update(b"petcare-pickup:");
    h.update(code.as_bytes());
    hex::encode(h.finalize())
}

/// 生成 8 位数字取宠码：从随机 UUID 派生
fn generate_code() -> String {
    let u = Uuid::new_v4();
    let mut h = Sha256::new();
    h.update(b"petcare-code-gen:");
    h.update(u.as_bytes());
    let digest = h.finalize();
    let mut n: u64 = 0;
    for b in digest.iter().take(8) {
        n = (n << 8) | (*b as u64);
    }
    format!("{:08}", n % 100_000_000)
}

// ---------- 列出授权 ----------

pub async fn list_authorizations(
    State(state): State<AppState>,
    user: User,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    require_pet_access(&state, &user, &id).await?;
    let rows = sqlx::query(
        "SELECT a.*, u.display_name AS staff_name
         FROM pickup_authorizations a
         LEFT JOIN users u ON u.id = a.used_by_staff
         WHERE a.pet_id = ? ORDER BY a.created_at DESC",
    )
    .bind(&id)
    .fetch_all(state.pool())
    .await?;

    let items: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String,_>("id"),
                "pet_id": r.get::<String,_>("pet_id"),
                "proxy_name": r.get::<String,_>("proxy_name"),
                "proxy_contact": r.get::<String,_>("proxy_contact"),
                "status": r.get::<String,_>("status"),
                "expires_at": r.get::<Option<String>,_>("expires_at"),
                "used_at": r.get::<Option<String>,_>("used_at"),
                "used_by_staff": r.get::<Option<String>,_>("used_by_staff"),
                "staff_name": r.get::<Option<String>,_>("staff_name"),
                "revoke_reason": r.get::<String,_>("revoke_reason"),
                "created_at": r.get::<String,_>("created_at"),
                // 绝不返回明文码/哈希；列表只展示状态
            })
        })
        .collect();
    Ok(Json(json!({ "authorizations": items })))
}

// ---------- 主人签发代接授权 ----------

#[derive(Deserialize)]
pub struct CreateAuthReq {
    proxy_name: String,
    proxy_contact: Option<String>,
    expires_at: Option<String>, // 可选 RFC3339
}

pub async fn create_authorization(
    State(state): State<AppState>,
    owner: Owner,
    Path(id): Path<String>,
    Json(req): Json<CreateAuthReq>,
) -> AppResult<(axum::http::StatusCode, Json<Value>)> {
    if req.proxy_name.trim().is_empty() {
        return Err(AppError::bad_request("代接人姓名不能为空"));
    }
    // 只有主人本人能为自己的宠物授权
    let pet_owner: Option<String> =
        sqlx::query_scalar("SELECT owner_id FROM pets WHERE id = ?")
            .bind(&id)
            .fetch_optional(state.pool())
            .await?
            .flatten();
    let pet_owner = pet_owner.ok_or_else(|| AppError::not_found("宠物不存在"))?;
    if pet_owner != owner.0.id {
        return Err(AppError::not_found("宠物不存在"));
    }
    let pet_status: String = sqlx::query_scalar("SELECT status FROM pets WHERE id = ?")
        .bind(&id)
        .fetch_one(state.pool())
        .await?;
    if pet_status == "picked_up" {
        return Err(AppError::conflict("pet_picked_up", "宠物已接走，不能再创建代接授权"));
    }

    // 生成唯一码（极小概率碰撞时重试）
    let (auth_id, code) = {
        let auth_id = new_id();
        let mut code = generate_code();
        for _ in 0..5 {
            let exists: i64 =
                sqlx::query_scalar("SELECT COUNT(*) FROM pickup_authorizations WHERE code_hash = ?")
                    .bind(hash_code(&code))
                    .fetch_one(state.pool())
                    .await?;
            if exists == 0 {
                break;
            }
            code = generate_code();
        }
        (auth_id, code)
    };

    sqlx::query(
        "INSERT INTO pickup_authorizations
            (id, pet_id, owner_id, proxy_name, proxy_contact, code_hash, status, expires_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?, 'active', ?, ?)",
    )
    .bind(&auth_id)
    .bind(&id)
    .bind(&owner.0.id)
    .bind(req.proxy_name.trim())
    .bind(req.proxy_contact.unwrap_or_default())
    .bind(hash_code(&code))
    .bind(&req.expires_at)
    .bind(now_iso())
    .execute(state.pool())
    .await?;

    tracing::info!(owner = %owner.0.id, pet = %id, proxy = %req.proxy_name, "已签发代接授权");
    // 明文取宠码仅此一次返回，后端只保存哈希
    Ok((
        axum::http::StatusCode::CREATED,
        Json(json!({
            "id": auth_id,
            "pickup_code": code,
            "hint": "请当面或通过可信渠道把取宠码交给代接人；撤销授权后此码立即失效"
        })),
    ))
}

// ---------- 主人撤销授权（旧码立即失效） ----------

#[derive(Deserialize)]
pub struct RevokeReq {
    reason: Option<String>,
}

pub async fn revoke_authorization(
    State(state): State<AppState>,
    owner: Owner,
    Path(auth_id): Path<String>,
    Json(req): Json<RevokeReq>,
) -> AppResult<Json<Value>> {
    let row = sqlx::query(
        "SELECT owner_id, status, pet_id, proxy_name FROM pickup_authorizations WHERE id = ?",
    )
    .bind(&auth_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or_else(|| AppError::not_found("代接授权不存在"))?;

    let auth_owner: String = row.get("owner_id");
    if auth_owner != owner.0.id {
        // 不能撤销他人的授权
        return Err(AppError::not_found("代接授权不存在"));
    }
    let status: String = row.get("status");
    if status != "active" {
        return Err(AppError::conflict(
            "not_active",
            &format!("授权当前状态为 {status}，无需撤销"),
        ));
    }

    sqlx::query(
        "UPDATE pickup_authorizations SET status = 'revoked', revoke_reason = ? WHERE id = ?",
    )
    .bind(req.reason.unwrap_or_else(|| "主人主动撤销".into()))
    .bind(&auth_id)
    .execute(state.pool())
    .await?;

    // 撤销立即生效：旧码哈希仍在库中，但核销时状态校验会拦截。
    // 为体现“立即失效”，额外记录一条本地通知/日志。
    let pet_id: String = row.get("pet_id");
    let proxy: String = row.get("proxy_name");
    tracing::warn!(
        auth = %auth_id, pet = %pet_id, proxy = %proxy,
        "代接授权已撤销，原取宠码立即失效"
    );
    crate::routes::common::notify(
        &state,
        &owner.0.id,
        "pickup_revoked",
        "代接授权已撤销",
        &format!("代接人「{proxy}」的取宠码已立即失效，旧码无法再领取宠物。"),
        Some(&pet_id),
        None,
    )
    .await?;

    Ok(Json(json!({ "ok": true })))
}

// ---------- 员工核销取宠码 ----------

#[derive(Deserialize)]
pub struct RedeemReq {
    code: String,
}

pub async fn redeem_code(
    State(state): State<AppState>,
    staff: Staff,
    Json(req): Json<RedeemReq>,
) -> AppResult<Json<Value>> {
    let code = req.code.trim();
    if code.is_empty() {
        return Err(AppError::bad_request("请输入取宠码"));
    }

    let row = sqlx::query(
        "SELECT a.*, p.name AS pet_name, p.status AS pet_status
         FROM pickup_authorizations a JOIN pets p ON p.id = a.pet_id
         WHERE a.code_hash = ?",
    )
    .bind(hash_code(code))
    .fetch_optional(state.pool())
    .await?;

    let row = row.ok_or_else(|| {
        AppError::new(
            axum::http::StatusCode::NOT_FOUND,
            "pickup_code_invalid",
            "取宠码无效，查不到对应授权",
        )
    })?;

    let auth_id: String = row.get("id");
    let status: String = row.get("status");
    let pet_id: String = row.get("pet_id");
    let pet_name: String = row.get("pet_name");
    let pet_status: String = row.get("pet_status");
    let proxy_name: String = row.get("proxy_name");
    let proxy_contact: String = row.get("proxy_contact");
    let expires_at: Option<String> = row.get("expires_at");

    // 关键规则：授权撤销后，旧取宠码立即失效——即使码本身正确也必须拦截
    match status.as_str() {
        "revoked" => {
            return Err(AppError::conflict(
                "pickup_code_revoked",
                "该取宠码对应的代接授权已被主人撤销，旧码已失效，不能领取宠物",
            ));
        }
        "used" => {
            return Err(AppError::conflict(
                "pickup_code_used",
                "该取宠码已使用过，不能重复领取",
            ));
        }
        _ => {}
    }

    if let Some(exp) = &expires_at {
        if exp < &now_iso() {
            return Err(AppError::new(
                axum::http::StatusCode::GONE,
                "pickup_code_expired",
                "取宠码已过期",
            ));
        }
    }
    if pet_status == "picked_up" {
        return Err(AppError::conflict("pet_picked_up", "宠物已被接走"));
    }

    // 核销成功：码置为 used，同时办理接走并取消后续任务
    let mut tx = state.pool().begin().await?;
    sqlx::query(
        "UPDATE pickup_authorizations
         SET status = 'used', used_at = ?, used_by_staff = ?
         WHERE id = ? AND status = 'active'",
    )
    .bind(now_iso())
    .bind(staff.id())
    .bind(&auth_id)
    .execute(&mut *tx)
    .await?;

    sqlx::query("UPDATE pets SET status = 'picked_up', picked_up_at = ? WHERE id = ?")
        .bind(now_iso())
        .bind(&pet_id)
        .execute(&mut *tx)
        .await?;

    let cancelled = sqlx::query(
        "UPDATE care_tasks SET status = 'cancelled'
         WHERE pet_id = ? AND status = 'pending'",
    )
    .bind(&pet_id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    // 同一宠物的其他有效授权一并失效，防止多码重复领取
    sqlx::query(
        "UPDATE pickup_authorizations SET status = 'revoked', revoke_reason = '宠物已被接走'
         WHERE pet_id = ? AND status = 'active' AND id != ?",
    )
    .bind(&pet_id)
    .bind(&auth_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    let owner_id: String = sqlx::query_scalar("SELECT owner_id FROM pets WHERE id = ?")
        .bind(&pet_id)
        .fetch_one(state.pool())
        .await?;
    crate::routes::common::notify(
        &state,
        &owner_id,
        "pickup_redeemed",
        "代接人已凭码接走宠物",
        &format!(
            "{pet_name} 已由代接人「{proxy_name}」({}) 凭取宠码接走，{cancelled} 项后续任务已取消。",
            if proxy_contact.is_empty() { "联系方式未留".to_string() } else { proxy_contact }
        ),
        Some(&pet_id),
        None,
    )
    .await?;

    tracing::info!(staff = staff.id(), pet = %pet_id, proxy = %proxy_name, "取宠码核销成功");
    Ok(Json(json!({
        "ok": true,
        "pet_id": pet_id,
        "pet_name": pet_name,
        "proxy_name": proxy_name,
        "cancelled_tasks": cancelled,
    })))
}
