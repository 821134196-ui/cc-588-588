-- 用户：门店员工(staff) 与 宠物主人(owner)
CREATE TABLE users (
    id            TEXT PRIMARY KEY,
    username      TEXT NOT NULL UNIQUE,
    password_salt TEXT NOT NULL,
    password_hash TEXT NOT NULL,
    role          TEXT NOT NULL CHECK (role IN ('staff', 'owner')),
    display_name  TEXT NOT NULL,
    created_at    TEXT NOT NULL
);

-- 登录会话（本地模拟 token）
CREATE TABLE sessions (
    token_hash TEXT PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES users(id),
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL
);

-- 寄养宠物卡片
CREATE TABLE pets (
    id               TEXT PRIMARY KEY,
    owner_id         TEXT NOT NULL REFERENCES users(id),
    name             TEXT NOT NULL,
    species          TEXT NOT NULL DEFAULT '',
    food_portion     TEXT NOT NULL DEFAULT '',          -- 食物份量
    food_brand       TEXT NOT NULL DEFAULT '',          -- 当前主粮
    contraindications TEXT NOT NULL DEFAULT '',         -- 禁忌
    feeding_times    TEXT NOT NULL DEFAULT '08:00,18:00', -- 每日喂食时刻
    notes            TEXT NOT NULL DEFAULT '',
    boarding_start   TEXT NOT NULL,
    boarding_end     TEXT NOT NULL,
    status           TEXT NOT NULL DEFAULT 'boarding' CHECK (status IN ('boarding', 'picked_up')),
    picked_up_at     TEXT,
    created_at       TEXT NOT NULL
);

-- 宠物用药（药名/剂量/服用时刻）
CREATE TABLE medications (
    id          TEXT PRIMARY KEY,
    pet_id      TEXT NOT NULL REFERENCES pets(id) ON DELETE CASCADE,
    drug_name   TEXT NOT NULL,
    dosage      TEXT NOT NULL,          -- 剂量，如 50mg
    instructions TEXT NOT NULL DEFAULT '',
    times       TEXT NOT NULL DEFAULT '08:00,20:00', -- 每日给药时刻 HH:MM
    active      INTEGER NOT NULL DEFAULT 1,
    created_at  TEXT NOT NULL
);

-- 照护日历任务：每次喂食 / 每次给药各一行
CREATE TABLE care_tasks (
    id            TEXT PRIMARY KEY,
    pet_id        TEXT NOT NULL REFERENCES pets(id) ON DELETE CASCADE,
    kind          TEXT NOT NULL CHECK (kind IN ('feeding', 'medication')),
    medication_id TEXT REFERENCES medications(id) ON DELETE SET NULL,
    slot          TEXT NOT NULL UNIQUE, -- 幂等槽位，防止重复生成
    scheduled_at  TEXT NOT NULL,        -- 计划时间（本地时钟，ISO 文本）
    status        TEXT NOT NULL DEFAULT 'pending'
                  CHECK (status IN ('pending', 'done', 'anomaly', 'cancelled')),
    created_at    TEXT NOT NULL
);
CREATE INDEX idx_care_tasks_pet_time ON care_tasks(pet_id, scheduled_at);

-- 执行记录：一个任务至多一条（UNIQUE 保证重复提交不算执行两次）
CREATE TABLE executions (
    id          TEXT PRIMARY KEY,
    task_id     TEXT NOT NULL UNIQUE REFERENCES care_tasks(id),
    performer_id TEXT NOT NULL REFERENCES users(id),
    verifier_id TEXT REFERENCES users(id),  -- 给药双人核对：另一名员工
    executed_at TEXT NOT NULL,              -- 实际执行时间
    notes       TEXT NOT NULL DEFAULT '',
    photo_path  TEXT,
    created_at  TEXT NOT NULL
);

-- 异常事件：拒食 / 漏服 / 临时换粮 / 其他，必须单独记录经过
CREATE TABLE incidents (
    id          TEXT PRIMARY KEY,
    pet_id      TEXT NOT NULL REFERENCES pets(id) ON DELETE CASCADE,
    task_id     TEXT REFERENCES care_tasks(id) ON DELETE SET NULL,
    incident_type TEXT NOT NULL CHECK (incident_type IN
                  ('refused_food', 'missed_medication', 'food_swap', 'other')),
    description TEXT NOT NULL,   -- 异常情况
    handling    TEXT NOT NULL,   -- 处理经过
    staff_id    TEXT NOT NULL REFERENCES users(id),
    photo_path  TEXT,
    created_at  TEXT NOT NULL
);
CREATE INDEX idx_incidents_pet ON incidents(pet_id, created_at);

-- 本地模拟通知
CREATE TABLE notifications (
    id           TEXT PRIMARY KEY,
    owner_id     TEXT NOT NULL REFERENCES users(id),
    kind         TEXT NOT NULL,
    title        TEXT NOT NULL,
    body         TEXT NOT NULL,
    pet_id       TEXT,
    incident_id  TEXT,
    read_at      TEXT,
    created_at   TEXT NOT NULL
);
CREATE INDEX idx_notifications_owner ON notifications(owner_id, created_at);

-- 代接授权 / 取宠码
CREATE TABLE pickup_authorizations (
    id              TEXT PRIMARY KEY,
    pet_id          TEXT NOT NULL REFERENCES pets(id) ON DELETE CASCADE,
    owner_id        TEXT NOT NULL REFERENCES users(id),
    proxy_name      TEXT NOT NULL,
    proxy_contact   TEXT NOT NULL DEFAULT '',
    code_hash       TEXT NOT NULL UNIQUE,  -- 只存哈希，明文仅创建时返回一次
    status          TEXT NOT NULL DEFAULT 'active'
                    CHECK (status IN ('active', 'revoked', 'used')),
    expires_at      TEXT,
    used_at         TEXT,
    used_by_staff   TEXT REFERENCES users(id),
    revoke_reason   TEXT NOT NULL DEFAULT '',
    created_at      TEXT NOT NULL
);
CREATE INDEX idx_pickup_pet ON pickup_authorizations(pet_id);
