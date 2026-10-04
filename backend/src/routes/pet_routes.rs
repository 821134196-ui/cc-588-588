use crate::app::{AppError, AppResult, AppState};
use crate::auth::{new_id, now_iso, require_pet_access, Owner, Staff, User};
use crate::schedule::ensure_care_calendar;
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

// ---------- 列表 / 详情 ----------

#[derive(Deserialize)]
pub struct ListQuery {
    #[allow(dead_code)]
    mine: Option<String>, // 兼容参数；主人始终只返回自己的
}

pub async fn list_pets(
    State(state): State<AppState>,
    user: User,
    Query(_q): Query<ListQuery>,
) -> AppResult<Json<Value>> {
    let rows = if user.role == "owner" {
        sqlx::query(
            "SELECT p.*, u.display_name AS owner_name
             FROM pets p JOIN users u ON u.id = p.owner_id
             WHERE p.owner_id = ? ORDER BY p.created_at DESC",
        )
        .bind(&user.id)
        .fetch_all(state.pool())
        .await?
    } else {
        sqlx::query(
            "SELECT p.*, u.display_name AS owner_name
             FROM pets p JOIN users u ON u.id = p.owner_id
             ORDER BY p.status ASC, p.created_at DESC",
        )
        .fetch_all(state.pool())
        .await?
    };
    let pets: Vec<Value> = rows.iter().map(pet_json).collect();
    Ok(Json(json!({ "pets": pets })))
}

/// 员工创建宠物卡片时可选择的主人名录
pub async fn list_owners(_staff: Staff, State(state): State<AppState>) -> AppResult<Json<Value>> {
    let rows = sqlx::query("SELECT id, username, display_name FROM users WHERE role = 'owner' ORDER BY display_name")
        .fetch_all(state.pool())
        .await?;
    let owners: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String,_>("id"),
                "username": r.get::<String,_>("username"),
                "display_name": r.get::<String,_>("display_name"),
            })
        })
        .collect();
    Ok(Json(json!({ "owners": owners })))
}

/// 员工名录（给药任务选择“另一名核对员工”用）
pub async fn list_staff(
    State(state): State<AppState>,
    user: User,
) -> AppResult<Json<Value>> {
    let rows = sqlx::query(
        "SELECT id, username, display_name FROM users WHERE role = 'staff' ORDER BY display_name",
    )
    .fetch_all(state.pool())
    .await?;
    let staff: Vec<Value> = rows
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String,_>("id"),
                "username": r.get::<String,_>("username"),
                "display_name": r.get::<String,_>("display_name"),
            })
        })
        .collect();
    Ok(Json(json!({ "staff": staff, "me": user.id })))
}

fn pet_json(r: &SqliteRow) -> Value {
    json!({
        "id": r.get::<String,_>("id"),
        "owner_id": r.get::<String,_>("owner_id"),
        "owner_name": r.get::<String,_>("owner_name"),
        "name": r.get::<String,_>("name"),
        "species": r.get::<String,_>("species"),
        "food_portion": r.get::<String,_>("food_portion"),
        "food_brand": r.get::<String,_>("food_brand"),
        "contraindications": r.get::<String,_>("contraindications"),
        "feeding_times": r.get::<String,_>("feeding_times"),
        "notes": r.get::<String,_>("notes"),
        "boarding_start": r.get::<String,_>("boarding_start"),
        "boarding_end": r.get::<String,_>("boarding_end"),
        "status": r.get::<String,_>("status"),
        "picked_up_at": r.get::<Option<String>,_>("picked_up_at"),
        "created_at": r.get::<String,_>("created_at"),
    })
}

pub async fn pet_detail(
    State(state): State<AppState>,
    user: User,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    require_pet_access(&state, &user, &id).await?;
    let row = sqlx::query(
        "SELECT p.*, u.display_name AS owner_name
         FROM pets p JOIN users u ON u.id = p.owner_id WHERE p.id = ?",
    )
    .bind(&id)
    .fetch_optional(state.pool())
    .await?
    .ok_or_else(|| AppError::not_found("宠物不存在"))?;

    let meds = sqlx::query("SELECT * FROM medications WHERE pet_id = ? ORDER BY created_at")
        .bind(&id)
        .fetch_all(state.pool())
        .await?
        .iter()
        .map(med_json)
        .collect::<Vec<_>>();

    Ok(Json(json!({
        "pet": pet_json(&row),
        "medications": meds,
    })))
}

