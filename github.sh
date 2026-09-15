#!/usr/bin/env bash
# github.sh — 一鍵 add → commit → push（GitHub）。
# 用法:
#   bash github.sh               # 自動: 檢視變更 → 全 stage → commit（訊息自動帶日期/版本）→ push
#   bash github.sh "說明文字"     # 同上，但用指定訊息
#   bash github.sh --dry-run     # 只列出將被 commit 的檔案，不寫入
set -u
cd "$(dirname "$0")"

# ---- 前置檢查 ----
command -v git >/dev/null || { echo "❌ 找不到 git"; exit 1; }
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || { echo "❌ 不在 git repo 內"; exit 1; }
git remote get-url origin >/dev/null 2>&1 || { echo "❌ 沒有 origin remote（需先 git remote add）"; exit 1; }

# ---- 收集變更 ----
STAGED_ALL=$(git status --short)
[ -n "$STAGED_ALL" ] || { echo "✔ 沒有變更，無事可做"; exit 0; }

if [ "${1:-}" = "--dry-run" ]; then
  echo "將要 commit 的檔案："
  echo "$STAGED_ALL"
  exit 0
fi

# ---- 安全檢查：攔截不該進 repo 的檔案（D=刪除無害，放行）----
RISKY=$(printf '%s\n' "$STAGED_ALL" | grep -Ev '^D' | grep -E '(^| )\.[Dd][Ss]_[Ss]tore( |$)|dev\.db|(^| )\.env( |$)|(^| )target/|node_modules/')
if [ -n "$RISKY" ]; then
  echo "⚠️  偵測到以下暫存內容可能不該 push（secret / 二進位 / 垃圾）："
  echo "$RISKY"
  echo "── 請先 git reset / 更新 .gitignore 後再跑 ──"
  exit 1
fi

# ---- stage + commit ----
git add -A
if [ -z "${1:-}" ]; then
  VER=$(grep -m1 '版本 \*\*v' README.md 2>/dev/null | grep -o 'v[0-9.]*' | head -1)
  MSG="更新（${VER:-}）：$(date +%Y-%m-%d)"
else
  MSG="$1"
fi
git commit -m "$MSG" || { echo "❌ commit 失敗"; exit 1; }

# ---- push ----
echo "推送 origin main ..."
git push origin HEAD || { echo "❌ push 失敗（可能需先 pull --rebase，或 SSH key 未設定）"; exit 1; }
echo "✅ 已完成："
git log --oneline -1
