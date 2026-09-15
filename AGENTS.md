# AGENTS.md — NQU-ERP 開發指南

## 專案概述

通用校務系統 MVP（NQU-ERP），聚焦選課與成績管理。Backend Rust (Axum + SeaORM)，前端 React + TailwindCSS（v0.2 起）。v0.3 起涵蓋學生 / 教師 / 管理員三角色前端。

## Build & Run

```bash
# Backend
cargo build                          # 編譯（零 warnings 為目標）
cargo run                            # 啟動 server（自動跑 migration）
./target/debug/nqu-erp               # 直接執行 binary

# Frontend
cd frontend && npm install && npm run dev   # http://localhost:5173

# Docker (v0.4) — db(PostgreSQL) + api(Rust) + web(nginx 同源) + seed(一次性)
bash docker_test.sh                         # 建置→啟動→seed→23 項整合測試；全過保留 stack 上線
bash docker_test.sh --down                  # 收尾關閉 stack
docker compose --profile seed run --rm seed # 重新喂入假資料（一次性；seed 掛 profile 不會被 up 自動跑）
```


品質關卡（提交前必須全過）：

```bash
cargo fmt && cargo clippy && cargo build && bash test.sh
cd frontend && bash test.sh
```

## Testing

```bash
bash test.sh                         # 後端整合測試（72 tests，含 build + server + seed + API）
cargo test                           # Rust 單元測試（目前為空）

cd frontend
bash test.sh                         # 前端自動化（build → seed → Vitest 26 → Playwright E2E 10）
npm test                             # 僅 Vitest 單元測試（jsdom）
npx playwright test                  # 僅 E2E（自動起 Vite :5173）
```

`test.sh`（後端）會自動：build → 啟動 server → seed → 測試所有 API → kill server。退出碼 0 = 全數通過。
`frontend/test.sh` 會自動：`cargo build` → 啟動後端（輪詢 `/health`）→ `seed.py --seed 42` → 驗證課程數 `= 15` → Vitest → Playwright。E2E 前會重置 `dev.db`，用後需手動 `git checkout dev.db` 或重跑 seed。

### E2E 陷阱（易踩）

- 測試帳號：學生 `11303001` / `student123`；教師 `T001` / `teacher123`；管理員 `admin` / `admin123`
- 後端規則「**退選後不可重選**」：加退選 E2E 各用獨立課程（journey 用 course 3，重複加選測試用 course 8），避免測試間污染狀態
- Playwright `request` fixture 的 `Authorization` header 要手動加 `Bearer ` 前綴
- `page.addInitScript(() => localStorage.clear())` 會在**每次** `page.goto` 重跑把登入者登出 → 改在 `beforeEach` 先 `goto('/login')` 再 `evaluate` 清一次
- `getByText` / `getByRole('cell')` name 匹配不區分大小寫：`T001` 會命中 `t001@nqu.edu.tw` → 加 `exact: true`；`getByPlaceholder('姓名')` 會命中「搜尋帳號 / 姓名 / 角色」→ 加 `exact: true`
- Vitest `include` 限定 `src/__tests__`，否則會抓到 `e2e/*.spec.ts` 造成混亂
- E2E 依賴「課程 `.rounded-lg.border` 卡片、按鈕文字 加選/退選/已選修、中文成功訊息」等 DOM 結構
- 教師成績 E2E 用 `teacherGradeTarget()` 動態找未送交學生（用 `request` 查名冊），roster 輸入框 `aria-label="期中考/期末考 {學號}"`
- 管理員刪除需用 `page.once('dialog', d => d.accept())` 處理 `window.confirm`；刪除訊息中間夾帳號/課程名（如「帳號 S099 刪除成功」），斷言用 `刪除成功` substring

## 環境變數

在 `.env` 設定（已 gitignore）：

```
DATABASE_URL=sqlite://dev.db?mode=rwc   # 開發用 SQLite
JWT_SECRET=nqu-erp-secret-key-change-in-production
SERVER_ADDR=0.0.0.0:8080
```

切換 PostgreSQL：改 `DATABASE_URL=postgres://user:pass@localhost:5432/nqu_db`，程式碼零修改。

## 專案結構

