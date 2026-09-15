# 校務系統 MVP 實作規劃

## 系統概述

通用校務系統 MVP，聚焦「選課與成績管理」核心流程，服務學生、教師與教務處三類使用者。

## 技術堆疊

| 層級 | 技術選型 |
|------|---------|
| 前端 | React + TailwindCSS（RWD 響應式） |
| 後端 | Rust (Axum + SeaORM) |
| 資料庫 | SQLite（開發）/ PostgreSQL（生產），透過 `DATABASE_URL` 切換 |
| 快取/佇列 | Redis（名額快取、加選排隊）— 後期優化 |
| 認證 | JWT + RBAC（角色權限控管） |
| 假資料 | Python seed script（Faker + argparse） |

### 資料庫抽換機制

- 使用 **SeaORM**，透過 `sea_orm::Database::connect(&database_url)` 根據 URL scheme 自動選擇 SQLite 或 PostgreSQL driver
- Entity 由 `sea-orm-cli` 從 migration 產生（本專案因 SeaORM v2 API 變動改為手動撰寫），backend-agnostic
- SeaQuery 查詢建立器自動處理 dialect 差異（`?` vs `$1`、`RETURNING` 支援等）
- 切換方式：只需改 `.env` 的 `DATABASE_URL`，程式碼零修改

```bash
# 開發用 SQLite
DATABASE_URL=sqlite://dev.db?mode=rwc

# 生產用 PostgreSQL
DATABASE_URL=postgres://user:pass@localhost:5432/nqu_db
```

## 核心使用者與功能

### 學生端 (Student Portal)
- 課程查詢與加退選（即時名額、3 步驟內完成）
- 個人週課表自動生成（支援 PDF/行事曆匯出）
- 歷年成績單與學分累計查詢

### 教師端 (Faculty Portal)
- 授課課表與學生名冊匯出
- 期中/期末成績線上填報與鎖定送交

### 管理員端 (Admin Portal)
- 開課資料匯入與選課時程管理（預選、正選、加退選）
- 帳號與權限控管（SSO 整合）

## 資料庫架構 (ERD)

### entities

```
departments (科系表)
├── dept_id        INTEGER PRIMARY KEY AUTOINCREMENT
├── dept_code      VARCHAR(10) UNIQUE NOT NULL
└── dept_name      VARCHAR(50) NOT NULL

users (使用者表)
├── user_id        INTEGER PRIMARY KEY AUTOINCREMENT
├── username       VARCHAR(20) UNIQUE NOT NULL    -- 學號/教職工號
├── password_hash  VARCHAR(255) NOT NULL
├── full_name      VARCHAR(50) NOT NULL
├── role           VARCHAR(10) NOT NULL            -- STUDENT / TEACHER / ADMIN
├── dept_id        INT REFERENCES departments
├── email          VARCHAR(100) UNIQUE NOT NULL
└── created_at     DATETIME DEFAULT CURRENT_TIMESTAMP

courses (課程表)
├── course_id      INTEGER PRIMARY KEY AUTOINCREMENT
├── course_code    VARCHAR(20) NOT NULL
├── academic_year  INT NOT NULL
├── semester       INT NOT NULL                    -- 1 或 2
├── course_name    VARCHAR(100) NOT NULL
├── teacher_id     INT REFERENCES users(user_id)
├── credits        INT NOT NULL CHECK (credits > 0)
├── capacity       INT NOT NULL CHECK (capacity > 0)
├── enrolled_count INT DEFAULT 0
└── dept_id        INT REFERENCES departments

class_schedules (上課時間地點表)
├── schedule_id    INTEGER PRIMARY KEY AUTOINCREMENT
├── course_id      INT REFERENCES courses ON DELETE CASCADE
├── day_of_week    INT CHECK (day_of_week BETWEEN 1 AND 7)
├── start_period   INT CHECK (start_period BETWEEN 1 AND 14)
├── end_period     INT CHECK (end_period BETWEEN start_period AND 14)
└── location       VARCHAR(50) NOT NULL

enrollments (選課紀錄表)
├── enrollment_id  INTEGER PRIMARY KEY AUTOINCREMENT
├── student_id     INT REFERENCES users
├── course_id      INT REFERENCES courses
├── status         VARCHAR(15) DEFAULT 'ENROLLED'  -- ENROLLED / DROPPED
├── created_at     DATETIME DEFAULT CURRENT_TIMESTAMP
└── UNIQUE (student_id, course_id)

grades (成績表)
├── grade_id       INTEGER PRIMARY KEY AUTOINCREMENT
├── enrollment_id  INT UNIQUE REFERENCES enrollments ON DELETE CASCADE
├── midterm_score  NUMERIC(5,2) CHECK (midterm_score BETWEEN 0 AND 100)
├── final_score    NUMERIC(5,2) CHECK (final_score BETWEEN 0 AND 100)
├── total_score    NUMERIC(5,2) CHECK (total_score BETWEEN 0 AND 100)
├── is_submitted   BOOLEAN DEFAULT FALSE
└── updated_at     DATETIME DEFAULT CURRENT_TIMESTAMP
```

