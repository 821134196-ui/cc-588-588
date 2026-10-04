# 家庭宠物寄养喂食与用药交接系统

寄养门店按主人的照护要求安排每日喂食与给药任务，员工记录实际执行情况，主人随时查看宠物是否按时吃饭、用药。

- **后端**：Rust + Axum + SQLx + SQLite
- **前端**：Svelte 4 + Vite
- **通知**：本地模拟（入库 + 服务端日志 + 主人端消息中心）
- **照片**：保存在本地 `backend/data/uploads/`，通过 `/uploads/*` 访问

## 一条命令本地启动

```bash
./run.sh
```

脚本会自动安装 Rust（如缺失）、`npm install`、构建前端，然后启动后端并托管前端。
打开 http://localhost:8080 即可使用（首次启动自动建库、建表并写入演示数据）。

开发模式（Vite 热更新 :5173 + 后端 :8080）：`./run.sh dev`

## 演示账号

| 角色 | 用户名 | 密码 | 说明 |
| --- | --- | --- | --- |
| 员工 | `alice` | `staff123` | 王护工 |
| 员工 | `bob` | `staff123` | 陈护工（给药第二核对人） |
| 主人 | `lisi` | `owner123` | 金毛「豆豆」的主人 |
| 主人 | `zhangsan` | `owner123` | 英短「咪咪」的主人 |

## 功能与业务规则

### 宠物卡片与照护日历
- 卡片写明**每餐份量、主粮、禁忌、喂食时刻、备注**；用药写明**药名、剂量、服用时刻**。
- 建卡/改卡后系统按寄养日期 × 喂食时刻 × 用药时刻自动生成照护日历；
  任务用唯一槽位（`宠物:类型:药:日期-时刻`）`INSERT OR IGNORE`，重复生成不会产生重复任务。

### 执行记录
- 员工在看板执行喂食；记录**实际执行时间、备注、照片**。
- **给药必须由另一名员工核对**：核对人不能是执行人本人、必须是员工身份，
  且要确认药名与剂量与处方完全一致，任一不符均拒绝（422）。
- **同一任务重复提交不算执行两次**：`executions.task_id` 唯一约束 + 业务层 409 双重保证，
  双击、网络重试都只会保留第一条执行记录。

### 异常处理（拒食 / 漏服 / 临时换粮）
- 异常不能直接勾选完成，必须单独填写**异常情况**和**处理经过**（否则 422 `handling_required`）。
- 任务状态置为 `anomaly`（不是 `done`），独立留档，并向主人发本地模拟通知。

### 主人只读
- 主人只能查看**自己**宠物的卡片、照护日历、异常与通知；访问他人宠物返回 404。
- 所有写接口（改卡、执行、异常、核销等）对主人返回 403，门店已填内容不可修改。

### 提前接走与代接取宠码
- 员工办理提前接走（或代接人凭码核销）后，**所有未执行任务立即取消**，历史完成记录保留。
- 主人可签发 8 位取宠码给代接人（明文码仅创建时返回一次，库内只存哈希）。
- **撤销代接授权后旧码立即失效**：码虽正确，核销仍被 409 `pickup_code_revoked` 拦截，宠物不会被领走。
- 码使用一次后不可重复领取；宠物接走后其余有效授权一并失效。

## 自动化验证

```bash
cd backend && cargo test
```

7 个集成测试全部通过，关键用例：

| 测试 | 验证内容 |
| --- | --- |
| `duplicate_execution_is_counted_once` | 同一任务二次/三次提交均 409，执行记录只有一条 |
| `medication_requires_distinct_staff_verification` | 无核对人/自核/主人核/药名剂量不符均 422，另一员工核对一致才 201 |
| `revoked_pickup_code_is_rejected` | 撤销后旧码核销 409 被拦截，宠物仍在寄养、任务不取消 |
| `valid_pickup_code_redeems_once` | 有效码核销成功并取消后续任务、通知主人；重复使用 409 |
| `owner_is_read_only_and_scoped` | 主人写操作 403、看他人宠物 404、任务列表只含自己宠物 |
| `early_checkout_cancels_future_tasks` | 接走取消全部待执行任务、保留已完成记录、取消的任务不可再执行 |
| `anomaly_requires_handling_and_notifies_owner` | 异常无处理经过 422；登记后任务为 anomaly 且通知主人 |

## 主要接口

```
POST /api/auth/login                 登录
GET  /api/pets                       宠物列表（主人自动限定自己的）
POST /api/pets                       员工建卡（自动生成日历）
PATCH /api/pets/:id                  员工改卡
GET  /api/pets/:id                   卡片 + 用药
POST /api/pets/:id/medications       添加用药并生成给药任务
DELETE /api/pets/:id/medications/:m  停用药物（取消其待执行任务）
GET  /api/pets/:id/timeline          照护时间线 + 异常 + 汇总
POST /api/pets/:id/incidents         登记异常（情况 + 处理经过）
POST /api/pets/:id/checkout          提前接走（取消后续任务）
GET  /api/tasks                      看板（可按宠物/类型/状态/日期过滤）
POST /api/tasks/:id/execute          执行（给药需 verifier_id + 药名剂量核对）
POST /api/tasks/:id/anomaly          任务维度异常上报
POST /api/uploads                    照片上传（multipart，本地保存）
POST /api/pets/:id/pickup-authorizations  主人签发代接码
POST /api/pickup-authorizations/:id/revoke 主人撤销（旧码立即失效）
POST /api/pickup/redeem              员工核销取宠码
GET  /api/notifications              主人消息中心
```

## 目录结构

```
backend/   Rust/Axum/SQLx 后端（migrations/ 建表，tests/ 集成测试）
frontend/  Svelte 前端（src/staff 员工端，src/owner 主人端）
run.sh     一条命令启动
```
