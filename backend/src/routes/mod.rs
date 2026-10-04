pub mod auth_routes;
pub mod common;
pub mod notification_routes;
pub mod pet_routes;
pub mod pickup_routes;
pub mod task_routes;

use axum::routing::{delete, get, post};
use axum::Router;

pub fn router() -> Router<crate::app::AppState> {
    Router::new()
        // 认证
        .route("/auth/login", post(auth_routes::login))
        .route("/auth/me", get(auth_routes::me))
        .route("/auth/logout", post(auth_routes::logout))
        // 名录（员工建卡片选主人 / 给药核对选员工）
        .route("/owners", get(pet_routes::list_owners))
        .route("/staff", get(pet_routes::list_staff))
        // 宠物卡片与用药
        .route("/pets", get(pet_routes::list_pets).post(pet_routes::create_pet))
        .route(
            "/pets/:id",
            get(pet_routes::pet_detail).patch(pet_routes::update_pet),
        )
        .route(
            "/pets/:id/medications",
            post(pet_routes::add_medication),
        )
        .route(
            "/pets/:id/medications/:med_id",
            delete(pet_routes::remove_medication),
        )
        // 照护时间线 / 异常
        .route("/pets/:id/timeline", get(pet_routes::pet_timeline))
        .route(
            "/pets/:id/incidents",
            get(pet_routes::list_incidents).post(pet_routes::create_incident),
        )
        // 主人提前接走
        .route("/pets/:id/checkout", post(pet_routes::checkout))
        // 任务看板与执行
        .route("/tasks", get(task_routes::list_tasks))
        .route("/tasks/:id", get(task_routes::task_detail))
        .route("/tasks/:id/execute", post(task_routes::execute_task))
        .route("/tasks/:id/anomaly", post(task_routes::report_anomaly))
        // 照片上传（本地保存）
        .route("/uploads", post(common::upload_photo))
        // 代接授权 / 取宠码
        .route(
            "/pets/:id/pickup-authorizations",
            get(pickup_routes::list_authorizations).post(pickup_routes::create_authorization),
        )
        .route(
            "/pickup-authorizations/:id/revoke",
            post(pickup_routes::revoke_authorization),
        )
        .route("/pickup/redeem", post(pickup_routes::redeem_code))
        // 通知（本地模拟）
        .route("/notifications", get(notification_routes::list_notifications))
        .route("/notifications/read-all", post(notification_routes::read_all))
        .route("/notifications/:id/read", post(notification_routes::mark_read))
}