### 關聯圖

```
departments ──1:N── users
departments ──1:N── courses
users (teacher) ──1:N── courses
courses ──1:N── class_schedules
users (student) ──1:N── enrollments ──N:1── courses
enrollments ──1:1── grades
```

### 索引

```sql
CREATE INDEX idx_courses_year_sem ON courses(academic_year, semester);
CREATE INDEX idx_enrollments_student ON enrollments(student_id, status);
CREATE INDEX idx_schedules_course ON class_schedules(course_id);
```

## 核心業務邏輯

### 1. 加選課程流程（高併發安全）

```
學生發起 POST /api/v1/enrollments
    │
    ▼
[1] 衝堂檢查
    查詢學生已選課程時間 vs 新課程時間
    衝堂條件: (DayA == DayB) AND (StartA <= EndB) AND (StartB <= EndA)
    │
    ▼ (無衝突)
[2] 原子化名額扣減
    UPDATE courses SET enrolled_count = enrolled_count + 1
    WHERE course_id = ? AND enrolled_count < capacity
    影響列數 = 0 → 名額已滿，回傳失敗
    │
    ▼ (成功)
[3] 寫入選課紀錄 + 初始化成績紀錄
    BEGIN TRANSACTION
    INSERT INTO enrollments ...
    INSERT INTO grades (enrollment_id) ...
    COMMIT
```

- 使用 SeaORM Transaction 確保 ACID
- SQL 條件式 UPDATE 防止超賣（Overbooking）

### 2. 退選課程流程

```
DELETE /api/v1/enrollments?student_id=X&course_id=Y
    │
    ▼
[1] 更新 enrollment status = 'DROPPED'
[2] 該門課 enrolled_count - 1
[3] 刪除對應 grades 紀錄
```

### 3. 教師成績登錄

```
PUT /api/v1/grades/batch
    - 批次寫入期中/期末成績
    - is_submitted = false（可修改）

POST /api/v1/grades/submit
    - 將指定課程所有成績 is_submitted 設為 true
    - 鎖定後不可再修改
```

### 4. 學生個人課表查詢

```
GET /api/v1/students/me/schedule?year=113&semester=1

JOIN enrollments → courses → users (teacher) → class_schedules
回傳週課表結構（day_of_week, start_period, end_period, location, course_name, teacher_name）
```

## API 端點設計

| Method | Path | 說明 | 角色 |
|--------|------|------|------|
| POST | /api/v1/auth/login | 登入取得 JWT | All |
| GET | /api/v1/courses | 查詢課程清單 | All |
| POST | /api/v1/enrollments | 加選課程 | Student |
| DELETE | /api/v1/enrollments | 退選課程 | Student |
| GET | /api/v1/students/me/schedule | 個人課表 | Student |
| GET | /api/v1/students/me/grades | 歷年成績 | Student |
| GET | /api/v1/teachers/me/courses | 授課清單 | Teacher |
| GET | /api/v1/teachers/me/courses/{id}/students | 課程學生名冊（含成績） | Teacher |
| PUT | /api/v1/grades/batch | 批次成績登錄 | Teacher |
| POST | /api/v1/grades/submit | 成績鎖定送交 | Teacher |
| GET | /api/v1/admin/users | 帳號列表 | Admin |
| POST | /api/v1/admin/users | 建立帳號 | Admin |
| POST | /api/v1/admin/courses | 匯入開課資料 | Admin |
| GET | /api/v1/admin/teachers | 教師列表（開課下拉） | Admin |
| GET | /api/v1/admin/departments | 科系列表（表單下拉） | Admin |
| PUT | /api/v1/admin/enrollment-period | 設定選課時程 | Admin |

## 假資料 Seed 計畫

### 方案

使用 **Python script**（`scripts/seed.py`）搭配 **Faker** 產生假資料，產出 SQL INSERT 指令檔（`scripts/seed.sql`），可直接對 SQLite 或 PostgreSQL 執行。

### 工具

- Python 3.10+
- `faker` — 產生中文姓名、email 等
- `bcrypt` — 密碼雜湊（與 Rust bcrypt crate 相容）

### 假資料規模

