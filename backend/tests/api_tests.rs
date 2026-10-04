mod common;

use common::TestApp;
use serde_json::{json, Value};

const DOUDOU: &str = "pet-doudou"; // 主人 lisi
const MIMI: &str = "pet-mimi"; // 主人 zhangsan
const ALICE: &str = "u-staff-alice";
const BOB: &str = "u-staff-bob";
const LISI: &str = "u-owner-li";

async fn err_code(resp: reqwest::Response) -> (u16, String) {
    let status = resp.status().as_u16();
    let v: Value = resp.json().await.unwrap_or(json!({}));
    (status, v["error"].as_str().unwrap_or("").to_string())
}

// ========== 规则 1：同一任务重复提交不得算作执行两次 ==========

#[tokio::test]
async fn duplicate_execution_is_counted_once() {
    let app = TestApp::spawn().await;
    let alice = app.login("alice", "staff123").await;

    let task = app
        .first_pending_task(&alice, DOUDOU, "feeding")
        .await;
    let task_id = task["id"].as_str().unwrap();

    // 第一次执行成功
    let resp = app
        .http
        .post(app.url(&format!("/api/tasks/{task_id}/execute")))
        .bearer_auth(&alice)
        .json(&json!({ "notes": "正常吃完 150g" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201, "首次执行应成功");

    // 第二次提交同一任务：必须被拒绝，而不是再记一条
    let resp = app
        .http
        .post(app.url(&format!("/api/tasks/{task_id}/execute")))
        .bearer_auth(&alice)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    let (status, code) = err_code(resp).await;
    assert_eq!(status, 409, "重复执行应返回 409");
    assert_eq!(code, "already_executed");

    // 第三次（模拟双击/网络重试）同样被拦截
    let resp = app
        .http
        .post(app.url(&format!("/api/tasks/{task_id}/execute")))
        .bearer_auth(&alice)
        .json(&json!({ "notes": "网络重试" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);

    // 任务列表中该任务只有一条执行记录，状态 done
    let resp = app
        .http
        .get(app.url("/api/tasks"))
        .bearer_auth(&alice)
        .query(&[("pet_id", DOUDOU), ("kind", "feeding")])
        .send()
        .await
        .unwrap();
    let v: Value = resp.json().await.unwrap();
    let execs: Vec<&Value> = v["tasks"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|t| t["id"] == task_id && t["execution"].is_object())
        .collect();
    assert_eq!(execs.len(), 1, "同一任务只能有一条执行记录");
    assert_eq!(execs[0]["status"], "done");
    assert_eq!(execs[0]["execution"]["performer_name"], "王护工");
}

// ========== 规则 2：给药必须由另一名员工核对药名及剂量 ==========

#[tokio::test]
async fn medication_requires_distinct_staff_verification() {
    let app = TestApp::spawn().await;
    let alice = app.login("alice", "staff123").await;

    let task = app
        .first_pending_task(&alice, DOUDOU, "medication")
        .await;
    let task_id = task["id"].as_str().unwrap().to_string();
    assert_eq!(task["drug_name"], "阿莫西林");
    assert_eq!(task["dosage"], "50mg");

    let execute = |body: Value| {
        let tid = task_id.clone();
        let token = alice.clone();
        let http = app.http.clone();
        let addr = app.addr.clone();
        async move {
            http.post(format!("{addr}/api/tasks/{tid}/execute"))
                .bearer_auth(token)
                .json(&body)
                .send()
                .await
                .unwrap()
        }
    };

    // 2.1 完全没有核对人 → 拒绝
    let (s, c) = err_code(execute(json!({})).await).await;
    assert_eq!(s, 422);
    assert_eq!(c, "verifier_required");

    // 2.2 核对人是执行本人 → 拒绝
    let (s, c) = err_code(
        execute(json!({ "verifier_id": ALICE, "verify_drug_name": "阿莫西林", "verify_dosage": "50mg" }))
            .await,
    )
    .await;
    assert_eq!(s, 422);
    assert_eq!(c, "verifier_must_differ");

    // 2.3 核对人是主人（非员工）→ 拒绝
    let (s, c) = err_code(
        execute(json!({ "verifier_id": LISI, "verify_drug_name": "阿莫西林", "verify_dosage": "50mg" }))
            .await,
    )
    .await;
    assert_eq!(s, 422);
    assert_eq!(c, "verifier_must_be_staff");

    // 2.4 核对人是另一名员工，但药名/剂量对不上 → 拒绝
    let (s, c) = err_code(
        execute(json!({ "verifier_id": BOB, "verify_drug_name": "头孢", "verify_dosage": "50mg" }))
            .await,
    )
    .await;
    assert_eq!(s, 422);
    assert_eq!(c, "verify_mismatch");
    let (s, c) = err_code(
        execute(json!({ "verifier_id": BOB, "verify_drug_name": "阿莫西林", "verify_dosage": "100mg" }))
            .await,
    )
    .await;
    assert_eq!(s, 422);
    assert_eq!(c, "verify_mismatch");

    // 2.5 药名剂量留空 → 拒绝
    let (s, c) = err_code(execute(json!({ "verifier_id": BOB })).await).await;
    assert_eq!(s, 422);
    assert_eq!(c, "verify_fields_required");

    // 2.6 另一名员工核对且药名剂量一致 → 成功
    let resp = execute(json!({
        "verifier_id": BOB,
        "verify_drug_name": "阿莫西林",
        "verify_dosage": "50mg",
        "notes": "餐后给药"
    }))
    .await;
    assert_eq!(resp.status(), 201, "双人核对通过后应成功");

    // 执行详情记录了执行人 + 核对人两个不同员工
    let resp = app
        .http
        .get(app.url(&format!("/api/tasks/{task_id}")))
        .bearer_auth(&alice)
        .send()
        .await
        .unwrap();
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["task"]["status"], "done");
    assert_eq!(v["execution"]["performer_name"], "王护工");
    assert_eq!(v["execution"]["verifier_name"], "陈护工");
    assert_ne!(
        v["execution"]["performer_id"],
        v["execution"]["verifier_id"],
        "执行人与核对人必须不同"
    );

    // 2.7 普通喂食任务不需要核对人（对照）
    let feeding = app
        .first_pending_task(&alice, DOUDOU, "feeding")
        .await;
    let fid = feeding["id"].as_str().unwrap();
    let resp = app
        .http
        .post(app.url(&format!("/api/tasks/{fid}/execute")))
        .bearer_auth(&alice)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
}

// ========== 规则 3：代接授权撤销后，旧取宠码立即失效被拦截 ==========

#[tokio::test]
async fn revoked_pickup_code_is_rejected() {
    let app = TestApp::spawn().await;
    let lisi = app.login("lisi", "owner123").await;
    let alice = app.login("alice", "staff123").await;

    // 主人签发代接授权，拿到明文取宠码
    let resp = app
        .http
        .post(app.url(&format!("/api/pets/{DOUDOU}/pickup-authorizations")))
        .bearer_auth(&lisi)
        .json(&json!({ "proxy_name": "王阿姨", "proxy_contact": "13800000000" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);
    let v: Value = resp.json().await.unwrap();
    let auth_id = v["id"].as_str().unwrap().to_string();
    let code = v["pickup_code"].as_str().unwrap().to_string();
    assert_eq!(code.len(), 8, "取宠码应为 8 位");

    // 撤销前，码可以通过查询找到授权（列表为 active）
    let resp = app
        .http
        .get(app.url(&format!("/api/pets/{DOUDOU}/pickup-authorizations")))
        .bearer_auth(&lisi)
        .send()
        .await
        .unwrap();
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["authorizations"][0]["status"], "active");

    // 主人撤销授权
    let resp = app
        .http
        .post(app.url(&format!("/api/pickup-authorizations/{auth_id}/revoke")))
        .bearer_auth(&lisi)
        .json(&json!({ "reason": "代接人临时有事" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 员工用旧码领取：码本身正确，但必须被拦截
    let resp = app
        .http
        .post(app.url("/api/pickup/redeem"))
        .bearer_auth(&alice)
        .json(&json!({ "code": code }))
        .send()
        .await
        .unwrap();
    let (s, c) = err_code(resp).await;
    assert_eq!(s, 409, "撤销后的旧码必须被拦截");
    assert_eq!(c, "pickup_code_revoked");

    // 宠物仍在寄养中（没被领走）
    let resp = app
        .http
        .get(app.url(&format!("/api/pets/{DOUDOU}/timeline")))
        .bearer_auth(&alice)
        .send()
        .await
        .unwrap();
    let v: Value = resp.json().await.unwrap();
    // 任务没有因为核销而取消
    assert!(
        v["summary"]["cancelled"].as_i64().unwrap() == 0,
        "拦截后不应取消任何任务"
    );
}

#[tokio::test]
async fn valid_pickup_code_redeems_once() {
    let app = TestApp::spawn().await;
    let zhangsan = app.login("zhangsan", "owner123").await;
    let alice = app.login("alice", "staff123").await;

    let resp = app
        .http
        .post(app.url(&format!("/api/pets/{MIMI}/pickup-authorizations")))
        .bearer_auth(&zhangsan)
        .json(&json!({ "proxy_name": "邻居小赵" }))
        .send()
        .await
        .unwrap();
    let v: Value = resp.json().await.unwrap();
    let code = v["pickup_code"].as_str().unwrap().to_string();

    // 错误的码 → 404
    let resp = app
        .http
        .post(app.url("/api/pickup/redeem"))
        .bearer_auth(&alice)
        .json(&json!({ "code": "00000000" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // 正确码核销成功
    let resp = app
        .http
        .post(app.url("/api/pickup/redeem"))
        .bearer_auth(&alice)
        .json(&json!({ "code": code }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["pet_name"], "咪咪");
    assert!(v["cancelled_tasks"].as_i64().unwrap() > 0, "接走后应取消后续任务");

    // 同一码不能重复领取
    let resp = app
        .http
        .post(app.url("/api/pickup/redeem"))
        .bearer_auth(&alice)
        .json(&json!({ "code": code }))
        .send()
        .await
        .unwrap();
    let (s, c) = err_code(resp).await;
    assert_eq!(s, 409);
    assert_eq!(c, "pickup_code_used");

    // 主人收到了接走通知
    let resp = app
        .http
        .get(app.url("/api/notifications"))
        .bearer_auth(&zhangsan)
        .send()
        .await
        .unwrap();
    let v: Value = resp.json().await.unwrap();
    assert!(
        v["notifications"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["kind"] == "pickup_redeemed"),
        "应通知主人宠物已被代接"
    );
}

// ========== 规则 4：主人只读，不能看他人宠物，不能改门店填写内容 ==========

#[tokio::test]
async fn owner_is_read_only_and_scoped() {
    let app = TestApp::spawn().await;
    let lisi = app.login("lisi", "owner123").await;
    let alice = app.login("alice", "staff123").await;

    // 主人可以查看自己宠物的时间线（只读）
    let resp = app
        .http
        .get(app.url(&format!("/api/pets/{DOUDOU}/timeline")))
        .bearer_auth(&lisi)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);

    // 修改卡片 → 403
    let resp = app
        .http
        .patch(app.url(&format!("/api/pets/{DOUDOU}")))
        .bearer_auth(&lisi)
        .json(&json!({ "food_portion": "999kg/餐" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);

    // 执行任务 → 403
    let task = app.first_pending_task(&alice, DOUDOU, "feeding").await;
    let tid = task["id"].as_str().unwrap();
    let resp = app
        .http
        .post(app.url(&format!("/api/tasks/{tid}/execute")))
        .bearer_auth(&lisi)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);

    // 上报异常 → 403
    let resp = app
        .http
        .post(app.url(&format!("/api/tasks/{tid}/anomaly")))
        .bearer_auth(&lisi)
        .json(&json!({
            "incident_type": "refused_food",
            "description": "x", "handling": "y"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);

    // 核销取宠码 → 403（员工操作）
    let resp = app
        .http
        .post(app.url("/api/pickup/redeem"))
        .bearer_auth(&lisi)
        .json(&json!({ "code": "12345678" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 403);

    // 不能看别人的宠物（按不存在处理，避免泄露）
    let resp = app
        .http
        .get(app.url(&format!("/api/pets/{MIMI}/timeline")))
        .bearer_auth(&lisi)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 404);

    // 任务列表里也看不到别人的宠物
    let resp = app
        .http
        .get(app.url("/api/tasks"))
        .bearer_auth(&lisi)
        .send()
        .await
        .unwrap();
    let v: Value = resp.json().await.unwrap();
    assert!(
        v["tasks"]
            .as_array()
            .unwrap()
            .iter()
            .all(|t| t["pet_id"] == DOUDOU),
        "主人只能看到自己宠物的任务"
    );

    // 未登录 → 401
    let resp = app
        .http
        .get(app.url("/api/tasks"))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 401);
}

// ========== 规则 5：提前接走取消后续任务；异常必须单独记录经过 ==========

#[tokio::test]
async fn early_checkout_cancels_future_tasks() {
    let app = TestApp::spawn().await;
    let alice = app.login("alice", "staff123").await;
    let lisi = app.login("lisi", "owner123").await;

    // 先完成一个喂食，留一条历史
    let task = app.first_pending_task(&alice, DOUDOU, "feeding").await;
    let done_id = task["id"].as_str().unwrap().to_string();
    let pending_task = app
        .first_pending_task(&alice, DOUDOU, "medication")
        .await;
    let pending_id = pending_task["id"].as_str().unwrap().to_string();

    app.http
        .post(app.url(&format!("/api/tasks/{done_id}/execute")))
        .bearer_auth(&alice)
        .json(&json!({}))
        .send()
        .await
        .unwrap();

    let before = app.timeline(&alice, DOUDOU).await;
    let pending_before = before["summary"]["pending"].as_i64().unwrap();
    assert!(pending_before > 0);

    // 提前接走
    let resp = app
        .http
        .post(app.url(&format!("/api/pets/{DOUDOU}/checkout")))
        .bearer_auth(&alice)
        .json(&json!({ "note": "主人提前 2 天接走" }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let v: Value = resp.json().await.unwrap();
    assert_eq!(v["cancelled_tasks"], pending_before);

    let after = app.timeline(&alice, DOUDOU).await;
    assert_eq!(after["summary"]["pending"], 0);
    assert_eq!(after["summary"]["cancelled"], pending_before);
    // 已完成记录保留
    assert!(after["summary"]["done"].as_i64().unwrap() >= 1);

    // 被取消的任务不能再执行
    let resp = app
        .http
        .post(app.url(&format!("/api/tasks/{pending_id}/execute")))
        .bearer_auth(&alice)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    let (s, c) = err_code(resp).await;
    assert_eq!(s, 409);
    assert_eq!(c, "task_cancelled");

    // 重复接走被拦截
    let resp = app
        .http
        .post(app.url(&format!("/api/pets/{DOUDOU}/checkout")))
        .bearer_auth(&alice)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 409);

    // 主人收到接走通知
    let resp = app
        .http
        .get(app.url("/api/notifications"))
        .bearer_auth(&lisi)
        .send()
        .await
        .unwrap();
    let v: Value = resp.json().await.unwrap();
    assert!(v["notifications"]
        .as_array()
        .unwrap()
        .iter()
        .any(|n| n["kind"] == "checkout"));
}

#[tokio::test]
async fn anomaly_requires_handling_and_notifies_owner() {
    let app = TestApp::spawn().await;
    let alice = app.login("alice", "staff123").await;
    let lisi = app.login("lisi", "owner123").await;

    let task = app.first_pending_task(&alice, DOUDOU, "feeding").await;
    let tid = task["id"].as_str().unwrap().to_string();

    // 只点“完成”但不写处理经过 → 拒绝（异常不能直接勾完成）
    let resp = app
        .http
        .post(app.url(&format!("/api/tasks/{tid}/anomaly")))
        .bearer_auth(&alice)
        .json(&json!({
            "incident_type": "refused_food",
            "description": "  ",
            "handling": ""
        }))
        .send()
        .await
        .unwrap();
    let (s, c) = err_code(resp).await;
    assert_eq!(s, 422);
    assert_eq!(c, "handling_required");

    // 完整记录情况与处理经过 → 成功
    let resp = app
        .http
        .post(app.url(&format!("/api/tasks/{tid}/anomaly")))
        .bearer_auth(&alice)
        .json(&json!({
            "incident_type": "refused_food",
            "description": "早餐闻了一下就走开，完全没吃",
            "handling": "30 分钟后少量多次投喂，吃下约 80g；持续观察并通知主人"
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), 201);

    // 任务状态是 anomaly 而非 done，且不能再被直接勾完成
    let resp = app
        .http
        .post(app.url(&format!("/api/tasks/{tid}/execute")))
        .bearer_auth(&alice)
        .json(&json!({}))
        .send()
        .await
        .unwrap();
    let (s, c) = err_code(resp).await;
    assert_eq!(s, 409);
    assert_eq!(c, "anomaly_open");

    // 时间线里有独立异常记录
    let tl = app.timeline(&alice, DOUDOU).await;
    assert_eq!(tl["incidents"][0]["incident_type"], "refused_food");
    assert!(tl["incidents"][0]["handling"].as_str().unwrap().contains("80g"));

    // 主人收到拒食通知
    let resp = app
        .http
        .get(app.url("/api/notifications"))
        .bearer_auth(&lisi)
        .send()
        .await
        .unwrap();
    let v: Value = resp.json().await.unwrap();
    let note = v["notifications"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["kind"] == "refused_food")
        .expect("应通知主人拒食");
    assert!(note["body"].as_str().unwrap().contains("80g"));
}
