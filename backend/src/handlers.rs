use crate::db::{find_user_by_username, hash_password};
use crate::error::ApiError;
use crate::models::*;
use crate::{ApiResult, AppState};
use axum::async_trait;
use axum::extract::{multipart::Field, FromRequestParts, Multipart, Path, Query, State};
use axum::http::header::AUTHORIZATION;
use axum::http::request::Parts;
use axum::routing::{get, post};
use axum::{Json, Router};
use rand::Rng;
use serde_json::{json, Value};
use sqlx::FromRow;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/me", get(me))
        .route("/users", get(list_users))
        // 宠物
        .route("/pets", get(list_pets).post(create_pet))
        .route("/pets/:id", get(get_pet))
        .route("/pets/:id/checkout", post(checkout_pet))
        .route("/pets/:id/medications", get(list_medications).post(add_medication))
        // 照护任务
        .route("/tasks", get(list_tasks).post(create_task))
        .route("/tasks/:id", get(get_task))
        .route("/tasks/:id/execute", post(execute_task))
        .route("/tasks/:id/execute-photo", post(execute_task_photo))
        .route("/tasks/:id/cancel", post(cancel_task))
        // 异常与通知
        .route("/incidents", get(list_incidents).post(create_incident))
        .route("/notifications", get(list_notifications))
        .route("/notifications/:id/read", post(read_notification))
        // 取宠码
        .route("/pickup-codes", get(list_pickup_codes).post(create_pickup_code))
        .route("/pickup-codes/:id/revoke", post(revoke_pickup_code))
        .route("/pickup/redeem", post(redeem_pickup_code))
        .fallback(api_not_found)
}

async fn api_not_found() -> ApiError {
    ApiError::not_found("接口不存在")
}

// ============================ 鉴权提取器 ============================

#[derive(Debug)]
pub struct AuthUser(pub User);

impl AuthUser {
    pub fn id(&self) -> i64 {
        self.0.id
    }
    pub fn is_staff(&self) -> bool {
        self.0.role == "staff"
    }
    pub fn require_staff(&self) -> ApiResult<()> {
        if self.is_staff() {
            Ok(())
        } else {
            Err(ApiError::forbidden("仅门店员工可执行该操作"))
        }
    }
}

#[async_trait]
impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;
    async fn from_request_parts(
        parts: &mut Parts,
        state: &AppState,
    ) -> Result<Self, Self::Rejection> {
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.strip_prefix("Bearer "))
            .ok_or_else(|| ApiError::unauthorized("缺少登录令牌"))?;
        let row: Option<(i64,)> = sqlx::query_as("SELECT user_id FROM sessions WHERE token = ?1")
            .bind(token)
            .fetch_optional(&state.pool)
            .await?;
        let user_id = row
            .ok_or_else(|| ApiError::unauthorized("登录已失效，请重新登录"))?
            .0;
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?1")
            .bind(user_id)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::unauthorized("用户不存在"))?;
        Ok(AuthUser(user))
    }
}

// ============================ 认证 ============================

async fn login(
    State(state): State<AppState>,
    Json(req): Json<LoginReq>,
) -> ApiResult<Json<Value>> {
    let user = find_user_by_username(&state.pool, &req.username)
        .await?
        .ok_or_else(|| ApiError::unauthorized("用户名或密码错误"))?;
    let hash = hash_password(&req.password, &user.salt);
    if hash != user.password_hash {
        return Err(ApiError::unauthorized("用户名或密码错误"));
    }
    let token = gen_hex(16);
    sqlx::query("INSERT INTO sessions (token, user_id) VALUES (?1, ?2)")
        .bind(&token)
        .bind(user.id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "token": token, "user": user })))
}

