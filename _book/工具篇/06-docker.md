# 第六章　Docker：把環境打包的 DevOps

> 本章以 NQU-ERP v0.4 的容器化為例。讀者應先裝好 Docker（`docker --version`）。本章讀完，你會親手 `up` 起 db + api + web 三件套，並理解每一行的理由。

---

## 6.1　為什麼要 Docker：從「我這邊可以跑」到「哪裡都能跑」

### 原理

沒有容器時，部署 = 「在目標機器上重複我的手工步驟」：裝 Rust、裝 Node、設環境變數、建 DB……每一步都可能因版本差異爆炸。Docker 把「**作業系統層以下的全部**」打包進映像：同一個映像，在筆電、CI、正式機跑起來一模一樣。

對照 NQU-ERP 的三個階段：

| 階段 | 跑法 | 痛 |
|------|------|----|
| 本地開發 | `cargo run` + `npm run dev` + SQLite | 換台機器重裝一遍 |
| 裸機部署 | 目標機裝 toolchain 再 build | Rust 編譯慢、版本易漂 |
| 容器部署（v0.4） | `docker compose up` + Postgres | 目標機只需 Docker |

> **重點**：Docker 解決的不是「程式問題」，是「**環境問題**」。程式一行不用改（6.2 證明），環境從此可重現。

---

## 6.2　`Dockerfile`：後端映像怎麼煉成的

全文僅 33 行（完整見 `Dockerfile`），兩段式建置：

```dockerfile
# ---------- builder：編譯 release binary ----------
FROM rust:slim-bookworm AS builder
WORKDIR /app
COPY Cargo.toml Cargo.lock ./
COPY migrations/Cargo.toml ./migrations/
COPY migrations/src ./migrations/src
COPY src ./src
RUN cargo build --release

# ---------- runtime：只帶 binary + 系統憑證 ----------
FROM debian:bookworm-slim
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/nqu-erp /usr/local/bin/nqu-erp
EXPOSE 8080
HEALTHCHECK --interval=10s --timeout=3s --retries=5 \
    CMD curl -sf http://localhost:8080/health || exit 1
CMD ["nqu-erp"]
```

逐段解說：

1. **多階段（multi-stage）**：`builder` 含完整 Rust toolchain（~GB 級），`runtime` 只有 debian + binary（~百 MB 級）。成品映像不帶原始碼、不帶編譯器——**小、快、攻擊面小**。
2. **先複製 manifest 再複製 source**：`Cargo.toml`/`Cargo.lock` 先進，利用 Docker layer cache——依賴沒變時重建可跳過漫長的 crate 編譯。
3. **`HEALTHCHECK` 打 `/health`**：容器平台用它判斷 api 是否就緒（compose 的 `depends_on: service_healthy`、k8s 的 readiness 都是同一概念）。`/health` 回 `"OK"`（`src/main.rs`），是全書最小卻最重要的介面。
4. **設定全由環境注入**：`DATABASE_URL` / `JWT_SECRET` / `SERVER_ADDR` 在執行期給，不 Bake 進映像——同一個映像可跑開發、測試、正式（12-Factor，設計篇 6.1）。

前端另有一個 `frontend/Dockerfile`（nginx 同源部署）：`npm run build` 產靜態檔 → nginx 同時 serve 前端並反向代理 `/api` 到後端。瀏覽器只跟同一個源講話，CORS 問題直接消失。

---

## 6.3　`docker-compose.yml`：三服務 + 一次性 seed

全文結構（完整見 `docker-compose.yml`）：

```yaml
services:
  db:     # postgres:16-alpine，volume 持久化，pg_isready 健康檢查
  api:    # build: .（後端 Dockerfile），DATABASE_URL 指 postgres，不對外暴露
  web:    # build: ./frontend，ports: ["${WEB_PORT:-80}:80"]，依賴 api 健康
  seed:   # profiles: ["seed"]，psql 灌 scripts/seed.sql，一次性
volumes:
  pgdata: # DB 資料持久化：container 重建不丟資料
```

四個設計決定的理由：

| 決定 | 理由 |
|------|------|
| `seed` 掛 `profiles: ["seed"]` | 預設 `up` 不跑它，避免 migration 完成前搶灌資料；要用時 `docker compose --profile seed run --rm seed` |
| `api` 不映射 host port | 瀏覽器只走 `web` 的 nginx `/api` 代理；且本機 `test.sh` 佔 `:8080`，映射會打架（v0.4 實測踩過） |
| `depends_on: condition: service_healthy` | api 等 db 健康、web 等 api 健康——**啟動順序用健康檢查表達**，不用 `sleep 10` 賭博 |
| `WEB_PORT` 帶預設值 | `bash docker_run.sh 3000` → `3000:80`；80 被佔用時換 port 不用改檔案 |

程式零修改的證明：切 Postgres 只換 `DATABASE_URL`（`postgres://nqu:nqu@db:5432/nqu`），靠的是設計篇 3.1 的 SeaORM 抽象——**好的抽象讓容器化免費**。

