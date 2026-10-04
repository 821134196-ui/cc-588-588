use crate::app::{AppError, AppResult, AppState};
use crate::auth::{new_id, now_iso, require_pet_access, Staff, User};
use axum::extract::{Path, Query, State};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use sqlx::sqlite::SqliteRow;
use sqlx::Row;

#[derive(Deserialize)]
pub struct TaskQuery {
    pet_id: Option<String>,
    status: Option<String>,
    kind: Option<String>,
    date: Option<String>, // YYYY-MM-DD，按计划时间过滤
    #[allow(dead_code)]
    mine: Option<String>,
}

/// 任务看板：员工可看全部，主人只能看到自己宠物的任务
pub async fn list_tasks(
    State(state): State<AppState>,
    user: User,
    q: Query<TaskQuery>,
) -> AppResult<Json<Value>> {
    let q = q.0;
    if let Some(status) = &q.status {
        if !["pending", "done", "anomaly", "cancelled"].contains(&status.as_str()) {
            return Err(AppError::bad_request("status 过滤值无效"));
        }
    }
    if let Some(kind) = &q.kind {
        if !["feeding", "medication"].contains(&kind.as_str()) {
            return Err(AppError::bad_request("kind 过滤值无效"));
        }
    }
    if let Some(date) = &q.date {
        chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
            .map_err(|_| AppError::bad_request("date 应为 YYYY-MM-DD"))?;
    }

    let mut qb = sqlx::QueryBuilder::<sqlx::Sqlite>::new(
        "SELECT t.*, m.drug_name, m.dosage, p.name AS pet_name, p.owner_id,
                e.id AS exec_id, e.performer_id, e.verifier_id, e.executed_at,
                e.notes AS exec_notes, e.photo_path,
                up.display_name AS performer_name,
                uv.display_name AS verifier_name
         FROM care_tasks t
         JOIN pets p ON p.id = t.pet_id
         LEFT JOIN medications m ON m.id = t.medication_id
         LEFT JOIN executions e ON e.task_id = t.id
         LEFT JOIN users up ON up.id = e.performer_id
         LEFT JOIN users uv ON uv.id = e.verifier_id
         WHERE 1=1",
    );
    if user.role == "owner" {
        qb.push(" AND p.owner_id = ").push_bind(user.id.clone());
    }
    if let Some(pet_id) = q.pet_id {
        qb.push(" AND t.pet_id = ").push_bind(pet_id);
    }
    if let Some(status) = q.status {
        qb.push(" AND t.status = ").push_bind(status);
    }
    if let Some(kind) = q.kind {
        qb.push(" AND t.kind = ").push_bind(kind);
    }
    if let Some(date) = q.date {
        qb.push(" AND substr(t.scheduled_at, 1, 10) = ")
            .push_bind(date);
    }
    qb.push(" ORDER BY t.scheduled_at ASC LIMIT 500");

    let rows = qb.build().fetch_all(state.pool()).await?;
    let tasks: Vec<Value> = rows.iter().map(task_json).collect();
    Ok(Json(json!({ "tasks": tasks })))
}

