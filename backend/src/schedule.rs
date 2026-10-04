use crate::app::{AppResult, AppState};
use crate::auth::{new_id, now_iso};
use chrono::{Local, NaiveDate, NaiveTime, TimeZone};
use sqlx::{Row, SqliteConnection};

/// 把 "08:00,18:30" 解析为时刻列表，非法项忽略
fn parse_times(s: &str) -> Vec<(u32, u32)> {
    s.split(',')
        .filter_map(|t| {
            let t = t.trim();
            let mut it = t.split(':');
            let h: u32 = it.next()?.trim().parse().ok()?;
            let m: u32 = it.next()?.trim().parse().ok()?;
            if h <= 23 && m <= 59 {
                Some((h, m))
            } else {
                None
            }
        })
        .collect()
}

fn join_scheduled(date: NaiveDate, h: u32, m: u32) -> String {
    let nt = NaiveTime::from_hms_opt(h, m, 0).unwrap();
    Local
        .from_local_datetime(&date.and_time(nt))
        .single()
        .unwrap()
        .to_rfc3339()
}

/// 为宠物在寄养区间内补全全部喂食/给药任务。
/// 用唯一 slot + INSERT OR IGNORE 保证：重复调用不会产生重复任务。
#[allow(clippy::too_many_arguments)]
async fn insert_task(
    conn: &mut SqliteConnection,
    pet_id: &str,
    kind: &str,
    medication_id: Option<&str>,
    slot: &str,
    scheduled_at: &str,
) -> AppResult<bool> {
    let res = sqlx::query(
        "INSERT OR IGNORE INTO care_tasks
            (id, pet_id, kind, medication_id, slot, scheduled_at, created_at)
         VALUES (?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(new_id())
    .bind(pet_id)
    .bind(kind)
    .bind(medication_id)
    .bind(slot)
    .bind(scheduled_at)
    .bind(now_iso())
    .execute(conn)
    .await?;
    Ok(res.rows_affected() == 1)
}

/// 补全日历，返回新生成的任务数
pub async fn ensure_care_calendar(state: &AppState, pet_id: &str) -> AppResult<u64> {
    let row = sqlx::query(
        "SELECT boarding_start, boarding_end, feeding_times, status
         FROM pets WHERE id = ?",
    )
    .bind(pet_id)
    .fetch_optional(state.pool())
    .await?
    .ok_or_else(|| crate::app::AppError::not_found("宠物不存在"))?;

    let start_s: String = row.get("boarding_start");
    let end_s: String = row.get("boarding_end");
    let feeding_times_s: String = row.get("feeding_times");
    let status: String = row.get("status");

    let start = NaiveDate::parse_from_str(&start_s, "%Y-%m-%d")
        .map_err(|_| crate::app::AppError::bad_request("boarding_start 应为 YYYY-MM-DD"))?;
    let end = NaiveDate::parse_from_str(&end_s, "%Y-%m-%d")
        .map_err(|_| crate::app::AppError::bad_request("boarding_end 应为 YYYY-MM-DD"))?;

    let meds: Vec<(String, String)> = if status == "boarding" {
        sqlx::query("SELECT id, times FROM medications WHERE pet_id = ? AND active = 1")
            .bind(pet_id)
            .fetch_all(state.pool())
            .await?
            .into_iter()
            .map(|r| (r.get("id"), r.get("times")))
            .collect()
    } else {
        vec![]
    };

    let mut count = 0u64;
    let mut date = start;
    while date <= end {
        let ds = date.format("%Y-%m-%d").to_string();
        for (h, m) in parse_times(&feeding_times_s) {
            let slot = format!("{pet_id}:feeding:{ds}-{h:02}:{m:02}");
            let scheduled_at = join_scheduled(date, h, m);
            let mut tx = state.pool().begin().await?;
            if insert_task(&mut tx, pet_id, "feeding", None, &slot, &scheduled_at).await? {
                count += 1;
            }
            tx.commit().await?;
        }
        for (med_id, times_s) in &meds {
            for (h, m) in parse_times(times_s) {
                let slot = format!("{pet_id}:med:{med_id}:{ds}-{h:02}:{m:02}");
                let scheduled_at = join_scheduled(date, h, m);
                let mut tx = state.pool().begin().await?;
                if insert_task(&mut tx, pet_id, "medication", Some(med_id), &slot, &scheduled_at)
                    .await?
                {
                    count += 1;
                }
                tx.commit().await?;
            }
        }
        date = date.succ_opt().ok_or_else(|| {
            crate::app::AppError::bad_request("寄养日期超出可表示范围")
        })?;
    }
    Ok(count)
}