| 表 | 數量 | 說明 |
|----|------|------|
| departments | 5 | 資工、電機、機械、企管、觀光 |
| users (teacher) | 10 | 每系 2 位教師 |
| users (student) | 30 | 每系 6 位學生 |
| users (admin) | 2 | 教務處管理員 |
| courses | 15 | 113學年第1學期，每門課 1-2 個班 |
| class_schedules | 25 | 每門課 1-2 個時段 |
| enrollments | 50 | 每位學生選 1-4 門課 |
| grades | 50 | 對應 enrollment，部分已送交 |

### 假資料內容規則

**departments**
```
CSIE  資訊工程學系
EE    電機工程學系
ME    機械工程學系
BA    企業管理學系
TM    觀光管理學系
```

**users (teacher) — 教師帳號**
```
T001 ~ T010, password: teacher123
姓名: 用 Faker zh_TW 產生
email: {username}@nqu.edu.tw
```

**users (student) — 學生帳號**
```
S001 ~ S030, password: student123
學號格式: 113XXXXXX (e.g. 11303001)
姓名: 用 Faker zh_TW 產生
email: {username}@stu.nqu.edu.tw
```

**users (admin) — 管理員帳號**
```
admin / admin123
dean / dean123
```

**courses — 課程資料**
```
course_code 格式: {dept_code}{學分}{序號}  (e.g. CSIE301)
課程名稱: 資料結構、演算法、計算機網路、微積分、線性代數、普通物理、
          電路學、程式設計、資料庫系統、企業管理、會計學、觀光英語、
          人力資源管理、機械設計、熱力學
teacher_id: 隨機分配給同系教師
capacity: 30~60
academic_year: 113, semester: 1
```

**class_schedules — 上課時間**
```
day_of_week: 1~5 (週一到週五)
start_period: 1~14 隨機
end_period: start + 1 或 start + 2
location: 理工大樓 E{201~305} / 管理大樓 M{101~203} / 人文大樓 H{101~105}
```

**enrollments — 選課紀錄**
```
每位學生隨機選 1~4 門課
避免衝堂（同一學生同時段只選一門）
status: 90% ENROLLED, 10% DROPPED
```

**grades — 成績**
```
midterm_score: 50~98 隨機
final_score: 45~100 隨機
total_score: (midterm * 0.4 + final * 0.6) 四捨五入
is_submitted: 70% true, 30% false
```

### Seed 檔案結構

```
scripts/
├── seed.py          -- Python 假資料產生器
├── seed.sql         -- 產出的 SQL INSERT 檔
└── run_seed.sh      -- 一鍵執行: python seed.py > seed.sql && sqlite3 dev.db < seed.sql
```

### 使用方式

```bash
# 產生 SQL 檔
python3 scripts/seed.py > scripts/seed.sql

# 塞入 SQLite
sqlite3 dev.db < scripts/seed.sql

# 塞入 PostgreSQL
psql -d nqu_db -f scripts/seed.sql
```

## 專案目錄結構

```
nqu-erp/
├── Cargo.toml
├── .env
├── src/
│   ├── main.rs                  -- 啟動入口、路由設定
│   ├── config.rs                -- 環境變數設定
│   ├── db.rs                    -- sea_orm::Database::connect
│   ├── entities/                -- sea-orm-cli 產生
│   │   ├── mod.rs
│   │   ├── departments.rs
│   │   ├── users.rs
│   │   ├── courses.rs
│   │   ├── class_schedules.rs
│   │   ├── enrollments.rs
│   │   └── grades.rs
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── auth.rs
│   │   ├── enrollment.rs
│   │   ├── grade.rs
│   │   ├── course.rs
│   │   └── admin.rs
│   ├── middleware/
│   │   ├── auth.rs
│   │   └── mod.rs
│   └── errors.rs
├── migrations/
│   ├── Cargo.toml
│   └── src/
│       ├── lib.rs
│       ├── m20240101_000001_create_departments.rs
│       ├── m20240101_000002_create_users.rs
│       ├── m20240101_000003_create_courses.rs
│       ├── m20240101_000004_create_class_schedules.rs
│       ├── m20240101_000005_create_enrollments.rs
│       └── m20240101_000006_create_grades.rs
├── scripts/
│   ├── seed.py                   -- Python 假資料產生器
│   ├── seed.sql                  -- 產出的 SQL INSERT 檔
│   └── run_seed.sh              -- 一鍵 seed
├── frontend/
│   ├── package.json
│   ├── src/
│   │   ├── App.tsx
│   │   ├── pages/
│   │   │   ├── Login.tsx
│   │   │   ├── CourseList.tsx
│   │   │   ├── MySchedule.tsx
│   │   │   ├── MyGrades.tsx
│   │   │   └── TeacherGrade.tsx
│   │   ├── components/
│   │   │   ├── Navbar.tsx
│   │   │   ├── WeeklySchedule.tsx
│   │   │   └── GradeTable.tsx
│   │   └── api/
│   │       └── client.ts
│   └── tailwind.config.js
└── README.md
```

