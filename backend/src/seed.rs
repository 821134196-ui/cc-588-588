use crate::app::{AppResult, AppState};
use crate::auth::{hash_password, now_iso};
use crate::schedule::ensure_care_calendar;
use chrono::{Duration, Utc};

async fn user_count(state: &AppState) -> i64 {
    sqlx::query_scalar("SELECT COUNT(*) FROM users")
        .fetch_one(state.pool())
        .await
        .unwrap_or(0)
}

async fn create_user(
    state: &AppState,
    id: &str,
    username: &str,
    password: &str,
    role: &str,
    display_name: &str,
) -> AppResult<()> {
    let salt = uuid::Uuid::new_v4().simple().to_string();
    let hash = hash_password(&salt, password);
    sqlx::query(
        "INSERT INTO users (id, username, password_salt, password_hash, role, display_name, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id)
    .bind(username)
    .bind(salt)
    .bind(hash)
    .bind(role)
    .bind(display_name)
    .bind(now_iso())
    .execute(state.pool())
    .await?;
    Ok(())
}

/// 首次启动时写入演示数据：2 名员工、2 位主人、2 只宠物
pub async fn seed_if_empty(state: &AppState) -> AppResult<()> {
    if user_count(state).await > 0 {
        return Ok(());
    }

    create_user(state, "u-staff-alice", "alice", "staff123", "staff", "王护工").await?;
    create_user(state, "u-staff-bob", "bob", "staff123", "staff", "陈护工").await?;
    create_user(state, "u-owner-li", "lisi", "owner123", "owner", "李小姐").await?;
    create_user(state, "u-owner-zhang", "zhangsan", "owner123", "owner", "张先生").await?;

    let today = Utc::now().format("%Y-%m-%d").to_string();
    let end = (Utc::now() + Duration::days(5)).format("%Y-%m-%d").to_string();

    // 豆豆：狗粮 150g/餐，禁忌巧克力/葡萄，口服药一天两次
    sqlx::query(
        "INSERT INTO pets (id, owner_id, name, species, food_portion, food_brand,
             contraindications, feeding_times, notes, boarding_start, boarding_end, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("pet-doudou")
    .bind("u-owner-li")
    .bind("豆豆")
    .bind("狗 / 金毛 · 3岁")
    .bind("150g/餐，温水泡软")
    .bind("某成犬粮")
    .bind("巧克力、葡萄、洋葱；鸡肉轻度过敏")
    .bind("08:00,12:30,18:30")
    .bind("性格温顺，吃药可裹在零食里")
    .bind(&today)
    .bind(&end)
    .bind(now_iso())
    .execute(state.pool())
    .await?;

    sqlx::query(
        "INSERT INTO medications (id, pet_id, drug_name, dosage, instructions, times, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("med-doudou-1")
    .bind("pet-doudou")
    .bind("阿莫西林")
    .bind("50mg")
    .bind("餐后服用，整片裹零食")
    .bind("08:30,19:00")
    .bind(now_iso())
    .execute(state.pool())
    .await?;

    // 咪咪：猫粮 30g/餐，换粮期
    sqlx::query(
        "INSERT INTO pets (id, owner_id, name, species, food_portion, food_brand,
             contraindications, feeding_times, notes, boarding_start, boarding_end, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind("pet-mimi")
    .bind("u-owner-zhang")
    .bind("咪咪")
    .bind("猫 / 英短 · 2岁")
    .bind("30g/餐，少量多餐")
    .bind("旧粮与新处方粮 1:1 过渡")
    .bind("禁食人类乳制品；勿喂鱼罐头")
    .bind("07:30,12:00,18:00,22:00")
    .bind("胆小，喂食后请离开房间让它独自进食")
    .bind(&today)
    .bind(&end)
    .bind(now_iso())
    .execute(state.pool())
    .await?;

    for pet_id in ["pet-doudou", "pet-mimi"] {
        let n = ensure_care_calendar(state, pet_id).await?;
        let _ = n;
    }
    tracing::info!("已写入演示账号与示例宠物，并生成照护日历");
    Ok(())
}