async fn logout(State(state): State<AppState>, auth: AuthUser) -> ApiResult<Json<Value>> {
    // 本地模拟会话：登出即删除该用户的全部会话令牌
    sqlx::query("DELETE FROM sessions WHERE user_id = ?1")
        .bind(auth.id())
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

async fn me(auth: AuthUser) -> Json<Value> {
    Json(json!({ "user": auth.0 }))
}

async fn list_users(State(state): State<AppState>, auth: AuthUser) -> ApiResult<Json<Vec<User>>> {
    auth.require_staff()?;
    let users = sqlx::query_as::<_, User>(
        "SELECT * FROM users WHERE role = 'owner' ORDER BY id",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(users))
}

// ============================ 宠物 ============================

async fn list_pets(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<Pet>>> {
    let pets = if auth.is_staff() {
        sqlx::query_as::<_, Pet>("SELECT * FROM pets ORDER BY check_in_at DESC")
            .fetch_all(&state.pool)
            .await?
    } else {
        sqlx::query_as::<_, Pet>("SELECT * FROM pets WHERE owner_id = ?1 ORDER BY id DESC")
            .bind(auth.id())
            .fetch_all(&state.pool)
            .await?
    };
    Ok(Json(pets))
}

async fn get_pet(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let pet = load_pet(&state, id).await?;
    authorize_pet_read(&auth, &pet)?;
    let meds = sqlx::query_as::<_, Medication>(
        "SELECT * FROM medications WHERE pet_id = ?1 AND active = 1 ORDER BY id",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(json!({ "pet": pet, "medications": meds })))
}

async fn create_pet(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreatePetReq>,
) -> ApiResult<Json<Pet>> {
    if req.name.trim().is_empty() || req.species.trim().is_empty() {
        return Err(ApiError::bad_request("宠物名字和种类不能为空"));
    }
    let owner_id = if auth.is_staff() {
        let oid = req
            .owner_id
            .ok_or_else(|| ApiError::bad_request("员工登记宠物需指定主人"))?;
        let role: (String,) = sqlx::query_as("SELECT role FROM users WHERE id = ?1")
            .bind(oid)
            .fetch_optional(&state.pool)
            .await?
            .ok_or_else(|| ApiError::bad_request("指定的主人不存在"))?;
        if role.0 != "owner" {
            return Err(ApiError::bad_request("只能将宠物登记给主人账号"));
        }
        oid
    } else {
        // 主人自助登记：强制归属自己，忽略请求中的 owner_id
        auth.id()
    };
    let pet = sqlx::query_as::<_, Pet>(
        "INSERT INTO pets (name, species, owner_id, food_brand, food_portion,
                           meals_per_day, contraindications, notes)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8) RETURNING *",
    )
    .bind(req.name.trim())
    .bind(req.species.trim())
    .bind(owner_id)
    .bind(req.food_brand)
    .bind(req.food_portion)
    .bind(req.meals_per_day)
    .bind(req.contraindications)
    .bind(req.notes.unwrap_or_default())
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(pet))
}

async fn list_medications(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Vec<Medication>>> {
    let pet = load_pet(&state, id).await?;
    authorize_pet_read(&auth, &pet)?;
    let meds = sqlx::query_as::<_, Medication>(
        "SELECT * FROM medications WHERE pet_id = ?1 AND active = 1 ORDER BY id",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(meds))
}

async fn add_medication(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<UpsertMedicationReq>,
) -> ApiResult<Json<Medication>> {
    auth.require_staff()?;
    let pet = load_pet(&state, id).await?;
    if pet.status == "picked_up" {
        return Err(ApiError::conflict("宠物已接走，不可再维护用药"));
    }
    if req.name.trim().is_empty() || req.dosage.trim().is_empty() {
        return Err(ApiError::bad_request("药名和剂量不能为空"));
    }
    let med = sqlx::query_as::<_, Medication>(
        "INSERT INTO medications (pet_id, name, dosage, instructions)
         VALUES (?1,?2,?3,?4) RETURNING *",
    )
    .bind(id)
    .bind(req.name.trim())
    .bind(req.dosage.trim())
    .bind(req.instructions)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(med))
}