---

## 6.4　DevOps 實作：`docker_test.sh` 與 `docker_run.sh`

### 驗證管線：`docker_test.sh`（23 項）

```bash
bash docker_test.sh          # 建置→啟動→seed→23 項整合測試；全過保留 stack 上線
bash docker_test.sh --down   # 收尾關閉 stack
```

它就是「容器版的 `test.sh`」：在 Postgres 真實環境重打 23 項 API，證明「SQLite 過的，在 Postgres 也過」。若某支只在 SQLite 過（例如方言差異），這裡會紅——**容器測試補的是「換 DB」這條風險鏈**。

### 維運入口：`docker_run.sh`

```bash
bash docker_run.sh              # 上線（預設 port 80）
bash docker_run.sh 3000         # 指定對外 WEB_PORT=3000 上線
bash docker_run.sh stop         # 停止，保留資料（volume 不動）
bash docker_run.sh restart      # 重啟
bash docker_run.sh logs [svc]   # 跟 log（可指定 db/api/web）
bash docker_run.sh status       # 看目前狀態
bash docker_run.sh down         # 停止並清空資料（-v；慎用）
```

核心是**單一 dispatcher**：子命令（stop/restart/logs/status/down）直接執行後 `exit`，上線路徑才讀 port。實戰教訓（設計篇 6.4）：不要寫「兩層獨立 case 都讀 `$1`」——`shift` 後 `$1` 已空，第二層會落空誤走上線路徑。

從 GHCR 上線（第七章 CD 的產物，正式機不用裝 Rust）：

```bash
echo "$GHCR_TOKEN" | docker login ghcr.io -u <你的帳號> --password-stdin
WEB_PORT=80 docker compose pull   # 拉 api / web 映像
bash docker_run.sh                 # 一鍵上線
curl http://localhost/api/v1/health
```

> **重點**：`docker_run.sh` 把「上線」變成一句話，`docker_test.sh` 把「驗證」變成一句話。DevOps 的起點就是**沒人需要記步驟，步驟全在版控裡**。

---

## 6.5　何時不該上 k8s（本專案的否決書）

NQU-ERP 只有 db + api + web 三服務、單 host、單 replica。常見「上 k8s 的理由」逐條打臉：

| 理由 | 為何不成立 |
|------|------------|
| 容器掛了要自動重啟 | healthcheck + `restart: unless-stopped` 已做到 |
| 要水平擴縮應付選課高峰 | 瓶頸是單一 Postgres——api 開 10 副本全打同一顆 DB，白費 |
| 滾動更新不破連線 | 唯一看似成立的一條，但 DB 遷移仍是另一回事；k8s 只對無狀態層有感 |
| 別人都上 k8s | 工具崇拜 |

真正該談 k8s 的判準（任一成立）：主機數 ≥ 2、瓶頸已移出 DB（read replica / cache 就緒）、需要跨機 HA、多套環境要同一套 declarative 管理。**現在一個都不成立，所以現在不用。** 未來若成立，CD 已把映像推上 GHCR（第七章）——「跑什麼」不變，只換「誰來跑」。

### 本章練習（跟做版）

> 以下每題格式皆為：目標 → 操作步驟 → 預期結果 → 觀察（學到什麼）。
> 前置：stack 已在跑（若沒有：`docker compose up -d`，等三個都 healthy）。以下輸出皆為本機實測。

#### 練習 1　起 stack：確認三服務健康 + 打通兩條路

目標：親手驗證「容器即環境」——三個服務互相認得，且只有一個對外入口。

步驟：

```bash
docker compose ps
docker compose exec api curl -sf http://localhost:8080/health   # 容器內直連
curl -s http://localhost/ | head -c 120; echo                    # 經 nginx 的前端
curl -s -X POST http://localhost/api/v1/auth/login \
  -H 'Content-Type: application/json' \
  -d '{"username":"11303001","password":"student123"}' | head -c 200; echo
```

預期結果：

```
NAME            IMAGE       SERVICE   STATUS
nqu_erp-api-1   nqu_erp-api api       Up … (healthy)
nqu_erp-db-1    …           db        Up … (healthy)
nqu_erp-web-1   nqu_erp-web web       Up … (healthy)

OK
<!doctype html>
<html lang="zh-Hant"> … <title>選課系統</title> …
{"token":"eyJ0eXAiOi…","role":"STUDENT",…}
```

觀察：

1. 三條路徑各有分工：`:8080/health` 是容器內健康檢查（`HEALTHCHECK` 用的就是它）；`/` 是 nginx 靜態頁；`/api/v1/*` 是 nginx 反向代理到 `api:8080`（`frontend/nginx.conf` 的 `location /api/`）。**瀏覽器永遠只跟 `web` 講話**——api 不映射 host port 是刻意的（6.3）。
2. 登入回了 JWT token——容器裡的後端連的是 Postgres（不是 SQLite），證明「換 DB 零改碼」（`DATABASE_URL` 注入，設計篇 3.1）。

