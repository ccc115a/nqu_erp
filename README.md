# NQU-ERP 校務系統

通用校務系統 MVP，聚焦**選課與成績管理**核心流程，涵蓋學生 / 教師 / 管理員三角色。目前版本 **v0.5**（CI/CD：GitHub Actions 自動跑 fmt/clippy/測試 131＋Docker 23 整合，main 自動建置並推送 GHCR 映像）。

## 技術堆疊

| 層級 | 技術 |
|------|------|
| 後端 | Rust（Axum + SeaORM v2 + JWT + bcrypt） |
| 前端 | React + TypeScript + TailwindCSS + Vite |
| 資料庫 | SQLite（開發）/ PostgreSQL（生產），靠 `DATABASE_URL` 切換 |
| 測試 | Rust 整合測試（test.sh）、Vitest、Playwright E2E |

## 安裝與啟動

需求：Rust toolchain、Node.js（+ Python3 與 `sqlite3` 用於 seed）、`bcrypt` Python 套件。

```bash
# 1. 在專案根目錄建立 .env（內容見下方）

# 2. 啟動後端（自動跑 migration，監聽 0.0.0.0:8080）
cargo run

# 3. 插入假資料（另一 terminal）
python3 scripts/seed.py -o scripts/seed.sql --seed 42
sqlite3 dev.db < scripts/seed.sql

# 4. 啟動前端（http://localhost:5173）
cd frontend && npm install && npm run dev
```

或使用 `./run.sh` 一鍵完成：建置後端 → 重置 DB → seed → 啟動前後端（Ctrl+C 停止）。

`.env`：

```
DATABASE_URL=sqlite://dev.db?mode=rwc
JWT_SECRET=nqu-erp-secret-key-change-in-production
SERVER_ADDR=0.0.0.0:8080
```

切換 PostgreSQL 只需改 `DATABASE_URL=postgres://user:pass@localhost:5432/nqu_db`，程式碼零修改。

## Docker 部署（v0.4）

將整套系統「打包成 stack」一鍵上線：`db`（PostgreSQL 16）+ `api`（Rust release）+ `web`（nginx 同源代理），瀏覽器只打一個 port，免 CORS、免手動開 migration。

```bash
bash docker_run.sh                 # 啟動/上線：首建映像 → up → 空 DB 才 seed（含 sequence 同步）→ 顯示登入資訊
bash docker_run.sh 3000            # 指定對外 port（預設 80，改 http://localhost:3000）
bash docker_run.sh stop|restart|logs|status   # 維運（stop 保留資料 / logs 可加服務名）
bash docker_run.sh down            # 停止並清空資料（重新開始）

bash docker_test.sh                # 上線前整合測試：建置→重置→seed→23 項驗證；全過保留 stack 上線
bash docker_test.sh --down         # 測完收尾關閉
```

- **api 不對外暴露 host port**（與本機 dev `test.sh` 共存的 8080 會衝突），全部 API 走 nginx 同源 `/api/v1` 反向代理。
- **資料持久化**：`pgdata` volume；`stop`/`restart` 不丟資料，`down`（`-v`）才清空。
- **seed 為一次性**（掛 `profiles: ["seed"]`，不會隨 `up` 自動跑）；seed 用顯式 id 寫入，需同步 Postgres auto-increment sequence（腳本已自動處理）。

測試帳號：`admin / admin123`（管理員）、`T001 / teacher123`（教師）、`11303001 / student123`（學生）。

## 測試帳號

| 角色 | 帳號 | 密碼 |
|------|------|------|
| 學生 | `11303001` | `student123` |
| 教師 | `T001` | `teacher123` |
| 管理員 | `admin` | `admin123` |

## 功能一覽

- **學生端**：課程查詢、加選 / 退選（衝堂與名額檢查）、個人週課表、歷年成績
- **教師端**：授課清單、課程學生名冊、期中 / 期末成績登錄與鎖定送交
- **管理員端**：帳號與課程的建立 / 修改 / 刪除（含防刪保護）、教師與科系列表

## 測試

```bash
bash test.sh              # 後端整合測試：build → server → seed → API（72 tests）
cargo test                # Rust 單元測試（目前為空）

cd frontend
bash test.sh              # build → seed → Vitest（26）→ Playwright E2E（10）
npm test                  # 僅 Vitest（jsdom）
npx playwright test       # 僅 E2E
```

## API 概覽

前綴 `/api/v1`，認證用 `Authorization: Bearer <JWT>`。

| Method | Path | 角色 |
|--------|------|------|
| POST | `/auth/login` | 全部 |
| GET | `/courses` | 全部 |
| POST / DELETE | `/enrollments` | 學生 |
| GET | `/students/me/schedule`, `/students/me/grades` | 學生 |
| GET | `/teachers/me/courses`, `/teachers/me/courses/{id}/students` | 教師 |
| PUT | `/grades/batch`（登錄）/ POST `/grades/submit`（鎖定） | 教師 |
| GET/POST/PUT/DELETE | `/admin/users`、`/admin/courses` | 管理員 |
| GET | `/admin/teachers`, `/admin/departments` | 管理員 |

成功回傳 `{ "success": true, "message": "..." }`，錯誤回傳 `{ "error": "中文錯誤訊息" }`。

## 專案結構

```
src/
├── main.rs              # 啟動入口、Router、AppState、CORS
├── config.rs / db.rs    # 環境變數、DB 連線與 migration
├── errors.rs            # AppError（HTTP status + 訊息）
├── entities/            # SeaORM entity（手動撰寫，非 CLI 產生）
├── handlers/            # auth / course / enrollment / grade / admin
└── middleware/          # AuthUser extractor + JWT
migrations/              # SeaORM migration（6 tables + 3 indexes）
scripts/                 # seed.py（假資料產生器）+ seed.sql
frontend/
├── src/pages/           # Login / CourseList / MySchedule / MyGrades / TeacherCourses / TeacherGradeEntry / AdminUsers / AdminCourses
├── src/components/      # Navbar / PrivateRoute / WeeklySchedule / GradeTable / RosterTable / UserForm / CourseForm
├── src/__tests__/       # Vitest 單元測試
└── e2e/                 # Playwright E2E
_doc/                    # plan.md + 各版本說明（v0.1 / v0.2 / v0.3 / v0.3.1 / v0.4 / v0.5）
.github/workflows/       # ci.yml（CI）+ cd.yml（CD push main→GHCR）
```

詳細規畫與 API 設計見 `_doc/plan.md`，版本說明見各 `_doc/vX.Y.md`。