fn med_json(r: &SqliteRow) -> Value {
    json!({
        "id": r.get::<String,_>("id"),
        "pet_id": r.get::<String,_>("pet_id"),
        "drug_name": r.get::<String,_>("drug_name"),
        "dosage": r.get::<String,_>("dosage"),
        "instructions": r.get::<String,_>("instructions"),
        "times": r.get::<String,_>("times"),
        "active": r.get::<bool,_>("active"),
    })
}

// ---------- 员工：创建 / 编辑卡片 ----------

#[derive(Deserialize)]
pub struct CreatePetReq {
    owner_id: String,
    name: String,
    species: Option<String>,
    food_portion: Option<String>,
    food_brand: Option<String>,
    contraindications: Option<String>,
    feeding_times: Option<String>,
    notes: Option<String>,
    boarding_start: String, // YYYY-MM-DD
    boarding_end: String,
}

pub async fn create_pet(
    State(state): State<AppState>,
    staff: Staff,
    Json(req): Json<CreatePetReq>,
) -> AppResult<(axum::http::StatusCode, Json<Value>)> {
    if req.name.trim().is_empty() {
        return Err(AppError::bad_request("宠物名称不能为空"));
    }
    validate_date(&req.boarding_start)?;
    validate_date(&req.boarding_end)?;
    if req.boarding_end < req.boarding_start {
        return Err(AppError::bad_request("寄养结束日期不能早于开始日期"));
    }
    // owner 必须真实存在且为主人角色
    let owner_role: Option<String> =
        sqlx::query_scalar("SELECT role FROM users WHERE id = ?")
            .bind(&req.owner_id)
            .fetch_optional(state.pool())
            .await?
            .flatten();
    if owner_role.as_deref() != Some("owner") {
        return Err(AppError::bad_request("请选择有效的宠物主人"));
    }

    let id = new_id();
    sqlx::query(
        "INSERT INTO pets (id, owner_id, name, species, food_portion, food_brand,
             contraindications, feeding_times, notes, boarding_start, boarding_end, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&id)
    .bind(&req.owner_id)
    .bind(req.name.trim())
    .bind(req.species.unwrap_or_default())
    .bind(req.food_portion.unwrap_or_default())
    .bind(req.food_brand.unwrap_or_default())
    .bind(req.contraindications.unwrap_or_default())
    .bind(req.feeding_times.unwrap_or_else(|| "08:00,18:00".into()))
    .bind(req.notes.unwrap_or_default())
    .bind(&req.boarding_start)
    .bind(&req.boarding_end)
    .bind(now_iso())
    .execute(state.pool())
    .await?;

    ensure_care_calendar(&state, &id).await?;
    tracing::info!(staff = staff.id(), pet = %id, "已创建宠物卡片并生成照护日历");

    Ok((
        axum::http::StatusCode::CREATED,
        Json(json!({ "id": id })),
    ))
}

#[derive(Deserialize)]
pub struct UpdatePetReq {
    name: Option<String>,
    species: Option<String>,
    food_portion: Option<String>,
    food_brand: Option<String>,
    contraindications: Option<String>,
    feeding_times: Option<String>,
    notes: Option<String>,
    boarding_start: Option<String>,
    boarding_end: Option<String>,
}

pub async fn update_pet(
    State(state): State<AppState>,
    _staff: Staff,
    Path(id): Path<String>,
    Json(req): Json<UpdatePetReq>,
) -> AppResult<Json<Value>> {
    let status: String = sqlx::query_scalar("SELECT status FROM pets WHERE id = ?")
        .bind(&id)
        .fetch_optional(state.pool())
        .await?
        .flatten()
        .ok_or_else(|| AppError::not_found("宠物不存在"))?;
    if status == "picked_up" {
        return Err(AppError::conflict("pet_picked_up", "宠物已被接走，卡片不可再修改"));
    }
    if let Some(d) = &req.boarding_start {
        validate_date(d)?;
    }
    if let Some(d) = &req.boarding_end {
        validate_date(d)?;
    }

    // 逐字段更新（仅更新出现的字段）
    macro_rules! upd {
        ($col:literal, $val:expr) => {
            if let Some(v) = $val {
                sqlx::query(&format!("UPDATE pets SET {} = ? WHERE id = ?", $col))
                    .bind(v)
                    .bind(&id)
                    .execute(state.pool())
                    .await?;
            }
        };
    }
    upd!("name", req.name);
    upd!("species", req.species);
    upd!("food_portion", req.food_portion);
    upd!("food_brand", req.food_brand);
    upd!("contraindications", req.contraindications);
    upd!("feeding_times", req.feeding_times);
    upd!("notes", req.notes);
    upd!("boarding_start", req.boarding_start);
    upd!("boarding_end", req.boarding_end);

    // 时刻/日期变化后补齐新增任务，已有任务与执行记录不受影响
    ensure_care_calendar(&state, &id).await?;
    Ok(Json(json!({ "ok": true })))
}

