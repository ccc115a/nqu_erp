#!/usr/bin/env bash
# NQU-ERP 一鍵安裝：檢查系統工具 → 安裝 Python/前端依賴 → 安裝 Playwright 瀏覽器 → 預建後端。
set -euo pipefail
cd "$(dirname "$0")"

have() { command -v "$1" >/dev/null 2>&1; }
step() { echo "==> $1"; }

fail() { echo "❌ $1" >&2; exit 1; }

# ---- 0. 系統工具檢查（缺了請先手動安裝，不自動 sudo）----
step "[0] 檢查系統工具..."
MISSING=""
have cargo || MISSING="$MISSING cargo（https://rustup.rs → curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh）"
have node || MISSING="$MISSING node（mac: brew install node / linux: apt install nodejs npm）"
have npm || MISSING="$MISSING npm（隨 node 安裝）"
have python3 || MISSING="$MISSING python3（mac: brew install python / linux: apt install python3）"
have sqlite3 || MISSING="$MISSING sqlite3（mac: brew install sqlite / linux: apt install sqlite3）"
have curl || MISSING="$MISSING curl（mac: brew install curl / linux: apt install curl）"
if [ -n "$MISSING" ]; then
  echo "缺少系統工具：$MISSING" >&2
  fail "請先安裝上述工具後再重跑 bash install.sh"
fi
echo "    cargo $(cargo --version), node $(node --version), npm $(npm --version), python3 $(python3 --version), sqlite3 $(sqlite3 --version)"

# ---- 1. seed.py 依賴：bcrypt ----
step "[1] 安裝 Python 依賴（bcrypt）..."
if python3 -c "import bcrypt" 2>/dev/null; then
  echo "    bcrypt 已安裝"
else
  pip3 install bcrypt || pip3 install --break-system-packages bcrypt || fail "bcrypt 安裝失敗"
fi

# ---- 2. 前端依賴（含 vitest）----
step "[2] 安裝前端依賴（frontend/npm）..."
cd frontend
if [ -f package-lock.json ]; then
  npm ci || npm install
else
  npm install
fi
test -x node_modules/.bin/vitest || fail "vitest 仍不存在，請檢查 npm install 輸出"
echo "    vitest $(node_modules/.bin/vitest --version)"
cd ..

# ---- 3. Playwright 瀏覽器（E2E 用）----
step "[3] 安裝 Playwright 瀏覽器（chromium）..."
if [ "$(uname)" = "Linux" ]; then
  (cd frontend && npx playwright install --with-deps chromium) || (cd frontend && npx playwright install chromium)
else
  (cd frontend && npx playwright install chromium)
fi

# ---- 4. 後端依賴預建 ----
step "[4] 預建後端（cargo build）..."
cargo build

echo ""
echo "✅ 安裝完成！接著可跑：bash test_all.sh"
