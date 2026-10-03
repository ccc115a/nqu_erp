# 工具篇——以 NQU-ERP 為例學會現代開發工具鏈

這是「用一個真實開源專案教工具」的教科書。全書以 **NQU-ERP**（通用校務系統 MVP，Rust + React）作為唯一的貫穿案例：每一章先講工具的一般原理，再用這個專案的實際設定檔、腳本、測試與提交歷史當作具體證據。

## 專案背景

NQU-ERP 是一個實作中的校務系統，聚焦「選課與成績管理」：

- **後端**：Rust + Axum + SeaORM，JWT 認證 + RBAC 三角色（學生 / 教師 / 管理員）
- **前端**：React + TypeScript + TailwindCSS，三角色介面
- **資料庫**：SQLite（開發）/ PostgreSQL（生產），靠 `DATABASE_URL` 切換
- **測試**：後端整合測試 `test.sh`（72 項）、前端 Vitest（26 項）＋ Playwright E2E（10 項）、Docker 整合測試 `docker_test.sh`（23 項）
- **版本演進**：v0.1 純後端 → v0.2 學生端 → v0.3 教師/管理端 → v0.3.1 管理員改刪功能 → v0.4 Docker → v0.5 CI/CD（GHCR）
- **協作方式**：git + GitHub 多人協作，OpenCode / AI Agent 全程參與開發

建議讀者先在本機把專案跑起來，邊讀邊對照檔案：

```bash
git clone git@github.com:ccc115a/nqu_erp.git
cd nqu_erp
cargo run                 # 啟動後端（:8080，自動跑 migration）
python3 scripts/seed.py -o scripts/seed.sql --seed 42
sqlite3 dev.db < scripts/seed.sql
cd frontend && npm install && npm run dev   # :5173
```

帳號：學生 `11303001` / `student123`、教師 `T001` / `teacher123`、管理員 `admin` / `admin123`。

## 目錄

| 章節 | 主題 | 對應檔案 |
|------|------|----------|
| [01 — git + GitHub](01-git.md) | 建專案、版本管理、多人協作、PR 流程 | `.gitignore`、`github.sh`、`git log` |
| [02 — OpenCode](02-opencode.md) | 如何結合 AI 做 Agentic 開發 | `AGENTS.md`、`_doc/*.md` |
| [03 — Rust + Cargo](03-rust.md) | 建置、依賴、單元測試詳解（含範例） | `Cargo.toml`、`src/handlers/enrollment.rs` |
| [04 — Node.js + npm](04-nodejs.md) | 腳本、依賴、Vitest 單元測試詳解（含範例） | `frontend/package.json`、`src/__tests__/` |
| [05 — Playwright](05-e2e.md) | E2E 測試詳解（含範例） | `frontend/e2e/app.spec.ts`、`playwright.config.ts` |
| [06 — Docker](06-docker.md) | 容器化、Compose、DevOps 實作 | `Dockerfile`、`docker-compose.yml`、`docker_run.sh` |
| [07 — GitHub Actions](07-github-action.md) | CI、CD、Release、部署 | `.github/workflows/ci.yml`、`cd.yml` |

## 如何使用本書

- **授課**：每章可作 1–2 週的工具實作課教材，章末附練習題（全部可在本專案上動手做）。
- **自學**：讀每一章時在原始碼中搜尋文中引用的檔案，例如在 `frontend/e2e/app.spec.ts` 找 `teacherGradeTarget`。
- **與設計篇的關係**：設計篇講「為什麼這樣設計軟體」，工具篇講「用什麼工具把設計落地」。兩篇共用同一個專案，章節可交叉引用（例如工具篇 03 的 Rust 單元測試，是設計篇 5.2 TDD 的動手版）。

## 各章地圖（工具 ↔ 本專案）

| 工具 | 在本專案的位置 |
|------|----------------|
| git 版本歷史 | `git log --oneline`（Initial commit → v0.5 CI/CD） |
| AI 協作規格 | `AGENTS.md`（品質關卡、E2E 陷阱、程式慣例） |
| Rust 工具鏈 | `cargo fmt / clippy / build / test` |
| Node 工具鏈 | `npm ci / run dev / run build / vitest run` |
| E2E | `npx playwright test`（10 項旅程測試） |
| 容器 | `docker compose up`（db + api + web + seed） |
| 自動化 | `.github/workflows/ci.yml`（3 jobs）+ `cd.yml`（推 GHCR） |