fn task_json(r: &SqliteRow) -> Value {
    json!({
        "id": r.get::<String,_>("id"),
        "pet_id": r.get::<String,_>("pet_id"),
        "pet_name": r.get::<String,_>("pet_name"),
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
}

async fn load_task(state: &AppState, task_id: &str) -> AppResult<SqliteRow> {
    sqlx::query(
        "SELECT t.*, m.drug_name, m.dosage FROM care_tasks t
         LEFT JOIN medications m ON m.id = t.medication_id
         WHERE t.id = ?",
    )
    .bind(task_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or_else(|| AppError::not_found("任务不存在"))
}

pub async fn task_detail(
    State(state): State<AppState>,
    user: User,
    Path(id): Path<String>,
) -> AppResult<Json<Value>> {
    let row = load_task(&state, &id).await?;
    let pet_id: String = row.get("pet_id");
    require_pet_access(&state, &user, &pet_id).await?;

    let exec = sqlx::query(
        "SELECT e.*, up.display_name AS performer_name, uv.display_name AS verifier_name
         FROM executions e
         JOIN users up ON up.id = e.performer_id
         LEFT JOIN users uv ON uv.id = e.verifier_id
         WHERE e.task_id = ?",
    )
    .bind(&id)
    .fetch_optional(state.pool())
    .await?
    .map(|r| {
        json!({
            "performer_id": r.get::<String,_>("performer_id"),
            "performer_name": r.get::<String,_>("performer_name"),
            "verifier_id": r.get::<Option<String>,_>("verifier_id"),
            "verifier_name": r.get::<Option<String>,_>("verifier_name"),
            "executed_at": r.get::<String,_>("executed_at"),
            "notes": r.get::<String,_>("notes"),
            "photo_path": r.get::<Option<String>,_>("photo_path"),
        })
    });

    Ok(Json(json!({
        "task": {
            "id": row.get::<String,_>("id"),
            "pet_id": pet_id,
            "kind": row.get::<String,_>("kind"),
            "medication_id": row.get::<Option<String>,_>("medication_id"),
            "drug_name": row.get::<Option<String>,_>("drug_name"),
            "dosage": row.get::<Option<String>,_>("dosage"),
            "scheduled_at": row.get::<String,_>("scheduled_at"),
            "status": row.get::<String,_>("status"),
        },
        "execution": exec,
    })))
}

// ---------- 执行任务 ----------

#[derive(Deserialize)]
pub struct ExecuteReq {
    notes: Option<String>,
    photo_path: Option<String>,
    /// 给药时必填：另一名核对员工
    verifier_id: Option<String>,
    /// 核对员工确认看到的药名 / 剂量，必须与处方一致
    verify_drug_name: Option<String>,
    verify_dosage: Option<String>,
}

pub async fn execute_task(
    State(state): State<AppState>,
    staff: Staff,
    Path(id): Path<String>,
    Json(req): Json<ExecuteReq>,
) -> AppResult<(axum::http::StatusCode, Json<Value>)> {
    let task = load_task(&state, &id).await?;
    let task_id: String = task.get("id");
    let _pet_id: String = task.get("pet_id");
    let kind: String = task.get("kind");
    let status: String = task.get("status");

    if status == "cancelled" {
        return Err(AppError::conflict(
            "task_cancelled",
            "该任务已取消（宠物已接走或用药已停用），不能执行",
        ));
    }

    // 关键规则 1：同一任务重复提交不得算作执行两次。
    // executions.task_id 有 UNIQUE 约束，这里先给出明确的业务拒绝。
    let existing: Option<String> =
        sqlx::query_scalar("SELECT id FROM executions WHERE task_id = ?")
            .bind(&task_id)
            .fetch_optional(state.pool())
            .await?
            .flatten();
    if existing.is_some() {
        return Err(AppError::conflict(
            "already_executed",
            "该任务已执行过，重复提交不会重复记录",
        ));
    }

    // 异常未闭环（anomaly）的任务不能再直接勾完成，须先按异常流程处理
    if status == "anomaly" {
        return Err(AppError::conflict(
            "anomaly_open",
            "该任务存在未处理异常，请通过异常流程记录处理经过",
        ));
    }

    // 关键规则 2：给药必须由另一名员工核对药名及剂量
    let mut verifier_id: Option<String> = None;
    if kind == "medication" {
        let vid = req.verifier_id.as_deref().filter(|s| !s.is_empty())
            .ok_or_else(|| {
                AppError::unprocessable(
                    "verifier_required",
                    "给药任务必须由另一名员工核对后才能完成",
                )
            })?;

        if vid == staff.id() {
            return Err(AppError::unprocessable(
                "verifier_must_differ",
                "核对人不能是执行本人，给药必须由另一名员工核对",
            ));
        }

        let verifier_role: Option<String> =
            sqlx::query_scalar("SELECT role FROM users WHERE id = ?")
                .bind(vid)
                .fetch_optional(state.pool())
                .await?
                .flatten();
        if verifier_role.as_deref() != Some("staff") {
            return Err(AppError::unprocessable(
                "verifier_must_be_staff",
                "核对人必须是另一名门店员工",
            ));
        }

        // 核对药名与剂量：员工确认值必须与卡片处方一致
        let expect_name: String = task
            .get::<Option<String>, _>("drug_name")
            .unwrap_or_default();
        let expect_dose: String = task
            .get::<Option<String>, _>("dosage")
            .unwrap_or_default();
        let got_name = req.verify_drug_name.as_deref().unwrap_or("").trim();
        let got_dose = req.verify_dosage.as_deref().unwrap_or("").trim();
        if got_name.is_empty() || got_dose.is_empty() {
            return Err(AppError::unprocessable(
                "verify_fields_required",
                "核对员工必须确认药名与剂量",
            ));
        }
        if got_name != expect_name || got_dose != expect_dose {
            return Err(AppError::unprocessable(
                "verify_mismatch",
                &format!(
                    "核对不通过：处方为「{expect_name} {expect_dose}」，提交为「{got_name} {got_dose}」，请重新核对"
                ),
            ));
        }
        verifier_id = Some(vid.to_string());
    }

    // 写入执行记录并把任务置为 done，一个事务保证一致性
    let exec_id = new_id();
    let mut tx = state.pool().begin().await?;
    sqlx::query(
        "INSERT INTO executions
            (id, task_id, performer_id, verifier_id, executed_at, notes, photo_path, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&exec_id)
    .bind(&task_id)
    .bind(staff.id())
    .bind(&verifier_id)
    .bind(now_iso())
    .bind(req.notes.unwrap_or_default())
    .bind(&req.photo_path)
    .bind(now_iso())
    .execute(&mut *tx)
    .await
    .map_err(|e| {
        // 并发情况下 UNIQUE(task_id) 兜底：仍然只执行一次
        if let sqlx::Error::Database(db) = &e {
            if db.message().contains("UNIQUE") {
                return AppError::conflict(
                    "already_executed",
                    "该任务已执行过，重复提交不会重复记录",
                );
            }
        }
        AppError::from(e)
    })?;

    sqlx::query("UPDATE care_tasks SET status = 'done' WHERE id = ?")
        .bind(&task_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    tracing::info!(
        performer = staff.id(),
        verifier = ?verifier_id,
        task = %task_id,
        kind = %kind,
        "任务已执行"
    );
    Ok((axum::http::StatusCode::CREATED, Json(json!({ "id": exec_id }))))
}

// ---------- 异常上报（任务维度快捷入口） ----------

#[derive(Deserialize)]
pub struct AnomalyReq {
    incident_type: String,
    description: String,
    handling: String,
    photo_path: Option<String>,
}

pub async fn report_anomaly(
    State(state): State<AppState>,
    staff: Staff,
    Path(id): Path<String>,
    Json(req): Json<AnomalyReq>,
) -> AppResult<(axum::http::StatusCode, Json<Value>)> {
    const TYPES: [&str; 4] = ["refused_food", "missed_medication", "food_swap", "other"];
    if !TYPES.contains(&req.incident_type.as_str()) {
        return Err(AppError::bad_request("异常类型无效"));
    }
    if req.description.trim().is_empty() || req.handling.trim().is_empty() {
        return Err(AppError::unprocessable(
            "handling_required",
            "必须填写异常情况与处理经过，不能直接勾选完成",
        ));
    }

    let task = load_task(&state, &id).await?;
    let task_id: String = task.get("id");
    let pet_id: String = task.get("pet_id");
    let task_status: String = task.get("status");
    if task_status == "cancelled" {
        return Err(AppError::conflict("task_cancelled", "任务已取消，不能上报异常"));
    }

    let incident_id = new_id();
    let mut tx = state.pool().begin().await?;
    sqlx::query(
        "INSERT INTO incidents (id, pet_id, task_id, incident_type, description, handling, staff_id, photo_path, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(&incident_id)
    .bind(&pet_id)
    .bind(&task_id)
    .bind(&req.incident_type)
    .bind(req.description.trim())
    .bind(req.handling.trim())
    .bind(staff.id())
    .bind(&req.photo_path)
    .bind(now_iso())
    .execute(&mut *tx)
    .await?;

    // 待执行任务转为 anomaly；它不会被勾成 done，主人端会看到醒目标记
    sqlx::query("UPDATE care_tasks SET status = 'anomaly' WHERE id = ? AND status = 'pending'")
        .bind(&task_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    // 通知主人
    let owner_id = crate::routes::common::pet_owner_id(&state, &pet_id).await?;
    let (kind_code, title) = match req.incident_type.as_str() {
        "refused_food" => ("refused_food", "宠物拒食提醒"),
        "missed_medication" => ("missed_medication", "药物漏服提醒"),
        "food_swap" => ("food_swap", "临时换粮告知"),
        _ => ("other", "照护异常提醒"),
    };
    let body = format!(
        "情况：{}｜处理：{}",
        req.description.trim(),
        req.handling.trim()
    );
    crate::routes::common::notify(
        &state,
        &owner_id,
        kind_code,
        title,
        &body,
        Some(&pet_id),
        Some(&incident_id),
    )
    .await?;

    Ok((
        axum::http::StatusCode::CREATED,
        Json(json!({ "id": incident_id })),
    ))
}
