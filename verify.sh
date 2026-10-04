#!/usr/bin/env bash
# 端到端验证：
#   1) 同一任务重复提交不会执行两次（幂等：第二条返回 409，执行记录只有一条）
#   2) 给药必须另一名员工核对药名与剂量（无核对/本人核对/错密码/非员工均拦截）
#   3) 撤销代接授权后旧取宠码立即失效（核销被 403 拦截）
# 另覆盖：主人越权 403、异常必须写处理经过并通知主人、照片本地保存、提前接走取消后续任务
set -euo pipefail
cd "$(dirname "$0")"

# 非交互 shell 中加载 rustup 环境
[ -f "$HOME/.cargo/env" ] && . "$HOME/.cargo/env"

PORT="${VERIFY_PORT:-18080}"
TMP="$(mktemp -d)"
DB="$TMP/verify.db"
UPLOADS="$TMP/uploads"
mkdir -p "$UPLOADS"
BIN="backend/target/debug/petcare"

cleanup() {
  [ -n "${SRV_PID:-}" ] && kill "$SRV_PID" 2>/dev/null || true
  rm -rf "$TMP"
}
trap cleanup EXIT

if [ ! -x "$BIN" ]; then
  echo "==> 编译后端"
  (cd backend && cargo build)
fi

echo "==> 启动隔离验证服务 (port=$PORT, db=$DB)"
HOST=127.0.0.1 PORT="$PORT" DATABASE_URL="$DB" UPLOADS_DIR="$UPLOADS" \
  STATIC_DIR=frontend/dist RUST_LOG=warn \
  "$BIN" >"$TMP/server.log" 2>&1 &
SRV_PID=$!

# 等待端口就绪（未带令牌时 /api/me 返回 401，同样说明服务已起来）
for i in $(seq 1 50); do
  code=$(curl -s -o /dev/null -w '%{http_code}' "http://127.0.0.1:$PORT/api/me" || true)
  if [ "$code" = "401" ] || [ "$code" = "200" ]; then break; fi
  sleep 0.2
done

VERIFY_DB="$DB" VERIFY_PORT="$PORT" VERIFY_UPLOADS="$UPLOADS" python3 verify/e2e.py
