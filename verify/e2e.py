#!/usr/bin/env python3
"""端到端验证：重复执行幂等 / 给药双人核对 / 撤销代接旧码拦截，以及越权、异常通知等。"""
import json
import os
import sqlite3
import sys
import urllib.request
import urllib.error
import uuid

PORT = os.environ["VERIFY_PORT"]
DB = os.environ["VERIFY_DB"]
UPLOADS = os.environ["VERIFY_UPLOADS"]
BASE = f"http://127.0.0.1:{PORT}"

PASS, FAIL = 0, 0
FAILURES = []


def check(name, cond, detail=""):
    global PASS, FAIL
    if cond:
        PASS += 1
        print(f"  ✅ {name}")
    else:
        FAIL += 1
        FAILURES.append(name)
        print(f"  ❌ {name} {detail}")


def call(method, path, token=None, body=None):
    url = BASE + path
    data = None
    headers = {}
    if body is not None:
        data = json.dumps(body).encode()
        headers["Content-Type"] = "application/json"
    if token:
        headers["Authorization"] = f"Bearer {token}"
    req = urllib.request.Request(url, data=data, headers=headers, method=method)
    try:
        with urllib.request.urlopen(req) as r:
            return r.status, json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"{}")


def login(u, p):
    s, r = call("POST", "/api/auth/login", body={"username": u, "password": p})
    assert s == 200, f"登录失败 {u}: {r}"
    return r["token"]


def upload(path, token, fields=None, file_field=None):
    """multipart/form-data 上传（含文件）"""
    boundary = "----b" + uuid.uuid4().hex
    lines = []
    for k, v in (fields or {}).items():
        lines.append(f"--{boundary}\r\nContent-Disposition: form-data; name=\"{k}\"\r\n\r\n{v}\r\n")
    if file_field:
        fname, content, ctype = file_field
        lines.append(
            f"--{boundary}\r\nContent-Disposition: form-data; name=\"photo\"; filename=\"{fname}\"\r\n"
            f"Content-Type: {ctype}\r\n\r\n"
        )
    body = "".join(lines).encode() + (file_field[1] if file_field else b"") + f"\r\n--{boundary}--\r\n".encode()
    req = urllib.request.Request(
        BASE + path, data=body, method="POST",
        headers={"Authorization": f"Bearer {token}", "Content-Type": f"multipart/form-data; boundary={boundary}"},
    )
    try:
        with urllib.request.urlopen(req) as r:
            return r.status, json.loads(r.read() or b"{}")
    except urllib.error.HTTPError as e:
        return e.code, json.loads(e.read() or b"{}")


def db_one(sql, args=()):
    con = sqlite3.connect(DB)
    try:
        return con.execute(sql, args).fetchone()
    finally:
        con.close()


