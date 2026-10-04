# 家庭宠物寄养喂食与用药交接系统 🐾

寄养门店按主人的照护要求安排每日喂食与给药任务，员工记录实际执行情况（时间、备注、照片、异常），
主人可随时查看自己宠物是否按时吃饭用药。后端 **Rust + Axum + SQLx + SQLite**，前端 **Svelte + Vite**。

## 一条命令本地启动

```bash
./start.sh
```

脚本会自动：安装前端依赖（首次）→ 构建 Svelte 静态资源 → 编译 Rust 后端 → 建库迁移 → 写入演示数据 → 启动服务。

打开 http://127.0.0.1:8080 （可用 `PORT=xxxx ./start.sh` 改端口）。数据、照片均保存在本地 `data/` 目录，通知在后端日志中本地模拟输出（🔔）。

### 演示账号

| 角色 | 账号 | 密码 | 说明 |
| --- | --- | --- | --- |
| 员工 | `alice` / `bob` | `staff123` | 两名员工用于演示给药双人核对 |
| 主人 | `wang` | `owner123` | 宠物「咪咪」（猫，每日阿莫西林，需双人核对） |
| 主人 | `li` | `owner123` | 宠物「豆豆」（狗，用于越权隔离对照） |

前端开发热更新：`cd frontend && npm run dev`（5173，自动代理 `/api` 到 8080）。

## 验证

```bash
./verify.sh     # 启动隔离的临时库实例，跑 49 项端到端断言后自动清理
```

重点验证三条硬性要求：

1. **重复执行幂等**：任务已有执行记录后，同一员工或换员工、JSON 或 multipart 再次提交均返回 `409`，`task_executions.task_id` 有 UNIQUE 约束兜底，数据库始终只有一条记录。
2. **给药双人核对**：给药任务完成时必须提供**另一名员工**的账号密码，后端校验其身份为 staff、密码正确、且与执行人不是同一人，核对药名与剂量后留痕（`verifier_id / verified_at / verifier_med_name / verifier_dosage`）；无核对人、本人核对、错密码、主人充当核对人均被 `400` 拦截且不产生记录。
3. **撤销代接后旧码失效**：主人撤销代接授权后，旧 8 位取宠码立即 `revoked`，门店核销返回 `403` 拒绝放宠；不存在的码同样拦截；核销成功后码一次性失效，同宠物其他有效码一并作废。

## 业务规则落点

- **宠物卡片**：主粮、每餐份量、每日餐数、禁忌（过敏/禁食）、备注；药品卡含药名、剂量、服用说明。
- **照护日历**：员工按宠物排喂食/给药任务；执行时记录实际时间、备注，可上传照片（本地 `data/uploads/`，≤10MB）。
- **异常处理**：拒食 / 漏服 / 临时换粮 / 其他必须走「上报异常」，**处理经过为必填**，不能靠勾选完成掩盖；提交后写入异常台账并给主人生成通知（本地模拟推送）。
- **提前接走**：`/pets/:id/checkout` 或取宠码核销后，宠物标记已接走、后续 `scheduled` 任务批量取消、有效代接码全部作废；已完成记录保持不变。
- **权限隔离**：主人只能查看自己名下宠物（宠物/任务/异常/通知接口均做属主校验，越权返回 `403`），门店已填写内容主人端无任何修改入口；任务执行、异常上报、取宠核销仅员工可操作。
- **代接授权**：仅主人（或员工）可为在店宠物生成 8 位随机码；主人在自己的页面可见完整码并可撤销，员工列表中码打码，只能凭代接人报码核销。

## 结构

```
backend/
  Cargo.toml
  migrations/0001_init.sql      # SQLite 建表（幂等）
  src/main.rs                   # 启动、静态资源、SPA 回退
  src/db.rs                     # 运行时迁移 + 密码哈希 + 演示种子
  src/handlers.rs               # 路由/鉴权提取器/全部业务 API
  src/models.rs  src/error.rs
frontend/
  src/App.svelte                # 登录外壳
  src/components/               # StaffView / OwnerView / PetCard / TaskItem / IncidentForm ...
  src/lib/api.js                # fetch 封装（Bearer 令牌）
start.sh                        # 一条命令启动
verify.sh  verify/e2e.py        # 端到端验证
```

## 主要 API

`POST /api/auth/login` · `GET /api/pets` · `POST /api/pets` · `GET /api/pets/:id`
`POST /api/pets/:id/medications` · `POST /api/tasks` · `GET /api/tasks?pet_id=&date=`
`POST /api/tasks/:id/execute`（JSON）· `POST /api/tasks/:id/execute-photo`（multipart 照片+核对凭证）
`POST /api/incidents` · `GET /api/notifications`
`POST /api/pickup-codes` · `POST /api/pickup-codes/:id/revoke` · `POST /api/pickup/redeem` · `POST /api/pets/:id/checkout`
