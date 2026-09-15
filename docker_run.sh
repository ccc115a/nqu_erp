#!/usr/bin/env bash
# docker_run.sh — 一鍵啟動 NQU-ERP Docker stack（db + api + web）並維持「資料持久化」。
# 職責＝上線/維運，不跑測試、不重置既有資料（與 docker_test.sh 建置驗證分離）。
#
# 用法:
#   bash docker_run.sh [port]      # 啟動/上線；[port] 指定對外 WEB_PORT（預設 80）
#   bash docker_run.sh stop        # 停止（保留資料）
#   bash docker_run.sh restart     # 重啟
#   bash docker_run.sh logs        # 看 log（可加服務名）
#   bash docker_run.sh status      # 目前狀態
#   bash docker_run.sh down        # 停止並清空資料（重新開始）
set -u

cd "$(dirname "$0")"
COMPOSE="docker compose"
PROFILE_SEED="$COMPOSE --profile seed"

# ---- 單一 dispatcher：命令用完即退出 ----
case "${1:-}" in
stop)
  echo "停止 stack（保留資料）..."
  $COMPOSE stop
  exit 0
  ;;
restart)
  echo "重啟 stack..."
  $COMPOSE restart
  exit 0
  ;;
down)
  echo "停止並清空資料（-v）..."
  $COMPOSE down -v
  exit 0
  ;;
logs)
  $COMPOSE logs -f "${2:-}"
  exit 0
  ;;
status)
  $COMPOSE ps
  exit 0
  ;;
esac

# ---- 其餘 = 上線路徑；第一個位置參數為 port（可省略）----
WEB_PORT="${1:-}"
if [ -n "$WEB_PORT" ] && ! [[ "$WEB_PORT" =~ ^[0-9]+$ ]]; then
  echo "❌ port 必須是數字（目前傳入: $WEB_PORT）"; exit 1
fi
export WEB_PORT

echo "========================================="
echo "  NQU-ERP Docker 啟動助手"
echo "========================================="

# ---- seed.sql 預先產生（若尚不存在）----
if [ ! -f scripts/seed.sql ]; then
  echo "  產生 scripts/seed.sql（首次需要 python3 + bcrypt）..."
  python3 scripts/seed.py -o scripts/seed.sql --seed 42 || { echo "❌ seed 產生失敗"; exit 1; }
fi

# ---- 建置（僅在映像缺失時）----
if [ "$(docker images -q nqu_erp-api 2>/dev/null)" = "" ] || [ "$(docker images -q nqu_erp-web 2>/dev/null)" = "" ]; then
  echo "  建置映像（首次）..."
  $COMPOSE build
else
  echo "  映像已存在，跳過建置（可 docker compose build 強制重建）"
fi

# ---- 啟動並等待健康 ----
echo "啟動 db / api / web ..."
$COMPOSE up -d --wait || { echo "❌ 啟動失敗"; $COMPOSE ps; exit 1; }

# ---- 檢查資料是否已存在，沒有的話才 seed ----
if $COMPOSE exec -T db psql -U nqu -d nqu -tAc "SELECT COUNT(*) FROM courses;" 2>/dev/null | grep -q '^0$'; then
  echo "資料庫為空，喂入假資料（一次性）..."
  $PROFILE_SEED run --rm seed || { echo "❌ seed 失敗"; $COMPOSE ps; exit 1; }
  # 同步 auto-increment sequence（seed 用顯式 id 寫入，Postgres sequence 不會自動前進）
  echo "  同步 sequence..."
  $COMPOSE exec -T db psql -U nqu -d nqu >/dev/null 2>&1 <<'SQL'
SELECT setval('departments_dept_id_seq', (SELECT COALESCE(MAX(dept_id), 1) FROM departments));
SELECT setval('users_user_id_seq', (SELECT COALESCE(MAX(user_id), 1) FROM users));
SELECT setval('courses_course_id_seq', (SELECT COALESCE(MAX(course_id), 1) FROM courses));
SELECT setval('class_schedules_schedule_id_seq', (SELECT COALESCE(MAX(schedule_id), 1) FROM class_schedules));
SELECT setval('enrollments_enrollment_id_seq', (SELECT COALESCE(MAX(enrollment_id), 1) FROM enrollments));
SELECT setval('grades_grade_id_seq', (SELECT COALESCE(MAX(grade_id), 1) FROM grades));
SQL
else
  echo "資料庫已有資料，跳過 seed"
fi

# ---- 完成 ----
echo ""
echo "✅ NQU-ERP 已上線："
$COMPOSE ps --format 'table {{.Name}}\t{{.Status}}'
echo ""
echo "   前端  http://localhost          (admin / admin123)"
echo "   教師  T001 / teacher123"
echo "   學生  11303001 / student123"
echo ""