def main():
    alice = login("alice", "staff123")
    bob = login("bob", "staff123")
    wang = login("wang", "owner123")
    li = login("li", "owner123")

    # 找咪咪（王女士）及其任务/药品
    s, pets = call("GET", "/api/pets", wang)
    mimi = next(p for p in pets if p["name"] == "咪咪")
    pet_id = mimi["id"]
    s, detail = call("GET", f"/api/pets/{pet_id}", wang)
    med_id = detail["medications"][0]["id"]

    s, r = call("GET", f"/api/tasks?pet_id={pet_id}", alice)
    tasks = r["tasks"]
    feed_task = next(t for t in tasks if t["kind"] == "feed" and t["status"] == "scheduled")
    med_task = next(t for t in tasks if t["kind"] == "medication" and t["status"] == "scheduled")
    feed_id, med_task_id = feed_task["id"], med_task["id"]

    print("\n[前置] 主人越权隔离")
    # 李先生（豆豆主人）访问咪咪 → 403
    s, r = call("GET", f"/api/pets/{pet_id}", li)
    check("主人不能查看他人宠物（403）", s == 403, f"got {s} {r}")
    s, r = call("GET", f"/api/tasks?pet_id={pet_id}", li)
    check("主人不能查看他人宠物任务（403）", s == 403, f"got {s} {r}")
    s, r = call("POST", f"/api/tasks/{feed_id}/execute", wang, body={})
    check("主人不能执行门店任务（403）", s == 403, f"got {s}")
    s, r = call("POST", "/api/incidents", wang,
                body={"pet_id": pet_id, "incident_type": "refused_food",
                      "description": "x", "handling": "y"})
    check("主人不能填写异常记录（403）", s == 403, f"got {s}")

    print("\n[用例1] 同一任务重复提交不得算作执行两次（幂等）")
    # 第一次执行（喂食，附照片，验证照片本地保存）
    fake_png = b"\x89PNG\r\n\x1a\n" + b"0" * 64
    s, r = upload(f"/api/tasks/{feed_id}/execute-photo", alice,
                  fields={"note": "光盘，正常吃完"},
                  file_field=("meal.png", fake_png, "image/png"))
    check("首次执行成功（200）", s == 200, f"got {s} {r}")
    photo = r.get("photo_path", "")
    check("返回照片路径", photo.startswith("/uploads/"), str(r))
    check("照片已保存到本地上传目录", os.path.exists(os.path.join(UPLOADS, os.path.basename(photo))))

    # 立即重复提交（同一员工、同样参数）
    s, r = call("POST", f"/api/tasks/{feed_id}/execute", alice, body={"note": "再喂一次？"})
    check("重复提交被拒绝（409）", s == 409, f"got {s} {r}")
    # 换员工、换渠道（multipart）重复提交同样拦截
    s, r = upload(f"/api/tasks/{feed_id}/execute-photo", bob,
                  fields={"note": "换我再试"}, file_field=("a.png", fake_png, "image/png"))
    check("换员工重复提交仍被拒绝（409）", s == 409, f"got {s} {r}")

    row = db_one("SELECT COUNT(*), executed_by FROM task_executions WHERE task_id=?", (feed_id,))
    check("数据库中只有 1 条执行记录", row[0] == 1, f"实际 {row[0]} 条")
    check("执行者仍是首次提交人", row[1] == 1, f"executed_by={row[1]}")
    row = db_one("SELECT status FROM care_tasks WHERE id=?", (feed_id,))
    check("任务状态为 done", row[0] == "done")

    print("\n[用例2] 给药双人核对：必须另一名员工核对药名及剂量")
    # 2.1 不填核对人
    s, r = call("POST", f"/api/tasks/{med_task_id}/execute", alice, body={"note": ""})
    check("无核对人 → 400", s == 400, f"got {s} {r}")
    # 2.2 执行本人核对（alice 用自己的凭证）
    s, r = call("POST", f"/api/tasks/{med_task_id}/execute", alice,
                body={"verifier_username": "alice", "verifier_password": "staff123"})
    check("执行本人核对 → 400 拒绝", s == 400, f"got {s} {r}")
    # 2.3 核对人密码错误
    s, r = call("POST", f"/api/tasks/{med_task_id}/execute", alice,
                body={"verifier_username": "bob", "verifier_password": "wrong-pass"})
    check("核对人密码错误 → 400", s == 400, f"got {s} {r}")
    # 2.4 主人不能充当核对人
    s, r = call("POST", f"/api/tasks/{med_task_id}/execute", alice,
                body={"verifier_username": "wang", "verifier_password": "owner123"})
    check("非员工（主人）核对 → 400", s == 400, f"got {s} {r}")
    # 以上失败均不应产生执行记录
    row = db_one("SELECT COUNT(*) FROM task_executions WHERE task_id=?", (med_task_id,))
    check("核对未通过前不产生执行记录", row[0] == 0, f"{row[0]} 条")
    # 2.5 另一名员工 bob 核对通过
    s, r = call("POST", f"/api/tasks/{med_task_id}/execute", alice,
                body={"note": "随早餐喂下",
                      "verifier_username": "bob", "verifier_password": "staff123"})
    check("另一名员工核对通过 → 200", s == 200, f"got {s} {r}")
    check("响应带核对药名", r.get("verified_med_name") == "阿莫西林", str(r))
    check("响应带核对剂量", "5mg" in (r.get("verified_dosage") or ""), str(r))
    row = db_one(
        "SELECT executed_by, verifier_id, verifier_med_name, verifier_dosage, verified_at "
        "FROM task_executions WHERE task_id=?", (med_task_id,))
    check("执行人为 alice(id=1)", row[0] == 1, str(row))
    check("核对人为另一名员工 bob(id=2)", row[1] == 2, str(row))
    check("留痕药名", row[2] == "阿莫西林")
    check("留痕剂量", "5mg" in (row[3] or ""))
    check("核对时间已记录", bool(row[4]))
    # 给药任务同样幂等
    s, r = call("POST", f"/api/tasks/{med_task_id}/execute", alice,
                body={"verifier_username": "bob", "verifier_password": "staff123"})
    check("给药重复提交同样 409", s == 409, f"got {s} {r}")

    print("\n[用例3] 拒食/漏服/换粮：必须写处理经过并通知主人，不能直接勾选")
    # 先安排一个新喂食任务用于拒食
    s, r = call("POST", "/api/tasks", alice,
                body={"pet_id": pet_id, "kind": "feed",
                      "scheduled_at": "2099-01-01 09:00", "food_brand": "皇家成猫粮",
                      "portion": "40g", "note": "加餐"})
    future_feed = r["id"]
    # 处理经过为空 → 拒绝
    s, r = call("POST", "/api/incidents", alice,
                body={"pet_id": pet_id, "task_id": future_feed,
                      "incident_type": "refused_food", "description": "一口没吃", "handling": ""})
    check("不写处理经过 → 400", s == 400, f"got {s} {r}")
    # 正式上报拒食
    s, r = call("POST", "/api/incidents", alice,
                body={"pet_id": pet_id, "task_id": future_feed,
                      "incident_type": "refused_food",
                      "description": "晚餐闻了一下就走开，完全拒食",
                      "handling": "30 分钟后少量多餐补喂，进食 20g；已观察精神状态正常并告知值班经理"})
    check("上报成功并触发通知", s == 200 and r.get("notified") is True, f"got {s} {r}")
    inc_id = r["incident_id"]
    row = db_one("SELECT incident_type, handling FROM incidents WHERE id=?", (inc_id,))
    check("异常台账有类型与处理经过", row[0] == "refused_food" and bool(row[1]))
    row = db_one(
        "SELECT title, body, is_read FROM notifications WHERE incident_id=?", (inc_id,))
    check("已给主人写通知（标题含拒食）", row is not None and "拒食" in row[0], str(row))
    check("通知内容包含处理经过", "少量多餐" in (row[1] if row else ""))
    # 主人可见该通知
    s, notes = call("GET", "/api/notifications", wang)
    check("主人端可看到通知", any(n["incident_id"] == inc_id for n in notes))

    print("\n[用例4] 撤销代接授权后旧取宠码立即失效，不能继续用旧码领取")
    # 主人为代接人创建取宠码
    s, r = call("POST", "/api/pickup-codes", wang,
                body={"pet_id": pet_id, "authorized_name": "张阿姨"})
    check("创建代接取宠码成功", s == 200 and len(r.get("code", "")) == 8, f"got {s} {r}")
    code = r["code"]
    code_id = r["id"]
    # 主人不能直接自己核销（核销是门店动作）—— 再次确保权限
    s, r = call("POST", "/api/pickup/redeem", wang, body={"code": code})
    check("主人不能自助核销（403）", s == 403, f"got {s}")
    # 撤销前，员工可用码核销吗？——先不核销，验证撤销拦截；先撤销
    s, r = call("POST", f"/api/pickup-codes/{code_id}/revoke", wang)
    check("主人撤销授权成功", s == 200, f"got {s} {r}")
    # 关键断言：旧码立即失效
    s, r = call("POST", "/api/pickup/redeem", alice, body={"code": code})
    check("旧取宠码核销被拦截（403）", s == 403, f"got {s} {r}")
    check("拦截信息明确提示已撤销", "撤销" in r.get("message", ""), str(r))
    # 伪造/不存在码也拦截
    s, r = call("POST", "/api/pickup/redeem", alice, body={"code": "00000000"})
    check("不存在的取宠码被拦截（403）", s == 403, f"got {s} {r}")
    row = db_one("SELECT status FROM pickup_codes WHERE id=?", (code_id,))
    check("数据库中授权状态为 revoked", row[0] == "revoked", str(row))
    # 重复撤销 → 冲突
    s, r = call("POST", f"/api/pickup-codes/{code_id}/revoke", wang)
    check("重复撤销返回 409", s == 409, f"got {s} {r}")
    # 宠物仍在店（旧码没有放宠）
    row = db_one("SELECT status FROM pets WHERE id=?", (pet_id,))
    check("旧码被拦截，宠物仍为寄养中", row[0] == "boarding", str(row))

    print("\n[用例5] 有效新码核销成功且一次性；接走后后续任务取消")
    # 新授权一张码
    s, r = call("POST", "/api/pickup-codes", wang,
                body={"pet_id": pet_id, "authorized_name": "李叔叔"})
    code2 = r["code"]
    s, r = call("POST", "/api/pickup/redeem", bob, body={"code": code2})
    check("有效新码核销放宠成功", s == 200, f"got {s} {r}")
    check("放宠对象与宠物信息正确", r.get("authorized_name") == "李叔叔" and r.get("pet_id") == pet_id, str(r))
    # 码不可重复使用
    s, r = call("POST", "/api/pickup/redeem", bob, body={"code": code2})
    check("取宠码不可重复使用（409）", s == 409, f"got {s} {r}")
    # 后续任务被取消（2099 加餐任务）
    row = db_one("SELECT status FROM care_tasks WHERE id=?", (future_feed,))
    check("接走后后续待执行任务已取消", row[0] == "cancelled", str(row))
    row = db_one("SELECT status, check_out_at FROM pets WHERE id=?", (pet_id,))
    check("宠物状态变为已接走", row[0] == "picked_up" and bool(row[1]))

    print("\n[用例6] 提前接走：取消后续任务（独立路径，用豆豆验证）")
    s, r = call("GET", "/api/pets", li)
    doudou = next(p for p in r if p["name"] == "豆豆")
    s, r = call("POST", "/api/tasks", alice,
                body={"pet_id": doudou["id"], "kind": "feed",
                      "scheduled_at": "2099-06-01 08:00", "food_brand": "比瑞吉幼犬粮",
                      "portion": "80g", "note": "未来餐"})
    dd_future = r["id"]
    s, r = call("POST", f"/api/pets/{doudou['id']}/checkout", alice)
    check("提前接走办理成功", s == 200 and r.get("cancelled_tasks", 0) >= 1, f"got {s} {r}")
    row = db_one("SELECT status FROM care_tasks WHERE id=?", (dd_future,))
    check("豆豆的后续任务已取消", row[0] == "cancelled", str(row))
    # 已完成任务不被取消
    row = db_one("SELECT status FROM care_tasks WHERE id=?", (feed_id,))
    check("历史已完成记录保持 done", row[0] == "done")

    print(f"\n========== 结果：{PASS} 通过，{FAIL} 失败 ==========")
    if FAIL:
        print("失败项：" + "；".join(FAILURES))
        sys.exit(1)
    print("🎉 全部验证通过：重复执行幂等、给药双人核对、撤销后旧码拦截均符合要求")


if __name__ == "__main__":
    main()