## 版本規劃 (Release Plan)

### v0.1 — Backend Core ✅

> 後端 API + 資料庫 + 假資料，curl 測試驗證

**已完成：**
- Cargo 專案建立、SeaORM migration（6 tables, 3 indexes）
- JWT 認證 + RBAC（Student / Teacher / Admin）
- 所有核心 API：auth、courses、enrollments、grades、admin
- Python seed script（bcrypt, `--seed 42` 可複製）
- SQLite/PostgreSQL DB 抽換（`DATABASE_URL`）
- `test.sh` 33 項自動化整合測試
- `_doc/v0.1.md` 版本說明

**交付物：** API 透過 curl 驗證，`bash test.sh` 全數通過

---

### v0.2 — Student Frontend ✅

> 學生端 React 介面

**已完成：**
- 登入頁面（JWT token 管理、角色導向）
- 課程瀏覽頁（列表 + 搜尋/篩選）
- 加選 / 退選操作（含衝堂/額滿提示）
- 個人週課表視覺化（WeeklySchedule 元件）
- 歷年成績查詢頁
- Vitest 11 + Playwright E2E 7 自動化測試
- `_doc/v0.2.md` 版本說明

---

### v0.3 — Teacher & Admin Frontend ✅

> 教師端 + 管理員端介面

**已完成（教師端）：**
- 授課清單頁 `TeacherCourses.tsx`
- 成績登錄頁 `TeacherGradeEntry.tsx` + `RosterTable.tsx`（表格填寫、批次送出）
- 成績鎖定送交（含確認對話框）
- 教師名冊 API `GET /teachers/me/courses/{id}/students`

**已完成（管理員端）：**
- 帳號管理頁 `AdminUsers.tsx`（新增 / 列表 / 搜尋）+ `UserForm.tsx`
- 課程管理頁 `AdminCourses.tsx`（開課）+ `CourseForm.tsx`
- Admin 輔助 API：`GET /admin/users`、`/admin/teachers`、`/admin/departments`

**已完成（測試）：**
- 後端 `test.sh` 33 → 48（名冊 / 管理列表 / 開課 / 權限）
- Vitest 24（新增 RosterTable / UserForm / CourseForm）
- Playwright E2E 10（新增教師成績流程 + 管理員流程）
- `_doc/v0.3.md` 版本說明

**待 v0.5：** 開課時段（class_schedules）設定、選課時程管理（api: enrollment-period）

---

### v0.4 — Docker 部署

> 讓整個系統可部署在 Docker 上（前端 + 後端 + PostgreSQL），一鍵啟動

**功能：**
- 後端 `Dockerfile`：多階段建置（builder → runtime slim），release binary + healthcheck
- 前端 `Dockerfile` + `nginx.conf`：Vite 建置 → nginx 靜態服務 + `/api` 反向代理（同源，免 CORS）
- `docker-compose.yml`：`db`（PostgreSQL + pgdata volume）、`api`、`web`、`seed` 四服務，含 healthcheck 相依
- 設定調整：前端 API base 改由 `VITE_API_BASE` 設定；後端 CORS origin 走 `CORS_ORIGIN` 環境變數
- 部署驗證（`docker compose up` → seed → 全部功能可操作）
- `_doc/v0.4.md` 版本說明

**待 v0.5：** 開課時段（class_schedules）設定、選課時程管理（api: enrollment-period）

---

### v0.5 — Polish & Export

> 體驗優化、匯出功能、管理功能完善

**功能：**
- PDF 成績單匯出（學生端 + 教師端）
- 課表匯出（iCal / PDF）
- 加退選時程管理 API（admin 設定預選、正選、加退選期間）
- 前端 Error Handling + Loading states 統一
- README.md（安裝指南、API 文件、開發說明）

---

### v0.6 — Performance & Robustness

> 效能最佳化、壓力測試、安全性加強

**功能：**
- Redis 名額快取（加選高併發場景）
- 成績鎖定後解鎖 / rollback 機制
- API Rate Limiting
- 壓力測試腳本（加退選 500+ RPS）
- PostgreSQL 生產環境部署腳本
- SSO / OAuth2 整合評估

---

### Future