fn validate_date(s: &str) -> AppResult<()> {
    chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d")
        .map_err(|_| AppError::bad_request("日期格式应为 YYYY-MM-DD"))?;
    Ok(())
}

// ---------- 员工：用药管理 ----------

#[derive(Deserialize)]
pub struct AddMedReq {
    drug_name: String,
    dosage: String,
    instructions: Option<String>,
    times: String,
}

pub async fn add_medication(
    State(state): State<AppState>,
    _staff: Staff,
    Path(id): Path<String>,
    Json(req): Json<AddMedReq>,
) -> AppResult<(axum::http::StatusCode, Json<Value>)> {
    if req.drug_name.trim().is_empty() || req.dosage.trim().is_empty() {
        return Err(AppError::bad_request("药名与剂量不能为空"));
    }
    let exists: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM pets WHERE id = ?")
        .bind(&id)
        .fetch_one(state.pool())
        .await?;
    if exists == 0 {
        return Err(AppError::not_found("宠物不存在"));
    }
    let med_id = new_id();
    sqlx::query(
        "INSERT INTO medications (id, pet_id, drug_name, dosage, instructions, times, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&med_id)
    .bind(&id)
    .bind(req.drug_name.trim())
    .bind(req.dosage.trim())
    .bind(req.instructions.unwrap_or_default())
    .bind(&req.times)
    .bind(now_iso())
    .execute(state.pool())
    .await?;

    ensure_care_calendar(&state, &id).await?;
    Ok((axum::http::StatusCode::CREATED, Json(json!({ "id": med_id }))))
}

/// 停用药物：同时取消该药尚未执行的给药任务（已执行记录保留）
pub async fn remove_medication(
    State(state): State<AppState>,
    _staff: Staff,
    Path((id, med_id)): Path<(String, String)>,
) -> AppResult<Json<Value>> {
    let res = sqlx::query("UPDATE medications SET active = 0 WHERE id = ? AND pet_id = ?")
        .bind(&med_id)
        .bind(&id)
        .execute(state.pool())
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::not_found("用药记录不存在"));
    }
    sqlx::query(
        "UPDATE care_tasks SET status = 'cancelled'
         WHERE medication_id = ? AND status = 'pending'",
    )
    .bind(&med_id)
    .execute(state.pool())
    .await?;
    Ok(Json(json!({ "ok": true })))
}

// ---------- 时间线（员工与主人均可，主人限本宠） ----------

pub async fn pet_timeline(
    State(state): State<AppState>,
    user: User,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    require_pet_access(&state, &user, &id).await?;

    let tasks = sqlx::query(
        "SELECT t.*, m.drug_name, m.dosage,
                e.id AS exec_id, e.performer_id, e.verifier_id, e.executed_at,
                e.notes AS exec_notes, e.photo_path, e.created_at AS exec_created,
                up.display_name AS performer_name,
                uv.display_name AS verifier_name
         FROM care_tasks t
         LEFT JOIN medications m ON m.id = t.medication_id
         LEFT JOIN executions e ON e.task_id = t.id
         LEFT JOIN users up ON up.id = e.performer_id
         LEFT JOIN users uv ON uv.id = e.verifier_id
         WHERE t.pet_id = ?
         ORDER BY t.scheduled_at ASC",
    )
    .bind(&id)
    .fetch_all(state.pool())
    .await?;

    let task_items: Vec<Value> = tasks
        .iter()
        .map(|r| {
            json!({
                "id": r.get::<String,_>("id"),
                "kind": r.get::<String,_>("kind"),
                "medication_id": r.get::<Option<String>,_>("medication_id"),
                "drug_name": r.get::<Option<String>,_>("drug_name"),
                "dosage": r.get::<Option<String>,_>("dosage"),
                "scheduled_at": r.get::<String,_>("scheduled_at"),
                "status": r.get::<String,_>("status"),
                "execution": r.get::<Option<String>,_>("exec_id").map(|_| json!({
                    "performer_id": r.get::<String,_>("performer_id"),
                    "performer_name": r.get::<String,_>("performer_name"),
                    "verifier_id": r.get::<Option<String>,_>("verifier_id"),
                    "verifier_name": r.get::<Option<String>,_>("verifier_name"),
                    "executed_at": r.get::<String,_>("executed_at"),
                    "notes": r.get::<String,_>("exec_notes"),
                    "photo_path": r.get::<Option<String>,_>("photo_path"),
                })),
            })
        })
        .collect();

    let incidents = sqlx::query(
        "SELECT i.*, u.display_name AS staff_name
         FROM incidents i JOIN users u ON u.id = i.staff_id
         WHERE i.pet_id = ? ORDER BY i.created_at DESC",
    )
    .bind(&id)
    .fetch_all(state.pool())
    .await?;
    let incident_items: Vec<Value> = incidents.iter().map(incident_json).collect();

    // 汇总，方便主人一眼看到是否按时
    let summary = sqlx::query_as::<_, (String, i64)>(
        "SELECT status, COUNT(*) FROM care_tasks WHERE pet_id = ? GROUP BY status",
    )
    .bind(&id)
    .fetch_all(state.pool())
    .await?
    .into_iter()
    .map(|(k, v)| (k, v))
    .collect::<std::collections::HashMap<_, _>>();

    Ok(Json(json!({
        "tasks": task_items,
        "incidents": incident_items,
        "summary": {
            "pending": summary.get("pending").copied().unwrap_or(0),
            "done": summary.get("done").copied().unwrap_or(0),
            "anomaly": summary.get("anomaly").copied().unwrap_or(0),
            "cancelled": summary.get("cancelled").copied().unwrap_or(0),
        }
    })))
}

