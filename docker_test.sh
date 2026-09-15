#!/usr/bin/env bash
# Docker 建置與整合測試：建置映像 → 啟動 db/api/web → 等待健康 → 喂入假資料 →
# 逐一驗證 API（含 nginx 同源代理路徑）、前端頁面、重啟後資料持久化。
# 用法: bash docker_test.sh [--build] [--down]
#   --build  強制重新建置映像（預設：僅在映像不存在時建置）
#   --down   測試結束後關閉整個 stack（預設：保留 stack 以「上線」）
set -u

# api 不直接對外暴露 port（留給本機 dev 8080），所有 API 皆經 nginx 代理驗證
BASE_API="http://localhost/api/v1"        # 經 nginx 反向代理（與瀏覽器同路徑，驗證 /api 轉發）
BASE_FRONT="http://localhost"             # 前端靜態頁
PASS=0
FAIL=0

assert_contains() {
  local label="$1" body="$2" expected="$3"
  if echo "$body" | grep -q "$expected"; then
    echo "  ✅ $label"
    PASS=$((PASS + 1))
  else
    echo "  ❌ $label (expected '$expected')"
    echo "     got: ${body:0:200}"
    FAIL=$((FAIL + 1))
  fi
}

assert_eq() {
  local label="$1" actual="$2" expected="$3"
  if [ "$actual" = "$expected" ]; then
    echo "  ✅ $label"
    PASS=$((PASS + 1))
  else
    echo "  ❌ $label (expected '$expected', got '$actual')"
    FAIL=$((FAIL + 1))
  fi
}

assert_status() {
  local label="$1" actual="$2" expected="$3"
  if [ "$actual" = "$expected" ]; then
    echo "  ✅ $label (HTTP $actual)"
    PASS=$((PASS + 1))
  else
    echo "  ❌ $label (expected HTTP $expected, got HTTP $actual)"
    FAIL=$((FAIL + 1))
  fi
}

wait_http() {
  # 輪詢直到給定 URL 回傳非空（最多 N 次）
  local url="$1" label="$2" tries="${3:-30}"
  local i
  for i in $(seq 1 "$tries"); do
    if curl -sf "$url" >/dev/null 2>&1; then
      echo "    ${label} 就緒（第 $i 次嘗試）"
      return 0
    fi
    sleep 2
  done
  echo "  ❌ ${label} 逾時未就緒"
  return 1
}

echo "========================================="
echo "  NQU-ERP Docker 建置與整合測試"
echo "========================================="
echo ""

# ---- 0. 前置檢查 ----
echo "[0] 前置檢查"
if ! docker version >/dev/null 2>&1; then
  echo "❌ docker 未執行（docker daemon 無法連線）"; exit 1
fi
echo "    docker OK"

# seed.sql 需先在 host 產生（rust runtime 映像不包含 python）
if [ ! -f scripts/seed.sql ]; then
  echo "    產生 scripts/seed.sql..."
  python3 scripts/seed.py -o scripts/seed.sql --seed 42
fi
echo "    scripts/seed.sql OK"
echo ""

# ---- 1. 建置映像 ----
echo "[1] 建置映像"
if [ "${1:-}" = "--build" ] || [ "$(docker images -q nqu_erp-api 2>/dev/null)" = "" ] \
   || [ "$(docker images -q nqu_erp-web 2>/dev/null)" = "" ]; then
  docker compose build || { echo "❌ build 失敗"; exit 1; }
  echo "    build OK"
else
  echo "    映像已存在，跳過 build（加 --build 可強制重建）"
fi
echo ""

# ---- 2. 重置並啟動 stack ----
echo "[2] 重置並啟動 db / api / web"
docker compose down -v >/dev/null 2>&1
docker compose up -d --wait 2>&1 | tail -3
echo ""

