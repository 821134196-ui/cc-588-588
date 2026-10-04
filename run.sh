#!/usr/bin/env bash
# 家庭宠物寄养喂食与用药交接系统 —— 一条命令本地启动
# 用法： ./run.sh          （生产模式：构建前端 + 启动后端，单端口 8080）
#        ./run.sh dev      （开发模式：Vite 5173 热更新 + 后端 8080）
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"

# ---- Rust 工具链（若尚未安装）----
if ! command -v cargo >/dev/null 2>&1; then
  echo "[run] 未检测到 cargo，正在安装 Rust 工具链…"
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
export PATH="$HOME/.cargo/bin:$PATH"

# ---- 前端依赖 ----
if [ ! -d frontend/node_modules ]; then
  echo "[run] 安装前端依赖…"
  (cd frontend && npm install)
fi

mkdir -p backend/data/uploads

if [ "${1:-}" = "dev" ]; then
  echo "[run] 开发模式：后端 :8080，前端 :5173（Vite 代理 /api 与 /uploads）"
  (cd frontend && npm run dev) &
  FRONT_PID=$!
  trap 'kill $FRONT_PID 2>/dev/null || true' EXIT
  (cd backend && cargo run)
else
  echo "[run] 构建前端…"
  (cd frontend && npm run build)
  echo "[run] 启动后端（同时托管前端与本地照片）…"
  echo "[run] 打开 http://localhost:8080"
  (cd backend && cargo run)
fi