fn incident_json(r: &SqliteRow) -> Value {
    json!({
        "id": r.get::<String,_>("id"),
        "task_id": r.get::<Option<String>,_>("task_id"),
        "incident_type": r.get::<String,_>("incident_type"),
        "description": r.get::<String,_>("description"),
        "handling": r.get::<String,_>("handling"),
        "staff_id": r.get::<String,_>("staff_id"),
        "staff_name": r.get::<String,_>("staff_name"),
        "photo_path": r.get::<Option<String>,_>("photo_path"),
        "created_at": r.get::<String,_>("created_at"),
    })
}

// ---------- 异常事件 ----------

#[derive(Deserialize)]
pub struct ListIncidentsQuery {
    #[allow(dead_code)]
    r#type: Option<String>,
}

pub async fn list_incidents(
    State(state): State<AppState>,
    user: User,
    Path(id): Path<String>,
    Query(_q): Query<ListIncidentsQuery>,
) -> AppResult<Json<Value>> {
    require_pet_access(&state, &user, &id).await?;
    let rows = sqlx::query(
        "SELECT i.*, u.display_name AS staff_name
         FROM incidents i JOIN users u ON u.id = i.staff_id
         WHERE i.pet_id = ? ORDER BY i.created_at DESC",
    )
    .bind(&id)
    .fetch_all(state.pool())
    .await?;
    Ok(Json(json!({ "incidents": rows.iter().map(incident_json).collect::<Vec<_>>() })))
}

#[derive(Deserialize)]
pub struct CreateIncidentReq {
    incident_type: String, // refused_food | missed_medication | food_swap | other
    description: String,   // 异常情况
    handling: String,      // 处理经过
    task_id: Option<String>,
    photo_path: Option<String>,
}

