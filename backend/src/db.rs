use crate::models::User;
use sha2::{Digest, Sha256};
use sqlx::SqlitePool;

const MIGRATION: &str = include_str!("../migrations/0001_init.sql");

pub async fn migrate(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    // 单文件项目：整段迁移在一个事务里执行；SQLx 会按分号拆分语句
    let mut conn = pool.acquire().await?;
    // sqlx::raw_sql 支持多语句执行
    sqlx::raw_sql(MIGRATION)
        .execute(&mut *conn)
        .await?;
    Ok(())
}

pub fn hash_password(password: &str, salt: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(format!("{salt}:{password}").as_bytes());
    format!("{:x}", hasher.finalize())
}

pub fn new_salt() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 8] = rng.gen();
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub async fn find_user_by_username(
    pool: &SqlitePool,
    username: &str,
) -> Result<Option<User>, sqlx::Error> {
    sqlx::query_as::<_, User>("SELECT * FROM users WHERE username = ?1")
        .bind(username)
        .fetch_optional(pool)
        .await
}

/// 初始化演示数据（仅在库为空时）
pub async fn seed(pool: &SqlitePool) -> Result<(), sqlx::Error> {
    let count: (i64,) = sqlx::query_as("SELECT COUNT(*) FROM users")
        .fetch_one(pool)
        .await?;
    if count.0 > 0 {
        return Ok(());
    }

    let mut tx = pool.begin().await?;

    // 员工两名（给药双人核对演示）
    let _alice = insert_user(&mut *tx, "alice", "员工·小艾", "staff", "staff123").await?;
    let _bob = insert_user(&mut *tx, "bob", "员工·小博", "staff", "staff123").await?;
    // 主人两名（越权隔离演示）
    let wang = insert_user(&mut *tx, "wang", "王女士", "owner", "owner123").await?;
    let _li = insert_user(&mut *tx, "li", "李先生", "owner", "owner123").await?;

    // 王女士的猫：有每日用药
    let mimi: (i64,) = sqlx::query_as(
        "INSERT INTO pets (name, species, owner_id, food_brand, food_portion, meals_per_day,
                           contraindications, notes)
         VALUES ('咪咪','猫',?1,'皇家成猫粮','每餐 40g',2,
                 '禁食牛奶、巧克力；对鸡肉轻度过敏','胆小，喂药后请陪一会儿')
         RETURNING id",
    )
    .bind(wang)
    .fetch_one(&mut *tx)
    .await?;
    let mimi_id = mimi.0;

    let med: (i64,) = sqlx::query_as(
        "INSERT INTO medications (pet_id, name, dosage, instructions)
         VALUES (?1,'阿莫西林','5mg（约半片）','早晚各一次，随餐服用，需双人核对')
         RETURNING id",
    )
    .bind(mimi_id)
    .fetch_one(&mut *tx)
    .await?;
    let med_id = med.0;

    // 今天的喂食与给药任务
    let today = chrono::Local::now().format("%Y-%m-%d").to_string();
    sqlx::query(
        "INSERT INTO care_tasks (pet_id, kind, med_id, scheduled_at, food_brand, portion, note, created_by)
         VALUES (?1,'feed',NULL,?2,'皇家成猫粮','40g','早餐',1),
                (?1,'feed',NULL,?3,'皇家成猫粮','40g','晚餐',1),
                (?1,'medication',?5,?4,'','','阿莫西林 5mg 随早餐服用；需另一员工核对药名剂量',1)",
    )
    .bind(mimi_id)
    .bind(format!("{today} 08:00"))
    .bind(format!("{today} 18:00"))
    .bind(format!("{today} 08:10"))
    .bind(med_id)
    .execute(&mut *tx)
    .await?;

    // 李先生的狗（无用药，用于越权对照）
    let doudou: (i64,) = sqlx::query_as(
        "INSERT INTO pets (name, species, owner_id, food_brand, food_portion, meals_per_day,
                           contraindications)
         VALUES ('豆豆','狗',?1,'比瑞吉幼犬粮','每餐 80g',2,'禁食葡萄、洋葱')
         RETURNING id",
    )
    .bind(_li)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO care_tasks (pet_id, kind, scheduled_at, food_brand, portion, note, created_by)
         VALUES (?1,'feed',?2,'比瑞吉幼犬粮','80g','早餐',1)",
    )
    .bind(doudou.0)
    .bind(format!("{today} 07:30"))
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    tracing::info!(
        "演示数据已就绪：员工 alice/bob（密码 staff123），主人 wang/li（密码 owner123）"
    );
    Ok(())
}

async fn insert_user(
    conn: &mut sqlx::SqliteConnection,
    username: &str,
    display: &str,
    role: &str,
    password: &str,
) -> Result<i64, sqlx::Error> {
    let salt = new_salt();
    let hash = hash_password(password, &salt);
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO users (username, password_hash, salt, display_name, role)
         VALUES (?1, ?2, ?3, ?4, ?5) RETURNING id",
    )
    .bind(username)
    .bind(hash)
    .bind(salt)
    .bind(display)
    .bind(role)
    .fetch_one(conn)
    .await?;
    Ok(row.0)
}