- Waiting List（遞補排隊機制）
- 跨系選課審核流程
- 停休課申請
- 行動端 APP（React Native）
- 校園 SSO / LDAP 整合

## 實作筆記

### MVP 進度 (Sprint 1 ~ 2)

#### 已完成功能

| 功能 | 狀態 | 說明 |
|------|------|------|
| JWT 認證 | ✅ | `bcrypt` hash (Rust `bcrypt` 0.17 + Python `bcrypt` 5.0) |
| RBAC 權限 | ✅ | `AuthUser` extractor, `FromRequestParts<AppState>` |
| 科系/使用者 API | ✅ | 建立使用者 API (admin only) |
| 開課 API | ✅ | 建立課程 API (admin only) |
| 課程查詢 | ✅ | GET /api/v1/courses (year, semester filter) |
| 加選 (Enroll) | ✅ | 衝堂檢查 + 名額檢查 + Transaction |
| 退選 (Drop) | ✅ | 更新 status='DROPPED' + enrolled_count - 1 + 刪除 grade |
| 學生課表 | ✅ | 按 day/start_period 排序 |
| 教師授課清單 | ✅ | Filter by teacher_id |
| 課程學生名冊 | ✅ | GET /teachers/me/courses/{id}/students（含成績/送交狀態） |
| 成績登錄 | ✅ | PUT /api/v1/grades/batch (期中/期末) |
| 成績鎖定 | ✅ | POST /api/v1/grades/submit (is_submitted=true) |
| 學生成績查詢 | ✅ | GET /api/v1/students/me/grades |
| 帳號管理 API | ✅ | GET/POST /api/v1/admin/users（列表 + 建立） |
| 管理輔助 API | ✅ | GET /api/v1/admin/teachers、/admin/departments |
| 教師/管理員前端 | ✅ | v0.3 三角色 React 介面（含 RosterTable/UserForm/CourseForm） |
| 假資料 Seed | ✅ | Python + bcrypt, 可複製 (--seed 42) |
| DB 抽換 | ✅ | SeaORM, SQLite/PostgreSQL 透過 DATABASE_URL 切換 |
| Migrations | ✅ | SeaORM Migration (6 tables, 3 indexes) |

#### 技術決策

1. **AuthUser 實作**: 使用 `impl FromRequestParts<AppState> for AuthUser` 直接從 `AppState` 讀取 `jwt_secret`，避免需要額外 middleware 將 secret 注入 extensions。
2. **Entity 產生**: 因 `sea-orm-cli` 在 SeaORM v2 有 API 變動，改為手動撰寫 entity 定義（含 Relations impl）。
3. **Migration**: `pk_auto` 從 `sea_orm_migration::schema` import; index 用 `manager.create_index()` 而非 `Table::create()` 內建。
4. **Seed 資料**: Server 啟動時自動跑 migration，再由外部 `sqlite3 dev.db < seed.sql` 插入假資料。
5. **假資料規模**: 5 科系, 10 教師, 30 學生, 2 管理員, 15 課程, ~25 時段, ~50 選課紀錄, ~50 成績

#### 使用方式

```bash
# 啟動 server (自動跑 migration)
cargo run

# 插入假資料
python3 scripts/seed.py -o scripts/seed.sql --seed 42
sqlite3 dev.db < scripts/seed.sql

# 測試 API
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"11303001","password":"student123"}'

# 加選
curl -X POST http://localhost:8080/api/v1/enrollments \
  -H "Authorization: Bearer $TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"course_id":5}'

# 成績登錄
curl -X PUT http://localhost:8080/api/v1/grades/batch \
  -H "Authorization: Bearer $TEACHER_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"course_id":1,"grades":[{"enrollment_id":1,"midterm_score":85,"final_score":92}]}'

# 成績鎖定
curl -X POST http://localhost:8080/api/v1/grades/submit \
  -H "Authorization: Bearer $TEACHER_TOKEN" \
  -H 'Content-Type: application/json' \
  -d '{"course_id":1}'
```

#### 已知限制

- Seed 資料在 server 啟動後插入，需確保同一 SQLite file
- `enrolled_count` 在加選/退選時更新，但未用 DB-level atomic update (改用 SeaORM model update)
- 成績鎖定後無法解鎖（正式系統需加入 rollback 機制）
- 未實作 admin/enrollment-period API (設定期末加退選時程) → v0.5
- 開課尚不支援時段（class_schedules）設定 → v0.5

## 效能目標

- 學生加退選操作：3 步驟內完成
- 選課高峰期：穩定處理 500+ RPS（Redis + PostgreSQL 條件更新）
- Rust 無 GC Pause，确保高併發下的低延遲回應