```
nqu-erp/
├── src/
│   ├── main.rs              # 啟動入口、Router 設定、AppState
│   ├── config.rs            # Config::from_env()（dotenvy）
│   ├── db.rs                # establish_connection + run_migrations
│   ├── errors.rs            # AppError（HTTP status + message）
│   ├── entities/            # SeaORM entity（手動撰寫，非 CLI 產生）
│   │   ├── mod.rs
│   │   ├── departments.rs
│   │   ├── users.rs
│   │   ├── courses.rs
│   │   ├── class_schedules.rs
│   │   ├── enrollments.rs
│   │   └── grades.rs
│   ├── handlers/            # API handler 函式
│   │   ├── mod.rs
│   │   ├── auth.rs          # POST /api/v1/auth/login
│   │   ├── course.rs        # GET /courses, GET /teachers/me/courses
│   │   ├── enrollment.rs    # POST/DELETE /enrollments, GET /students/me/schedule
│   │   ├── grade.rs         # PUT /grades/batch, POST /grades/submit, GET /students/me/grades, GET /teachers/me/courses/:id/students
│   │   └── admin.rs         # POST/PUT/DELETE /admin/users, GET /admin/users, POST/PUT/DELETE /admin/courses, GET /admin/teachers, GET /admin/departments
│   └── middleware/
│       ├── mod.rs           # exports AuthUser, create_token
│       └── auth.rs          # AuthUser extractor + JWT encode/decode
├── migrations/              # SeaORM migration（6 tables + 3 indexes）
├── scripts/
│   ├── seed.py              # Python 假資料產生器（bcrypt）
│   └── seed.sql             # 產出的 SQL
├── test.sh                  # 自動化整合測試
├── frontend/                # React frontend（v0.2+）
│   ├── src/
│   │   ├── pages/           # Login / CourseList / MySchedule / MyGrades / TeacherCourses / TeacherGradeEntry / AdminUsers / AdminCourses
│   │   ├── components/      # Navbar / PrivateRoute / WeeklySchedule / GradeTable / RosterTable / UserForm / CourseForm
│   │   ├── api/client.ts    # Axios + JWT interceptor
│   │   ├── roles.ts         # ROLE_HOME（依角色定首頁）
│   │   └── __tests__/       # Vitest 單元測試（26 tests）
│   ├── e2e/app.spec.ts      # Playwright E2E（10 tests）
│   └── test.sh              # 前端自動化（build → seed → Vitest → Playwright）
└── _doc/
    ├── plan.md              # 完整規劃文件
    ├── v0.1.md              # v0.1 版本說明
    ├── v0.2.md              # v0.2 Student Frontend 版本說明
    ├── v0.3.md              # v0.3 Teacher/Admin Frontend 版本說明
    └── v0.3.1.md            # v0.3.1 Admin 修改/刪除帳號與課程
```

## Code Conventions

### Rust

- **Error Handling**: 所有 handler 回傳 `Result<Json<T>, AppError>`。用 `?` 傳播 DB/JSON 錯誤，用 `AppError::bad_request/unauthorized/forbidden/not_found/internal` 回傳業務錯誤。
- **中文錯誤訊息**: 業務錯誤訊息使用中文（如 `"帳號或密碼錯誤"`、`"課程時間衝突！"`）。
- **AuthUser**: Handler 第二個 parameter 使用 `auth: AuthUser`，自動從 `Authorization: Bearer <token>` 解析。用 `auth.is_student()` / `is_teacher()` / `is_admin()` 判斷角色。
- **Transaction**: 涉及多表寫入的操作（加選、退選）使用 `state.db.begin()` + `tx.commit()` 確保 ACID。
- **Entity**: 手動撰寫（因 SeaORM v2 CLI API 變動），含 `Relation` impl。
- **Migration**: `pk_auto` 從 `sea_orm_migration::schema` import；index 用 `manager.create_index()`。
- **不加不必要的 comments**，程式碼本身應自說明。
- **State**: 使用 `State(state): State<AppState>` 提取，不使用 extensions 注入。

### API 回傳格式

成功：
```json
{ "success": true, "message": "加選成功！" }
```

失敗：
```json
{ "error": "課程時間衝突！與已選課程在星期 1 第 6~8 節重疊" }
```

### Seed Data

```bash
python3 scripts/seed.py -o scripts/seed.sql --seed 42
sqlite3 dev.db < scripts/seed.sql                    # 重置資料一鍵指令
```

- `--seed 42` 確保可複製
- 密碼：學生 `student123`、教師 `teacher123`、管理員 `admin123`
- 學號格式：`113XXXXXX`（如 `11303001`）
- 教師帳號：`T001` ~ `T010`
- 佔用 port 8080，啟動前確認無其他 process

## Git

- `dev.db`、`.env`、`/target` 已 gitignore
- Commit message 使用中文，簡述改動
- 不要 commit `dev.db` 或任何 secrets

## 當前版本

v0.3.1 完成（Admin 修改/刪除帳號與課程）；v0.4 完成（Docker 部署）。規劃上 v0.5 為 CI/CD（GitHub Actions ci.yml + cd.yml，GHCR 映像）。詳見 `_doc/plan.md`、`_doc/v0.4.md`、`_doc/v0.5.md`。
