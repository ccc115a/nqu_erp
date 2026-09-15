# 軟體工程與 AI 時代——以 NQU-ERP 校務系統為例

這是一本「用一個真實開源專案教軟體工程」的教科書。全書以 **NQU-ERP**（通用校務系統 MVP，Rust + React）作為唯一的貫穿案例：每一章的先驗知識都先講一般原理，再用這個專案的實際程式碼、文件、測試與歷史 commit 當作具體證據。

## 專案背景

NQU-ERP 是一個實作中的校務系統，聚焦「選課與成績管理」：

- **後端**：Rust + Axum + SeaORM，JWT 認證 + RBAC 三角色（學生 / 教師 / 管理員）
- **前端**：React + TypeScript + TailwindCSS，三角色介面
- **資料庫**：SQLite（開發）/ PostgreSQL（生產），靠 `DATABASE_URL` 切換
- **測試**：後端整合測試 `test.sh`（72 項）、前端 Vitest（26 項）＋ Playwright E2E（10 項）
- **版本演進**：v0.1 純後端 → v0.2 學生端 → v0.3 教師/管理端 → v0.3.1 管理員改刪功能

建議讀者先在本機跑起專案，邊讀邊看程式：

```bash
cargo run                 # 啟動後端（:8080，自動跑 migration）
python3 scripts/seed.py -o scripts/seed.sql --seed 42
sqlite3 dev.db < scripts/seed.sql
cd frontend && npm install && npm run dev   # :5173
```

帳號：學生 `11303001` / `student123`、教師 `T001` / `teacher123`、管理員 `admin` / `admin123`。

## 目錄

| 章節 | 主題 | 對應檔案 |
|------|------|----------|
| [01 — 軟體工程的本質](01.md) | 複雜性、Brooks 法則、AI 時代的角色轉變 | `src/handlers/enrollment.rs`、`src/main.rs` |
| [02 — 需求工程](02.md) | 需求挖掘、Use Case、Edge Cases、Spec-Driven | `_doc/plan.md`、`src/handlers/enrollment.rs` |
| [03 — 系統設計](03.md) | 模組化、架構、API 設計、SOLID、高內聚低耦合 | `src/main.rs`、`src/middleware/auth.rs`、前端 `api/client.ts` |
| [04 — 程式設計與實作](04.md) | 程式品質、重構、設計模式、Agentic Coding、Code Review | 各 handler、`AGENTS.md` |
| [05 — 測試與品質保證](05.md) | 測試金字塔、TDD、整合/E2E、AI 生成測試、LLM-as-a-Judge、迴歸 | `test.sh`、`frontend/e2e/app.spec.ts` |
| [06 — 部署與維運](06.md) | CI/CD、DevOps 與 IaC、可觀測性、AI 自癒 | `run.sh`、`src/config.rs`、`src/db.rs` |

## 如何使用本書

- **授課**：每章可作 2–4 學分課的教材，章末附練習題（配合本專案實作）。
- **自學**：先跑起專案，讀每一章時在原始碼中搜尋文中引用的函式，例如在 `src/handlers/enrollment.rs` 找「衝堂檢查」。
- **實作延伸**：專案尚有 v0.4（選課時程、時段設定）等規劃，各章練習都設計成可直接接到專案的下一個功能。

## 各章地圖（原理 ↔ 本專案）

| 原理 | 在本專案的位置 |
|------|----------------|
| 本質性困難 | 衝堂檢查、名額原子扣減（`enrollment.rs`） |
| 需求規格化 | `_doc/plan.md`、遷移檔與 Entity |
| RBAC 橫切關注 | `middleware/auth.rs` 的 `AuthUser` extractor |
| API 一致性 | `errors.rs` 的 `AppError`、統一回傳格式 |
| 測試分層 | `test.sh`（整合）+ `frontend/__tests__`（單元）+ `e2e/`（端到端） |
| 可重現性 | `seed.py --seed 42`、`dev.db` 重置流程 |
| 可觀測性起點 | `tracing_subscriber`、`/health` 端點 |