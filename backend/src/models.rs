use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct User {
    pub id: i64,
    pub username: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    #[serde(skip_serializing)]
    pub salt: String,
    pub display_name: String,
    pub role: String,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Pet {
    pub id: i64,
    pub name: String,
    pub species: String,
    pub owner_id: i64,
    pub food_brand: String,
    pub food_portion: String,
    pub meals_per_day: i64,
    pub contraindications: String,
    pub notes: String,
    pub status: String,
    pub check_in_at: String,
    pub check_out_at: Option<String>,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Medication {
    pub id: i64,
    pub pet_id: i64,
    pub name: String,
    pub dosage: String,
    pub instructions: String,
    pub active: i64,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct CareTask {
    pub id: i64,
    pub pet_id: i64,
    pub kind: String,
    pub med_id: Option<i64>,
    pub scheduled_at: String,
    pub food_brand: String,
    pub portion: String,
    pub note: String,
    pub status: String,
    pub created_by: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[allow(dead_code)]
pub struct TaskExecution {
    pub id: i64,
    pub task_id: i64,
    pub executed_by: i64,
    pub executed_at: String,
    pub note: String,
    pub photo_path: String,
    pub verifier_id: Option<i64>,
    pub verified_at: Option<String>,
    pub verifier_med_name: String,
    pub verifier_dosage: String,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Incident {
    pub id: i64,
    pub pet_id: i64,
    pub task_id: Option<i64>,
    pub incident_type: String,
    pub description: String,
    pub handling: String,
    pub reported_by: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow, Serialize)]
pub struct Notification {
    pub id: i64,
    pub owner_id: i64,
    pub pet_id: i64,
    pub incident_id: Option<i64>,
    pub title: String,
    pub body: String,
    pub is_read: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, FromRow, Serialize)]
#[allow(dead_code)]
pub struct PickupCode {
    pub id: i64,
    pub pet_id: i64,
    #[serde(skip_serializing)]
    pub code: String,
    pub authorized_name: String,
    pub status: String,
    pub created_by: i64,
    pub created_at: String,
    pub revoked_at: Option<String>,
    pub redeemed_at: Option<String>,
    pub redeemed_by: Option<i64>,
}

// ---------- 请求 DTO ----------

#[derive(Debug, Deserialize)]
pub struct LoginReq {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePetReq {
    pub name: String,
    pub species: String,
    pub owner_id: Option<i64>, // 员工可指定主人；省略时由主人自助登记
    pub food_brand: String,
    pub food_portion: String,
    #[serde(default = "default_meals")]
    pub meals_per_day: i64,
    pub contraindications: String,
    pub notes: Option<String>,
}

fn default_meals() -> i64 {
    2
}

#[derive(Debug, Deserialize)]
pub struct UpsertMedicationReq {
    pub name: String,
    pub dosage: String,
    #[serde(default)]
    pub instructions: String,
}

#[derive(Debug, Deserialize)]
pub struct CreateTaskReq {
    pub pet_id: i64,
    pub kind: String, // feed | medication
    pub med_id: Option<i64>,
    pub scheduled_at: String,
    #[serde(default)]
    pub food_brand: String,
    #[serde(default)]
    pub portion: String,
    #[serde(default)]
    pub note: String,
}

/// 员工执行任务（JSON 版，无照片；照片走 /execute-with-photo multipart）
#[derive(Debug, Deserialize)]
pub struct ExecuteTaskReq {
    pub note: Option<String>,
    // 给药双人核对：另一名员工的登录凭证
    pub verifier_username: Option<String>,
    pub verifier_password: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateIncidentReq {
    pub pet_id: i64,
    pub task_id: Option<i64>,
    pub incident_type: String, // refused_food | missed_medication | food_change | other
    pub description: String,
    pub handling: String,
}

#[derive(Debug, Deserialize)]
pub struct CreatePickupCodeReq {
    pub pet_id: i64,
    pub authorized_name: String,
}

#[derive(Debug, Deserialize)]
pub struct RedeemCodeReq {
    pub code: String,
}

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    pub pet_id: Option<i64>,
    pub date: Option<String>, // YYYY-MM-DD，按本地日期过滤
    pub status: Option<String>,
}
