#!/usr/bin/env bash
# 家庭宠物寄养喂食与用药交接系统 —— 一条命令本地启动
# 用法： ./start.sh
set -euo pipefail
cd "$(dirname "$0")"

# 非交互 shell 中加载 rustup 环境
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

export HOST="${HOST:-127.0.0.1}"
export PORT="${PORT:-8080}"
export DATABASE_URL="${DATABASE_URL:-data/petcare.db}"
export STATIC_DIR="${STATIC_DIR:-frontend/dist}"
export UPLOADS_DIR="${UPLOADS_DIR:-data/uploads}"

mkdir -p data "$UPLOADS_DIR"

# 1) 前端：缺 node_modules 先装；每次做增量构建（无变更时 vite 很快空转）
if [ ! -d frontend/node_modules ]; then
  echo "==> 安装前端依赖 (npm install)"
  (cd frontend && npm install)
fi
echo "==> 构建前端 (vite build)"
(cd frontend && npm run build)

# 2) 后端：SQLx 使用运行时迁移（无需 DATABASE_URL 编译期变量）；增量编译，无变更时秒退
echo "==> 编译后端 (cargo build)"
(cd backend && cargo build)

echo "==> 启动服务：http://${HOST}:${PORT}"
echo "    员工 alice/bob 密码 staff123 ｜ 主人 wang/li 密码 owner123"
exec backend/target/debug/petcare