#### 練習 2　喂資料：seed 的 profile 之謎 + 驗數

目標：理解一次性任務的設計，並驗證 seed 結果。

步驟：

```bash
docker compose --profile seed run --rm seed
docker compose exec db psql postgres://nqu:nqu@db:5432/nqu -c "SELECT count(*) FROM courses;"
```

預期結果：

```
 count
-------
    15
(1 row)
```

15 門課——和 `frontend/test.sh` 驗證的 `courses = 15` 是同一個數字（**同一個世界觀**，設計篇 2.6）。

觀察：為什麼 seed 要掛 `profiles: ["seed"]`？做個對照實驗：`docker compose config --services`（預設只列 db/api/web，沒有 seed）。若不掛 profile，每次 `up` 都會重灌 seed 把正式資料洗掉——**一次性任務必須預設不跑**，這是 6.3 的關鍵設計。`--rm` 則讓跑完的容器自動刪除，不留垃圾。

#### 練習 3　換 port：`WEB_PORT` 的展開規則 + 進容器看環境

目標：理解「同一個映像、不同設定」的 12-Factor 精神，並親進容器。

步驟：

```bash
docker compose exec -it db sh -c 'cat /etc/os-release | head -2'
docker compose exec api env | grep -E 'DATABASE_URL|SERVER_ADDR'
```

預期結果（本機實測）：

```
NAME="Alpine Linux"          # db：alpine（小）
ID=alpine
SERVER_ADDR=0.0.0.0:8080
DATABASE_URL=postgres://nqu:nqu@db:5432/nqu
```

```bash
# 換 port 上線（80 被佔用時的標準動作）
bash docker_run.sh 3000
curl -s http://localhost:3000/ | head -c 60; echo
bash docker_run.sh           # 換回預設 80
```

觀察：

1. `DATABASE_URL` 裡的主機名是 `db`（compose 服務名），不是 `localhost`——**容器間用服務名互相尋址**，這是 Docker 內建 DNS。若寫 `localhost`，api 會連到自己容器裡面，當然連不上（HW1 觀察題第 2 題的答案）。
2. `${WEB_PORT:-80}`：有給用給的，沒給用 80。`bash docker_run.sh 3000` 把 3000 傳進去，compose 映射成 `3000:80`——**port 是執行期決定，不是建置期寫死**。
3. api 是 Debian 12（bookworm），db 是 Alpine——`cat /etc/os-release` 是進容器後的第一個動作（HW1 第 3 節）。

#### 練習 4　驗證：跑 `docker_test.sh` 並數 23 項

目標：理解「容器測試補的是哪條風險鏈」。

步驟：

```bash
bash docker_test.sh 2>&1 | tail -30   # 建置→啟動→seed→23 項，約數分鐘
```

預期結果：結尾類似 `23 passed, 0 failed`（數字以實測為準），且測試打的是 `http://localhost/api/v1`（經 nginx，和瀏覽器同路徑——`docker_test.sh` 第 10 行註解有寫）。

接著數：`grep -c "assert" docker_test.sh` vs `test.sh` 的斷言數，對照兩份清單——重疊的是「SQLite 也測過的行為」（登入、加退選、權限），容器特有的是「換 Postgres 才測得到的」（連線、方言、經 nginx 的路徑）。

觀察：**23 項不是 72 項的重複，是「換 DB + 換網路路徑」這條風險鏈的專屬保險**。若某支在 SQLite 過、在 Postgres 紅（例如方言差異），只有這裡抓得到。這就是測試分層的精髓：每層保一條別層保不住的風險。

#### 練習 5　否決書：2000 RPS 先加副本還是先加 cache

目標：把 6.5 的否決書變成算術——數字會說話。

步驟（紙上計算 + 驗證）：

```bash
# 步驟 1：看單 DB 的連線上限與目前用量
docker compose exec db psql postgres://nqu:nqu@db:5432/nqu \
  -c "SHOW max_connections;"
# 步驟 2：看加選流程一次打幾次 DB（數 enrollment.rs 的 Entity::find）
grep -c "Entity::find\|\.all(\|\.one(" src/handlers/enrollment.rs
```

預期結果：`max_connections` 預設 100；一次加選約 4–5 次查詢（含查課表、查選課、查課程、寫入）。2000 RPS × 5 queries = 10000 QPS 打向單顆 Postgres——連線池先爆，DB CPU 其次。

計算：就算 api 開 10 副本，每個副本的請求最後全進同一顆 DB（**瓶頸不在 api 層**）。反之，在 `GET /courses` 這種讀多寫少路徑加 cache（或 Postgres read replica），直接砍掉 80% 以上的 DB 查詢。

觀察：結論寫成一句話——「**先加 DB cache（或 read replica），再談 api 副本；k8s 目前不解決瓶頸，只增加複雜度**」。這就是 6.5 判準表的用法：升級決策要有數字，不要有信仰。
