-- 用户：员工(staff) 与 主人(owner)
CREATE TABLE IF NOT EXISTS users (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    username        TEXT NOT NULL UNIQUE,
    password_hash   TEXT NOT NULL,
    salt            TEXT NOT NULL,
    display_name    TEXT NOT NULL,
    role            TEXT NOT NULL CHECK (role IN ('staff', 'owner')),
    created_at      TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 登录会话（本地模拟 token）
CREATE TABLE IF NOT EXISTS sessions (
    token       TEXT PRIMARY KEY,
    user_id     INTEGER NOT NULL REFERENCES users(id),
    created_at  TEXT NOT NULL DEFAULT (datetime('now'))
);

-- 宠物卡片
CREATE TABLE IF NOT EXISTS pets (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    name             TEXT NOT NULL,
    species          TEXT NOT NULL,            -- 猫/狗/...
    owner_id         INTEGER NOT NULL REFERENCES users(id),
    food_brand       TEXT NOT NULL DEFAULT '',  -- 当前主粮
    food_portion     TEXT NOT NULL DEFAULT '',  -- 每餐份量，如 40g
    meals_per_day    INTEGER NOT NULL DEFAULT 2,
    contraindications TEXT NOT NULL DEFAULT '', -- 禁忌（过敏、不能喂的东西）
    notes            TEXT NOT NULL DEFAULT '',
    status           TEXT NOT NULL DEFAULT 'boarding'
                     CHECK (status IN ('boarding', 'picked_up')),
    check_in_at      TEXT NOT NULL DEFAULT (datetime('now')),
    check_out_at     TEXT
);

-- 药品（挂在宠物卡片上）
CREATE TABLE IF NOT EXISTS medications (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    pet_id        INTEGER NOT NULL REFERENCES pets(id) ON DELETE CASCADE,
    name          TEXT NOT NULL,               -- 药名
    dosage        TEXT NOT NULL,               -- 剂量，如 5mg / 半片
    instructions  TEXT NOT NULL DEFAULT '',    -- 服用说明，如 随餐服用
    active        INTEGER NOT NULL DEFAULT 1
);

-- 照护任务：喂食 / 给药
CREATE TABLE IF NOT EXISTS care_tasks (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    pet_id        INTEGER NOT NULL REFERENCES pets(id) ON DELETE CASCADE,
    kind          TEXT NOT NULL CHECK (kind IN ('feed', 'medication')),
    med_id        INTEGER REFERENCES medications(id),
    scheduled_at  TEXT NOT NULL,               -- ISO8601 本地时间
    food_brand    TEXT NOT NULL DEFAULT '',    -- 计划粮（可临时换）
    portion       TEXT NOT NULL DEFAULT '',    -- 计划份量
    note          TEXT NOT NULL DEFAULT '',
    status        TEXT NOT NULL DEFAULT 'scheduled'
                  CHECK (status IN ('scheduled', 'done', 'cancelled')),
    created_by    INTEGER NOT NULL REFERENCES users(id),
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_care_tasks_pet_time ON care_tasks(pet_id, scheduled_at);
CREATE INDEX IF NOT EXISTS idx_care_tasks_status_time ON care_tasks(status, scheduled_at);

-- 执行记录：每个任务至多一条（唯一约束从数据库层面保证重复提交不会执行两次）
CREATE TABLE IF NOT EXISTS task_executions (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id             INTEGER NOT NULL UNIQUE REFERENCES care_tasks(id),
    executed_by         INTEGER NOT NULL REFERENCES users(id),
    executed_at         TEXT NOT NULL DEFAULT (datetime('now')),
    note                TEXT NOT NULL DEFAULT '',
    photo_path          TEXT NOT NULL DEFAULT '',
    -- 给药双人核对
    verifier_id         INTEGER REFERENCES users(id),
    verified_at         TEXT,
    verifier_med_name   TEXT NOT NULL DEFAULT '',
    verifier_dosage     TEXT NOT NULL DEFAULT ''
);

-- 异常事件：拒食 / 漏服 / 临时换粮 等，必须写处理经过并通知主人，不能直接勾选完成
CREATE TABLE IF NOT EXISTS incidents (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    pet_id        INTEGER NOT NULL REFERENCES pets(id),
    task_id       INTEGER REFERENCES care_tasks(id),
    incident_type TEXT NOT NULL CHECK (incident_type IN
                  ('refused_food', 'missed_medication', 'food_change', 'other')),
    description   TEXT NOT NULL,
    handling      TEXT NOT NULL,               -- 处理经过
    reported_by   INTEGER NOT NULL REFERENCES users(id),
    created_at    TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_incidents_pet ON incidents(pet_id, created_at);

-- 通知（本地模拟：写库 + 后端日志打印）
CREATE TABLE IF NOT EXISTS notifications (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    owner_id     INTEGER NOT NULL REFERENCES users(id),
    pet_id       INTEGER NOT NULL REFERENCES pets(id),
    incident_id  INTEGER REFERENCES incidents(id),
    title        TEXT NOT NULL,
    body         TEXT NOT NULL,
    is_read      INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL DEFAULT (datetime('now'))
);
CREATE INDEX IF NOT EXISTS idx_notifications_owner ON notifications(owner_id, created_at);

-- 取宠码（代接授权）：active 有效 / revoked 已撤销 / redeemed 已核销
CREATE TABLE IF NOT EXISTS pickup_codes (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    pet_id         INTEGER NOT NULL REFERENCES pets(id),
    code           TEXT NOT NULL UNIQUE,
    authorized_name TEXT NOT NULL,             -- 被授权代接人
    status         TEXT NOT NULL DEFAULT 'active'
                   CHECK (status IN ('active', 'revoked', 'redeemed')),
    created_by     INTEGER NOT NULL REFERENCES users(id),
    created_at     TEXT NOT NULL DEFAULT (datetime('now')),
    revoked_at     TEXT,
    redeemed_at    TEXT,
    redeemed_by    INTEGER REFERENCES users(id)
);
CREATE INDEX IF NOT EXISTS idx_pickup_codes_pet ON pickup_codes(pet_id);