# ---- 3. 健康檢查 ----
echo "[3] 健康檢查"
# 後端 readiness：poll 容器內 /health（回應 OK 才算就緒）
readiness=""
for i in $(seq 1 30); do
  readiness=$(docker compose exec -T api curl -sf http://localhost:8080/health 2>/dev/null || echo "")
  [ -n "$readiness" ] && break
  sleep 2
done
[ -n "$readiness" ] || { echo "❌ 後端未就緒"; docker compose ps; exit 1; }
assert_eq "後端容器內 /health" "$readiness" "OK"
if ! wait_http "$BASE_FRONT/" "前端首頁 (nginx)" 10; then
  echo "❌ 前端未就緒"; docker compose ps; exit 1
fi
FRONT_BODY=$(curl -s "$BASE_FRONT/")
assert_contains "前端首頁可存取 (nginx)" "$FRONT_BODY" 'id="root"'
echo ""

# ---- 4. Seed 假資料 ----
echo "[4] 喂入假資料"
docker compose --profile seed run --rm seed 2>/dev/null || { echo "❌ seed 失敗"; docker compose ps; exit 1; }
# PostgreSQL 的 auto-increment sequence 不會因 seed 的「顯式指定 id」而前進，
# 這會讓後續 INSERT 撞到 primary key（如 enrollment_id=1）。把各表 sequence 對齊到目前最大值。
echo "    同步 auto-increment sequence..."
docker compose exec -T db psql -U nqu -d nqu >/dev/null 2>&1 <<'SQL' || { echo "❌ sequence 同步失敗"; exit 1; }
SELECT setval('departments_dept_id_seq', (SELECT COALESCE(MAX(dept_id), 1) FROM departments));
SELECT setval('users_user_id_seq', (SELECT COALESCE(MAX(user_id), 1) FROM users));
SELECT setval('courses_course_id_seq', (SELECT COALESCE(MAX(course_id), 1) FROM courses));
SELECT setval('class_schedules_schedule_id_seq', (SELECT COALESCE(MAX(schedule_id), 1) FROM class_schedules));
SELECT setval('enrollments_enrollment_id_seq', (SELECT COALESCE(MAX(enrollment_id), 1) FROM enrollments));
SELECT setval('grades_grade_id_seq', (SELECT COALESCE(MAX(grade_id), 1) FROM grades));
SQL
COURSE_COUNT=$(docker compose exec -T db psql -U nqu -d nqu -tAc "SELECT COUNT(*) FROM courses;")
assert_eq "DB 課程數 = 15" "$COURSE_COUNT" "15"
USER_COUNT=$(docker compose exec -T db psql -U nqu -d nqu -tAc "SELECT COUNT(*) FROM users;")
assert_eq "DB 帳號數 = 42" "$USER_COUNT" "42"
echo ""

# ---- 5. 登入測試（直接打後端）----
echo "[5] 登入測試"
STU=$(curl -s -w "\n%{http_code}" -X POST "$BASE_API/auth/login" -H 'Content-Type: application/json' \
  -d '{"username":"11303001","password":"student123"}')
STU_HTTP=$(echo "$STU" | tail -1)
STU_BODY=$(echo "$STU" | sed '$d')
STU_TOKEN=$(echo "$STU_BODY" | python3 -c "import sys,json; print(json.load(sys.stdin).get('token',''))" 2>/dev/null || echo "")
assert_status "登入 (student)" "$STU_HTTP" "200"
assert_contains "token 不為空" "$STU_TOKEN" "ey"

TEA=$(curl -s -w "\n%{http_code}" -X POST "$BASE_API/auth/login" -H 'Content-Type: application/json' \
  -d '{"username":"T001","password":"teacher123"}')
TEA_HTTP=$(echo "$TEA" | tail -1)
TEA_BODY=$(echo "$TEA" | sed '$d')
TEA_TOKEN=$(echo "$TEA_BODY" | python3 -c "import sys,json; print(json.load(sys.stdin).get('token',''))" 2>/dev/null || echo "")
assert_status "登入 (teacher)" "$TEA_HTTP" "200"

BAD=$(curl -s -w "\n%{http_code}" -X POST "$BASE_API/auth/login" -H 'Content-Type: application/json' \
  -d '{"username":"11303001","password":"wrong"}')
BAD_HTTP=$(echo "$BAD" | tail -1)
BAD_BODY=$(echo "$BAD" | sed '$d')
assert_status "錯誤密碼 → 401" "$BAD_HTTP" "401"
assert_contains "回傳中文錯誤訊息" "$BAD_BODY" "帳號或密碼錯誤"
echo ""

# ---- 6. 課程查詢（經 nginx 代理，與瀏覽器同路徑）----
echo "[6] 課程查詢"
CRS=$(curl -s -w "\n%{http_code}" "$BASE_API/courses" -H "Authorization: Bearer $STU_TOKEN")
assert_status "GET /courses HTTP 200" "$(echo "$CRS" | tail -1)" "200"
CRS_COUNT=$(echo "$CRS" | sed '$d' | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")
assert_eq "課程數量 = 15" "$CRS_COUNT" "15"
echo ""

# ---- 7. 選課／退選 ----
echo "[7] 加選／退選"
ENR=$(curl -s -w "\n%{http_code}" -X POST "$BASE_API/enrollments" -H "Authorization: Bearer $STU_TOKEN" \
  -H 'Content-Type: application/json' -d '{"course_id":3}')
assert_status "加選 course 3" "$(echo "$ENR" | tail -1)" "200"
assert_contains "加選成功訊息" "$(echo "$ENR" | sed '$d')" "加選成功"

DUP=$(curl -s -X POST "$BASE_API/enrollments" -H "Authorization: Bearer $STU_TOKEN" \
  -H 'Content-Type: application/json' -d '{"course_id":3}')
assert_contains "重複加選被拒絕" "$DUP" "已選修過"

SCH=$(curl -s "$BASE_API/students/me/schedule" -H "Authorization: Bearer $STU_TOKEN")
assert_contains "課表有資料" "$SCH" "course"

DRP=$(curl -s -w "\n%{http_code}" -X DELETE "$BASE_API/enrollments" -H "Authorization: Bearer $STU_TOKEN" \
  -H 'Content-Type: application/json' -d '{"course_id":3}')
assert_status "退選 course 3" "$(echo "$DRP" | tail -1)" "200"
assert_contains "退選成功訊息" "$(echo "$DRP" | sed '$d')" "退選成功"
echo ""

# ---- 8. 教師成績 = TEST101 ----
echo "[8] 教師成績 / 管理員"
GRD=$(curl -s "$BASE_API/students/me/grades" -H "Authorization: Bearer $STU_TOKEN")
assert_contains "學生成績查詢有資料" "$GRD" "course"

TCR=$(curl -s "$BASE_API/teachers/me/courses" -H "Authorization: Bearer $TEA_TOKEN")
assert_eq "T001 授課數 >= 1" "$(( $(echo "$TCR" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo 0) >= 1 ))" "1"

ADM=$(curl -s -w "\n%{http_code}" -X POST "$BASE_API/auth/login" -H 'Content-Type: application/json' \
  -d '{"username":"admin","password":"admin123"}')
ADM_TOKEN=$(echo "$ADM" | sed '$d' | python3 -c "import sys,json; print(json.load(sys.stdin).get('token',''))" 2>/dev/null || echo "")
NEWU=$(curl -s -w "\n%{http_code}" -X POST "$BASE_API/admin/users" -H "Authorization: Bearer $ADM_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"username":"S099","password":"test1234","full_name":"測試學生","role":"STUDENT","dept_id":1,"email":"s099@test.nqu.edu.tw"}')
assert_status "管理員建立帳號 S099" "$(echo "$NEWU" | tail -1)" "200"
assert_contains "建立成功訊息" "$(echo "$NEWU" | sed '$d')" "帳號建立成功"
echo ""

# ---- 9. 重啟後資料持久化 + 防呆清理 ----
echo "[9] 重啟驗證（pgdata 持久化）"
docker compose restart api >/dev/null 2>&1
for i in $(seq 1 30); do
  readiness=$(docker compose exec -T api curl -sf http://localhost:8080/health 2>/dev/null || echo "")
  [ -n "$readiness" ] && break
  sleep 2
done
[ -n "$readiness" ] || { echo "❌ 重啟失敗"; exit 1; }
CRS2=$(curl -s "$BASE_API/courses" -H "Authorization: Bearer $STU_TOKEN")
CRS2_COUNT=$(echo "$CRS2" | python3 -c "import sys,json; print(len(json.load(sys.stdin)))" 2>/dev/null || echo "0")
assert_eq "重啟後課程數仍 = 15" "$CRS2_COUNT" "15"
U2=$(docker compose exec -T db psql -U nqu -d nqu -tAc "SELECT COUNT(*) FROM users WHERE username='S099';")
assert_eq "重啟後 S099 仍在 DB" "$U2" "1"
echo ""

# ---- Summary ----
echo "========================================="
TOTAL=$((PASS + FAIL))
echo "  結果: $PASS/$TOTAL 通過, $FAIL 失敗"
echo "========================================="
echo ""
if [ "$FAIL" -gt 0 ]; then
  echo "有一項以上失敗，stack 保留供診斷（可手動 docker compose down -v 清理）"
  exit 1
fi

if [ "${1:-}" = "--down" ]; then
  echo "按 --down 參數，關閉 stack..."
  docker compose down -v
else
echo "✅ 全部通過，stack 已上線："
echo "   前端/API  http://localhost          (admin/admin123, T001/teacher123, 11303001/student123)"
echo "   狀態      docker compose ps"
echo "   停止      docker compose down"
fi