pub async fn create_incident(
    State(state): State<AppState>,
    staff: Staff,
    Path(id): Path<String>,
    Json(req): Json<CreateIncidentReq>,
) -> AppResult<(axum::http::StatusCode, Json<Value>)> {
    const TYPES: [&str; 4] = ["refused_food", "missed_medication", "food_swap", "other"];
    if !TYPES.contains(&req.incident_type.as_str()) {
        return Err(AppError::bad_request("异常类型无效"));
    }
    if req.description.trim().is_empty() || req.handling.trim().is_empty() {
        // 关键规则：异常不能像普通任务那样直接勾完成，必须写清情况与处理经过
        return Err(AppError::unprocessable(
            "handling_required",
            "异常事件必须填写异常情况与处理经过，不能直接勾选完成",
        ));
    }

    // 若关联任务，校验任务属于该宠物
    if let Some(task_id) = &req.task_id {
        let task_pet: Option<String> =
            sqlx::query_scalar("SELECT pet_id FROM care_tasks WHERE id = ?")
                .bind(task_id)
                .fetch_optional(state.pool())
                .await?
                .flatten();
        match task_pet {
            Some(p) if p == id => {}
            Some(_) => return Err(AppError::bad_request("任务与宠物不匹配")),
            None => return Err(AppError::not_found("任务不存在")),
        }
    }

    let incident_id = new_id();
    sqlx::query(
        "INSERT INTO incidents (id, pet_id, task_id, incident_type, description, handling, staff_id, photo_path, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&incident_id)
    .bind(&id)
    .bind(&req.task_id)
    .bind(&req.incident_type)
    .bind(req.description.trim())
    .bind(req.handling.trim())
    .bind(staff.id())
    .bind(&req.photo_path)
    .bind(now_iso())
    .execute(state.pool())
    .await?;

    // 关联任务若还没有执行记录，标记为 anomaly（独立于“完成”）；
    // 已完成的任务保留 done 状态，异常作为独立记录存在。
    if let Some(task_id) = &req.task_id {
        sqlx::query(
            "UPDATE care_tasks
             SET status = 'anomaly'
             WHERE id = ? AND status = 'pending'",
        )
        .bind(task_id)
        .execute(state.pool())
        .await?;
    }

    // 通知主人
    let owner_id = crate::routes::common::pet_owner_id(&state, &id).await?;
    let (type_label, title) = match req.incident_type.as_str() {
        "refused_food" => ("refused_food", "宠物拒食提醒"),
        "missed_medication" => ("missed_medication", "药物漏服提醒"),
        "food_swap" => ("food_swap", "临时换粮告知"),
        _ => ("other", "照护异常提醒"),
    };
    let body = format!("{}｜情况：{}｜处理：{}", pet_name(&state, &id).await?, req.description.trim(), req.handling.trim());
    crate::routes::common::notify(
        &state,
        &owner_id,
        type_label,
        title,
        &body,
        Some(&id),
        Some(&incident_id),
    )
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(json!({ "id": incident_id })),
    ))
}

async fn pet_name(state: &AppState, pet_id: &str) -> AppResult<String> {
    sqlx::query_scalar("SELECT name FROM pets WHERE id = ?")
        .bind(pet_id)
        .fetch_optional(state.pool())
        .await?
        .flatten()
        .ok_or_else(|| AppError::not_found("宠物不存在"))
}

// ---------- 提前接走：取消后续任务 ----------

#[derive(Deserialize)]
pub struct CheckoutReq {
    note: Option<String>,
}

pub async fn checkout(
    State(state): State<AppState>,
    staff: Staff,
    Path(id): Path<String>,
    Json(req): Json<CheckoutReq>,
) -> AppResult<Json<Value>> {
    let mut tx = state.pool().begin().await?;

    let status: Option<String> = sqlx::query_scalar("SELECT status FROM pets WHERE id = ?")
        .bind(&id)
        .fetch_optional(&mut *tx)
        .await?
        .flatten();
    let status = status.ok_or_else(|| AppError::not_found("宠物不存在"))?;
    if status == "picked_up" {
        return Err(AppError::conflict("already_picked_up", "该宠物已办理接走"));
    }

    sqlx::query("UPDATE pets SET status = 'picked_up', picked_up_at = ? WHERE id = ?")
        .bind(now_iso())
        .bind(&id)
        .execute(&mut *tx)
        .await?;

    // 取消所有尚未执行的任务；已完成/异常的记录保留作为交接凭证
    let cancelled = sqlx::query(
        "UPDATE care_tasks SET status = 'cancelled'
         WHERE pet_id = ? AND status = 'pending'",
    )
    .bind(&id)
    .execute(&mut *tx)
    .await?
    .rows_affected();

    // 未使用的代接授权一并失效（主人亲自接走）
    sqlx::query(
        "UPDATE pickup_authorizations SET status = 'revoked', revoke_reason = '宠物已接走'
         WHERE pet_id = ? AND status = 'active'",
    )
    .bind(&id)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;

    let owner_id = crate::routes::common::pet_owner_id(&state, &id).await?;
    let name = pet_name(&state, &id).await?;
    crate::routes::common::notify(
        &state,
        &owner_id,
        "checkout",
        "宠物已接走",
        &format!(
            "{name} 已提前接走，{} 项后续照护任务已取消。{}",
            cancelled,
            req.note.as_deref().unwrap_or("")
        ),
        Some(&id),
        None,
    )
    .await?;

    tracing::info!(staff = staff.id(), pet = %id, cancelled, "宠物提前接走，后续任务已取消");
    Ok(Json(json!({ "ok": true, "cancelled_tasks": cancelled })))
}

// Owner 提取器仅用于在路由文档中表达角色约束；保留引用避免未使用告警
#[allow(dead_code)]
fn _assert_owner(_o: Owner) {}