/// 主人提前接走：标记离店并取消后续所有待执行任务
async fn checkout_pet(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    auth.require_staff()?;
    let pet = load_pet(&state, id).await?;
    if pet.status == "picked_up" {
        return Err(ApiError::conflict("该宠物已接走"));
    }
    let n = cancel_future_tasks(&state, id).await?;
    sqlx::query("UPDATE pets SET status='picked_up', check_out_at=datetime('now','localtime') WHERE id=?1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    // 代接授权随之全部失效
    sqlx::query(
        "UPDATE pickup_codes SET status='revoked', revoked_at=datetime('now','localtime')
         WHERE pet_id=?1 AND status='active'",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;
    tracing::info!("宠物 {} 已接走，取消后续任务 {} 个", id, n);
    Ok(Json(json!({ "ok": true, "cancelled_tasks": n })))
}

// ============================ 照护任务 ============================

async fn list_tasks(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Value>> {
    if let Some(pid) = q.pet_id {
        let pet = load_pet(&state, pid).await?;
        authorize_pet_read(&auth, &pet)?;
    } else if !auth.is_staff() {
        // 主人必须限定自己的宠物
        return Err(ApiError::bad_request("请指定 pet_id"));
    }

    let mut sql = String::from(
        "SELECT * FROM care_tasks WHERE 1=1",
    );
    if q.pet_id.is_some() {
        sql.push_str(" AND pet_id = ?");
    }
    if q.date.is_some() {
        sql.push_str(" AND scheduled_at LIKE ?");
    }
    if q.status.is_some() {
        sql.push_str(" AND status = ?");
    }
    sql.push_str(" ORDER BY scheduled_at");

    let mut query = sqlx::query_as::<_, CareTask>(&sql);
    if let Some(pid) = q.pet_id {
        query = query.bind(pid);
    }
    if let Some(d) = q.date {
        query = query.bind(format!("{d}%"));
    }
    if let Some(s) = q.status {
        query = query.bind(s);
    }
    let tasks = query.fetch_all(&state.pool).await?;
    let enriched = enrich_tasks(&state, tasks).await?;
    Ok(Json(json!({ "tasks": enriched })))
}

async fn get_task(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let task = load_task(&state, id).await?;
    let pet = load_pet(&state, task.pet_id).await?;
    authorize_pet_read(&auth, &pet)?;
    let enriched = enrich_tasks(&state, vec![task]).await?.remove(0);
    Ok(Json(enriched))
}

async fn create_task(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateTaskReq>,
) -> ApiResult<Json<CareTask>> {
    auth.require_staff()?;
    if !matches!(req.kind.as_str(), "feed" | "medication") {
        return Err(ApiError::bad_request("任务类型只能是 feed 或 medication"));
    }
    let pet = load_pet(&state, req.pet_id).await?;
    if pet.status == "picked_up" {
        return Err(ApiError::conflict("宠物已接走，不能再安排任务"));
    }
    let mut med_id: Option<i64> = None;
    if req.kind == "medication" {
        let mid = req
            .med_id
            .ok_or_else(|| ApiError::bad_request("给药任务必须选择药品"))?;
        let exists: (i64,) =
            sqlx::query_as("SELECT COUNT(*) FROM medications WHERE id=?1 AND pet_id=?2 AND active=1")
                .bind(mid)
                .bind(req.pet_id)
                .fetch_one(&state.pool)
                .await?;
        if exists.0 == 0 {
            return Err(ApiError::bad_request("药品不存在或已停用"));
        }
        med_id = Some(mid);
    }
    let task = sqlx::query_as::<_, CareTask>(
        "INSERT INTO care_tasks (pet_id, kind, med_id, scheduled_at, food_brand, portion, note, created_by)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8) RETURNING *",
    )
    .bind(req.pet_id)
    .bind(&req.kind)
    .bind(med_id)
    .bind(&req.scheduled_at)
    .bind(&req.food_brand)
    .bind(&req.portion)
    .bind(&req.note)
    .bind(auth.id())
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(task))
}

async fn cancel_task(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    auth.require_staff()?;
    let task = load_task(&state, id).await?;
    if task.status == "done" {
        return Err(ApiError::conflict("已完成的任务不可取消（异常请单独上报）"));
    }
    sqlx::query("UPDATE care_tasks SET status='cancelled' WHERE id=?1 AND status='scheduled'")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

/// 执行任务（JSON，无照片）
async fn execute_task(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    Json(req): Json<ExecuteTaskReq>,
) -> ApiResult<Json<Value>> {
    auth.require_staff()?;
    let result = do_execute(
        &state,
        id,
        auth.id(),
        req.note.unwrap_or_default(),
        String::new(),
        req.verifier_username,
        req.verifier_password,
    )
    .await?;
    Ok(Json(result))
}

/// 执行任务（multipart：可附照片 + 核对员工凭证）
async fn execute_task_photo(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
    mut multipart: Multipart,
) -> ApiResult<Json<Value>> {
    auth.require_staff()?;
    let mut note = String::new();
    let mut photo_path = String::new();
    let mut verifier_username = None;
    let mut verifier_password = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| ApiError::bad_request(e.to_string()))? {
        let name = field.name().unwrap_or("").to_string();
        match name.as_str() {
            "note" => note = text_field(field).await?,
            "verifier_username" => verifier_username = Some(text_field(field).await?),
            "verifier_password" => verifier_password = Some(text_field(field).await?),
            "photo" => {
                photo_path = save_photo(&state, field).await?;
            }
            _ => {}
        }
    }

    let result = do_execute(
        &state,
        id,
        auth.id(),
        note,
        photo_path,
        verifier_username,
        verifier_password,
    )
    .await?;
    Ok(Json(result))
}

/// 核心执行逻辑：
/// 1. 幂等：任务已有执行记录 / 非 scheduled 状态 → 409，重复提交不会执行第二次
/// 2. 给药必须由「另一名员工」凭账号密码核对药名与剂量
async fn do_execute(
    state: &AppState,
    task_id: i64,
    executor_id: i64,
    note: String,
    photo_path: String,
    verifier_username: Option<String>,
    verifier_password: Option<String>,
) -> ApiResult<Value> {
    let task = load_task(state, task_id).await?;

    if task.status == "cancelled" {
        return Err(ApiError::conflict("该任务已取消，不能执行"));
    }
    // 幂等第一道闸：先查已有记录
    let existing: Option<(i64,)> =
        sqlx::query_as("SELECT id FROM task_executions WHERE task_id=?1")
            .bind(task_id)
            .fetch_optional(&state.pool)
            .await?;
    if existing.is_some() || task.status == "done" {
        return Err(ApiError::conflict(
            "该任务已执行并记录，重复提交不会重复执行；如有异常请单独上报",
        ));
    }

    let mut verifier_id: Option<i64> = None;
    let mut verified_med_name = String::new();
    let mut verified_dosage = String::new();

    if task.kind == "medication" {
        let vname = verifier_username
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ApiError::bad_request("给药任务必须由另一名员工核对：请填写核对人工号"))?;
        let vpass = verifier_password
            .filter(|s| !s.is_empty())
            .ok_or_else(|| ApiError::bad_request("给药任务必须由另一名员工核对：请填写核对人密码"))?;

        let verifier = find_user_by_username(&state.pool, &vname)
            .await?
            .ok_or_else(|| ApiError::bad_request("核对人账号或密码错误"))?;
        if verifier.role != "staff" {
            return Err(ApiError::bad_request("核对人必须是门店员工"));
        }
        if hash_password(&vpass, &verifier.salt) != verifier.password_hash {
            return Err(ApiError::bad_request("核对人账号或密码错误"));
        }
        if verifier.id == executor_id {
            return Err(ApiError::bad_request(
                "给药核对必须由另一名员工完成，不能由执行本人核对",
            ));
        }

        // 核对内容：药名 + 剂量（取任务计划药品作为核对留痕快照）
        let med = sqlx::query_as::<_, Medication>(
            "SELECT m.* FROM medications m
             JOIN care_tasks t ON t.med_id = m.id WHERE t.id=?1",
        )
        .bind(task_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::bad_request("该给药任务未关联有效药品，无法核对，请联系门店重新安排"))?;
        verified_med_name = med.name;
        verified_dosage = med.dosage;
        verifier_id = Some(verifier.id);
    }

    // 幂等第二道闸：task_id UNIQUE，并发重复提交由数据库拒绝
    let mut tx = state.pool.begin().await?;
    let exec_result = sqlx::query(
        "INSERT INTO task_executions
            (task_id, executed_by, note, photo_path, verifier_id, verified_at,
             verifier_med_name, verifier_dosage)
         VALUES (?1,?2,?3,?4,?5,datetime('now','localtime'),?6,?7)",
    )
    .bind(task_id)
    .bind(executor_id)
    .bind(&note)
    .bind(&photo_path)
    .bind(verifier_id)
    .bind(&verified_med_name)
    .bind(&verified_dosage)
    .execute(&mut *tx)
    .await;

    if let Err(e) = exec_result {
        // UNIQUE 失败 → 认定为重复提交
        if let sqlx::error::Error::Database(db) = &e {
            if db.message().contains("UNIQUE") {
                return Err(ApiError::conflict(
                    "该任务已存在执行记录，重复提交被拒绝（幂等）",
                ));
            }
        }
        return Err(ApiError::Sqlx(e));
    }

    sqlx::query("UPDATE care_tasks SET status='done' WHERE id=?1 AND status='scheduled'")
        .bind(task_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    tracing::info!(
        "任务 {}（{}）由员工 {} 执行完成{}",
        task_id,
        task.kind,
        executor_id,
        if verifier_id.is_some() {
            format!("，给药经员工 {} 双人核对", verifier_id.unwrap())
        } else {
            String::new()
        }
    );

    Ok(json!({
        "ok": true,
        "task_id": task_id,
        "photo_path": photo_path,
        "verified_by": verifier_id,
        "verified_med_name": verified_med_name,
        "verified_dosage": verified_dosage,
    }))
}

// ============================ 异常事件 ============================

async fn list_incidents(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Vec<Value>>> {
    let pet_id = q
        .pet_id
        .ok_or_else(|| ApiError::bad_request("请指定 pet_id"))?;
    let pet = load_pet(&state, pet_id).await?;
    authorize_pet_read(&auth, &pet)?;

    let rows = sqlx::query_as::<_, Incident>(
        "SELECT * FROM incidents WHERE pet_id=?1 ORDER BY created_at DESC",
    )
    .bind(pet_id)
    .fetch_all(&state.pool)
    .await?;

    let mut out = Vec::new();
    for inc in rows {
        let reporter = display_name(&state, inc.reported_by).await?;
        out.push(json!({ "incident": inc, "reporter_name": reporter }));
    }
    Ok(Json(out))
}

/// 拒食 / 漏服 / 临时换粮：必须写清处理经过，并立即通知主人 —— 不能靠勾选完成掩盖
async fn create_incident(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreateIncidentReq>,
) -> ApiResult<Json<Value>> {
    auth.require_staff()?;
    if !matches!(
        req.incident_type.as_str(),
        "refused_food" | "missed_medication" | "food_change" | "other"
    ) {
        return Err(ApiError::bad_request("异常类型不合法"));
    }
    if req.description.trim().is_empty() {
        return Err(ApiError::bad_request("请描述异常情况"));
    }
    if req.handling.trim().is_empty() {
        return Err(ApiError::bad_request(
            "必须填写处理经过，不能将异常直接标记为完成",
        ));
    }
    let pet = load_pet(&state, req.pet_id).await?;

    if let Some(tid) = req.task_id {
        let task = load_task(&state, tid).await?;
        if task.pet_id != req.pet_id {
            return Err(ApiError::bad_request("任务与宠物不匹配"));
        }
    }

    let mut tx = state.pool.begin().await?;
    let inc: (i64,) = sqlx::query_as(
        "INSERT INTO incidents (pet_id, task_id, incident_type, description, handling, reported_by)
         VALUES (?1,?2,?3,?4,?5,?6) RETURNING id",
    )
    .bind(req.pet_id)
    .bind(req.task_id)
    .bind(&req.incident_type)
    .bind(req.description.trim())
    .bind(req.handling.trim())
    .bind(auth.id())
    .fetch_one(&mut *tx)
    .await?;

    let type_label = incident_label(&req.incident_type);
    let title = format!("【{type_label}】{} 的照护异常通知", pet.name);
    let body = format!(
        "情况：{}\n处理经过：{}\n记录人员可在系统查看详情。",
        req.description.trim(),
        req.handling.trim()
    );
    sqlx::query(
        "INSERT INTO notifications (owner_id, pet_id, incident_id, title, body)
         VALUES (?1,?2,?3,?4,?5)",
    )
    .bind(pet.owner_id)
    .bind(pet.id)
    .bind(inc.0)
    .bind(&title)
    .bind(&body)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;

    // 本地模拟通知推送
    tracing::warn!("🔔 模拟通知 → 主人({})：{title}｜{body}", pet.owner_id);

    Ok(Json(json!({ "ok": true, "incident_id": inc.0, "notified": true })))
}

// ============================ 通知 ============================

async fn list_notifications(
    State(state): State<AppState>,
    auth: AuthUser,
) -> ApiResult<Json<Vec<Notification>>> {
    if auth.is_staff() {
        // 门店可看全部通知留痕
        let rows = sqlx::query_as::<_, Notification>(
            "SELECT * FROM notifications ORDER BY created_at DESC LIMIT 200",
        )
        .fetch_all(&state.pool)
        .await?;
        Ok(Json(rows))
    } else {
        let rows = sqlx::query_as::<_, Notification>(
            "SELECT * FROM notifications WHERE owner_id=?1 ORDER BY created_at DESC",
        )
        .bind(auth.id())
        .fetch_all(&state.pool)
        .await?;
        Ok(Json(rows))
    }
}

async fn read_notification(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let res = sqlx::query("UPDATE notifications SET is_read=1 WHERE id=?1 AND owner_id=?2")
        .bind(id)
        .bind(auth.id())
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(ApiError::not_found("通知不存在或无权操作"));
    }
    Ok(Json(json!({ "ok": true })))
}

// ============================ 取宠码 / 代接授权 ============================

#[derive(FromRow)]
struct PickupRow {
    id: i64,
    pet_id: i64,
    code: String,
    authorized_name: String,
    status: String,
    created_by: i64,
    created_at: String,
    revoked_at: Option<String>,
    redeemed_at: Option<String>,
    redeemed_by: Option<i64>,
}

async fn list_pickup_codes(
    State(state): State<AppState>,
    auth: AuthUser,
    Query(q): Query<ListQuery>,
) -> ApiResult<Json<Vec<Value>>> {
    let pet_id = q
        .pet_id
        .ok_or_else(|| ApiError::bad_request("请指定 pet_id"))?;
    let pet = load_pet(&state, pet_id).await?;
    authorize_pet_read(&auth, &pet)?;

    let rows = sqlx::query_as::<_, PickupRow>(
        "SELECT * FROM pickup_codes WHERE pet_id=?1 ORDER BY created_at DESC",
    )
    .bind(pet_id)
    .fetch_all(&state.pool)
    .await?;

    // 只有主人能看到完整取宠码（用于转交代接人）；员工列表中打码
    Ok(Json(
        rows.into_iter()
            .map(|r| {
                let code_out = if auth.is_staff() && r.status == "active" {
                    "****（凭代接人提供的码核销）".to_string()
                } else {
                    r.code
                };
                json!({
                    "id": r.id,
                    "pet_id": r.pet_id,
                    "code": code_out,
                    "authorized_name": r.authorized_name,
                    "status": r.status,
                    "created_by": r.created_by,
                    "created_at": r.created_at,
                    "revoked_at": r.revoked_at,
                    "redeemed_at": r.redeemed_at,
                    "redeemed_by": r.redeemed_by,
                })
            })
            .collect(),
    ))
}

async fn create_pickup_code(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<CreatePickupCodeReq>,
) -> ApiResult<Json<Value>> {
    if req.authorized_name.trim().is_empty() {
        return Err(ApiError::bad_request("请填写被授权代接人姓名"));
    }
    let pet = load_pet(&state, req.pet_id).await?;
    // 主人只能给自己的宠物开码；员工也可代开
    if !auth.is_staff() && pet.owner_id != auth.id() {
        return Err(ApiError::forbidden("只能为自己的宠物创建代接授权"));
    }
    if pet.status == "picked_up" {
        return Err(ApiError::conflict("宠物已接走"));
    }
    let code = gen_pickup_code();
    let row: (i64, String) = sqlx::query_as(
        "INSERT INTO pickup_codes (pet_id, code, authorized_name, created_by)
         VALUES (?1,?2,?3,?4) RETURNING id, code",
    )
    .bind(req.pet_id)
    .bind(&code)
    .bind(req.authorized_name.trim())
    .bind(auth.id())
    .fetch_one(&state.pool)
    .await?;

    tracing::info!(
        "🔔 模拟通知 → 门店：宠物 {} 新增代接授权，代接人「{}」取宠码 {}",
        req.pet_id,
        req.authorized_name.trim(),
        code
    );
    Ok(Json(json!({ "id": row.0, "code": row.1, "authorized_name": req.authorized_name })))
}

/// 撤销代接授权：旧取宠码立即失效
async fn revoke_pickup_code(
    State(state): State<AppState>,
    auth: AuthUser,
    Path(id): Path<i64>,
) -> ApiResult<Json<Value>> {
    let pc = sqlx::query_as::<_, PickupRow>("SELECT * FROM pickup_codes WHERE id=?1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("取宠码不存在"))?;
    let pet = load_pet(&state, pc.pet_id).await?;
    if !auth.is_staff() && pet.owner_id != auth.id() {
        return Err(ApiError::forbidden("只能撤销自己宠物的代接授权"));
    }
    if pc.status != "active" {
        return Err(ApiError::conflict(format!("该授权已是「{}」状态，无需撤销", pc.status)));
    }
    sqlx::query(
        "UPDATE pickup_codes SET status='revoked', revoked_at=datetime('now','localtime') WHERE id=?1",
    )
    .bind(id)
    .execute(&state.pool)
    .await?;
    tracing::warn!("🔔 代接授权 {} 已撤销，旧取宠码 {} 立即失效", id, pc.code);
    Ok(Json(json!({ "ok": true, "invalidated_code": pc.code })))
}

/// 门店核销取宠码：撤销 / 已使用 / 不存在一律拦截，旧码绝不能再取走宠物
async fn redeem_pickup_code(
    State(state): State<AppState>,
    auth: AuthUser,
    Json(req): Json<RedeemCodeReq>,
) -> ApiResult<Json<Value>> {
    auth.require_staff()?;
    let code = req.code.trim();
    if code.is_empty() {
        return Err(ApiError::bad_request("请输入取宠码"));
    }
    let pc = sqlx::query_as::<_, PickupRow>("SELECT * FROM pickup_codes WHERE code=?1")
        .bind(code)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::forbidden("取宠码无效：查无此码，拒绝放宠"))?;

    match pc.status.as_str() {
        "revoked" => {
            return Err(ApiError::forbidden(
                "取宠码已被主人撤销并立即失效，拒绝放宠，请联系主人重新授权",
            ))
        }
        "redeemed" => {
            return Err(ApiError::conflict("取宠码已使用过，不能重复领取"))
        }
        _ => {}
    }
    let pet = load_pet(&state, pc.pet_id).await?;
    if pet.status == "picked_up" {
        return Err(ApiError::conflict("该宠物已被接走"));
    }

    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "UPDATE pickup_codes SET status='redeemed', redeemed_at=datetime('now','localtime'),
             redeemed_by=?1 WHERE id=?2 AND status='active'",
    )
    .bind(auth.id())
    .bind(pc.id)
    .execute(&mut *tx)
    .await?;
    // 同只宠物的其他有效码一并失效
    sqlx::query(
        "UPDATE pickup_codes SET status='revoked', revoked_at=datetime('now','localtime')
         WHERE pet_id=?1 AND status='active'",
    )
    .bind(pc.pet_id)
    .execute(&mut *tx)
    .await?;
    // 接走：取消后续任务
    sqlx::query(
        "UPDATE care_tasks SET status='cancelled'
         WHERE pet_id=?1 AND status='scheduled'
         AND scheduled_at > datetime('now','localtime')",
    )
    .bind(pc.pet_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE pets SET status='picked_up', check_out_at=datetime('now','localtime') WHERE id=?1")
        .bind(pc.pet_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;

    tracing::info!(
        "✅ 代接人「{}」凭码 {} 接走宠物 {}，后续任务已取消",
        pc.authorized_name,
        code,
        pc.pet_id
    );
    Ok(Json(json!({
        "ok": true,
        "pet_id": pc.pet_id,
        "pet_name": pet.name,
        "authorized_name": pc.authorized_name,
    })))
}

// ============================ 辅助函数 ============================

async fn load_pet(state: &AppState, id: i64) -> ApiResult<Pet> {
    sqlx::query_as::<_, Pet>("SELECT * FROM pets WHERE id=?1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("宠物不存在"))
}

async fn load_task(state: &AppState, id: i64) -> ApiResult<CareTask> {
    sqlx::query_as::<_, CareTask>("SELECT * FROM care_tasks WHERE id=?1")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or_else(|| ApiError::not_found("任务不存在"))
}

/// 主人只能读自己的宠物；员工可读全部
fn authorize_pet_read(auth: &AuthUser, pet: &Pet) -> ApiResult<()> {
    if !auth.is_staff() && pet.owner_id != auth.id() {
        return Err(ApiError::forbidden("只能查看自己名下宠物的记录"));
    }
    Ok(())
}

async fn cancel_future_tasks(state: &AppState, pet_id: i64) -> ApiResult<u64> {
    let res = sqlx::query(
        "UPDATE care_tasks SET status='cancelled'
         WHERE pet_id=?1 AND status='scheduled'
         AND scheduled_at >= datetime('now','localtime')",
    )
    .bind(pet_id)
    .execute(&state.pool)
    .await?;
    Ok(res.rows_affected())
}

async fn display_name(state: &AppState, user_id: i64) -> ApiResult<String> {
    let row: Option<(String,)> = sqlx::query_as("SELECT display_name FROM users WHERE id=?1")
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?;
    Ok(row.map(|r| r.0).unwrap_or_else(|| format!("用户{user_id}")))
}

#[derive(FromRow)]
struct ExRow {
    executed_by: i64,
    executed_at: String,
    note: String,
    photo_path: String,
    verifier_id: Option<i64>,
    verified_at: Option<String>,
    verifier_med_name: String,
    verifier_dosage: String,
}

async fn enrich_tasks(state: &AppState, tasks: Vec<CareTask>) -> ApiResult<Vec<Value>> {
    let mut out = Vec::with_capacity(tasks.len());
    for t in tasks {
        let exec = sqlx::query_as::<_, ExRow>("SELECT * FROM task_executions WHERE task_id=?1")
            .bind(t.id)
            .fetch_optional(&state.pool)
            .await?;
        let med: Option<Medication> = if let Some(mid) = t.med_id {
            sqlx::query_as::<_, Medication>("SELECT * FROM medications WHERE id=?1")
                .bind(mid)
                .fetch_optional(&state.pool)
                .await?
        } else {
            None
        };
        let mut v = serde_json::to_value(&t).unwrap_or(json!({}));
        if let Some(med) = med {
            v["medication"] = json!({ "name": med.name, "dosage": med.dosage, "instructions": med.instructions });
        }
        if let Some(e) = exec {
            let verifier_name = match e.verifier_id {
                Some(id) => json!(display_name(state, id).await?),
                None => Value::Null,
            };
            v["execution"] = json!({
                "executed_at": e.executed_at,
                "note": e.note,
                "photo_path": e.photo_path,
                "executor_name": display_name(state, e.executed_by).await?,
                "verifier_id": e.verifier_id,
                "verifier_name": verifier_name,
                "verified_at": e.verified_at,
                "verifier_med_name": e.verifier_med_name,
                "verifier_dosage": e.verifier_dosage,
            });
        }
        out.push(v);
    }
    Ok(out)
}

async fn text_field(field: Field<'_>) -> ApiResult<String> {
    let bytes = field
        .bytes()
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    String::from_utf8(bytes.to_vec()).map_err(|_| ApiError::bad_request("文本字段编码错误"))
}

async fn save_photo(state: &AppState, field: Field<'_>) -> ApiResult<String> {
    let filename = field.file_name().unwrap_or("photo").to_string();
    let ext = filename
        .rsplit('.')
        .next()
        .filter(|s| s.len() <= 5 && s.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("jpg");
    let bytes = field
        .bytes()
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    if bytes.len() > 10 * 1024 * 1024 {
        return Err(ApiError::bad_request("照片不能超过 10MB"));
    }
    let name = format!("{}.{}", gen_hex(12), ext);
    let path = format!("{}/{}", state.uploads_dir, name);
    tokio::fs::write(&path, &bytes).await?;
    tracing::info!("照片已本地保存: {path}（{} 字节）", bytes.len());
    Ok(format!("/uploads/{name}"))
}

fn gen_hex(n: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..n).map(|_| format!("{:x}", rng.gen_range(0..16u8))).collect()
}

fn gen_pickup_code() -> String {
    let mut rng = rand::thread_rng();
    (0..8)
        .map(|_| rng.gen_range(0..10u8).to_string())
        .collect::<String>()
}

fn incident_label(t: &str) -> &'static str {
    match t {
        "refused_food" => "拒食",
        "missed_medication" => "漏服药物",
        "food_change" => "临时换粮",
        _ => "其他异常",
    }
}
