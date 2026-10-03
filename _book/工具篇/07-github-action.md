# 第七章　GitHub Actions：CI、CD 與 Release

> 本章以 `.github/workflows/ci.yml` 與 `cd.yml` 全文為教材。讀者應先有 GitHub repo 權限。本章讀完，你會看懂每一個 job 在擋什麼，並能親手發一個版本。

---

## 7.1　CI 是什麼：把品質關卡搬上雲

### 原理

CI（Continuous Integration）= **每次 push / PR 自動驗證**：build、lint、測試全過才准合併。它的價值在「**關卡**」——壞程式碼在離產生最近的地方被攔下，而不是上線後被使用者攔下。

設計篇 6.1 說過：NQU-ERP 的在地品質關卡（`cargo fmt && … && bash test.sh`）就是 CI 的本地版。CI 只是把它搬到乾淨的雲端 runner 重跑一遍，附帶一個本地做不到的保證：**「在別人機器上也過」**。

### 本專案例子：`ci.yml` 全文導讀

```yaml
name: ci

on:
  push:
    branches: [main]
  pull_request:

concurrency:
  group: ci-${{ github.ref }}
  cancel-in-progress: true
```

`on` 決定何時跑：推 main 與任何 PR 都跑。`concurrency` 是省錢設計：同一 branch 新 push 會取消還在跑的舊 CI——不用等兩輪，省分鐘數。

三個 job（完整見 `.github/workflows/ci.yml`）：

```yaml
jobs:
  backend:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
        with: { components: rustfmt, clippy }
      - run: cargo fmt --check
      - run: cargo clippy --all-targets -- -D warnings
      - run: cargo build
      - run: bash test.sh        # 整合測試 72 項

  frontend:
    runs-on: ubuntu-latest
    defaults: { run: { working-directory: frontend } }
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: 20, cache: npm,
                cache-dependency-path: frontend/package-lock.json }
      - run: npm ci              # 照 lock 精確安裝（不是 install）
      - run: npx vitest run      # 單元 26
      - run: npx playwright install --with-deps chromium
      - run: bash test.sh        # 含 Playwright E2E 10

  docker:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: bash docker_test.sh # 容器 23 項
```

| job | 在擋什麼 | 本地對應命令 |
|-----|----------|--------------|
| backend | 格式、lint、編譯、72 項 API | `cargo fmt && cargo clippy && cargo build && bash test.sh` |
| frontend | 依賴一致、Vitest、E2E | `npm ci && npx vitest run && bash test.sh` |
| docker | 容器化後仍全過（Postgres） | `bash docker_test.sh` |

CI 化時浮現的四件事（在地跑不會發現）：runner 是乾淨的（`:8080` 沒人佔用，正好符合測試假設）；`rm -f dev.db` 在 CI 即 immutable 語意；Playwright 瀏覽器要明示安裝；seed 的 Python 依賴（`faker`、`bcrypt`）必須先就緒——**CI 逼你把「我機器上有」變成「腳本裡有」**。

> **重點**：CI 紅了，先看是「哪個 job、哪一步」。`fmt --check` 紅 = 本地忘了 `cargo fmt`；`npm ci` 紅 = lock 與 package.json 脫鉤；E2E 紅 = 先看 artifact 截圖（失敗畫面），不要猜。

---

## 7.2　CD 是什麼：從「能跑」到「能上線」

### 原理

CD（Continuous Delivery/Deployment）= **通過 CI 的程式自動變成「可部署的產物」（映像）**。NQU-ERP 的 CD 不做自動 deploy（保留人按鈕，見 6.4），只負責「**備好映像**」——這是刻意的安全設計。

### 本專案例子：`cd.yml` 全文導讀

```yaml
name: cd

on:
  push:
    branches: [main]
    paths:   # 文件-only 的 push 不觸發建置，省分鐘數
      - 'src/**'
      - 'migrations/**'
      - 'Cargo.toml'
      - 'Cargo.lock'
      - 'Dockerfile'
      - 'frontend/**'
      - 'docker-compose.yml'
      - 'docker_run.sh'
      - 'scripts/seed.py'
```

`paths` 篩選是成本意識：只改 `_book/` 不用重建映像。然後：

```yaml
env:
  REGISTRY: ghcr.io
  IMAGE_NAME: ${{ github.repository }}   # ccc115a/nqu_erp

jobs:
  build-push:
    permissions: { contents: read, packages: write }
    steps:
      - uses: actions/checkout@v4
      - uses: docker/setup-buildx-action@v3
      - uses: docker/login-action@v3   # 用 secrets.GITHUB_TOKEN 登入 GHCR
        with: { registry: ghcr.io, username: ${{ github.actor }},
                password: ${{ secrets.GITHUB_TOKEN }} }
      - uses: docker/build-push-action@v5   # api 映像
        with:
          context: .
          file: Dockerfile
          push: true
          tags: |
            ghcr.io/ccc115a/nqu_erp-api:main
            ghcr.io/ccc115a/nqu_erp-api:${{ github.sha }}
          cache-from: type=gha
          cache-to: type=gha,mode=max
      # … web 映像同理（context: ./frontend）…
```

| 設計 | 理由 |
|------|------|
| 推 `ghcr.io`（GitHub Container Registry） | 與 repo 同權限體系，不用另申請 Docker Hub |
| 雙標籤 `:main` + `:${sha}` | `:main` 給人用（好記），`:sha` 給回滾用（精確） |
| GHA cache（`type=gha`） | Rust 編譯層跨次重用，建置從十幾分鐘降到幾分鐘 |
| `permissions: packages: write` | 最小權限：只給寫映像，不給改 repo 設定 |

正式機上線（人按鈕，呼應 6.4）：

```bash
echo "$GHCR_TOKEN" | docker login ghcr.io -u ccc115a --password-stdin
WEB_PORT=80 docker compose pull
bash docker_run.sh
curl http://localhost/api/v1/health
```

> **重點**：**CI 管「能不能合」，CD 管「有沒有映像可上」，人管「要不要上」**。三權分立，任何一環都不自動越權。

---

## 7.3　Release：給版本一個名字

### 原理

CI/CD 保證「每次 main 都可上」，Release 回答「**這次上了什麼**」。實務做法：git tag（`v0.5.0`）+ GitHub Release（變更筆記）+ 對應的映像標籤。三者同名，才能從「使用者回報的版本」一路追到「哪個 commit、哪個映像」。

### 本專案例子：在 NQU-ERP 發一個版本

```bash
# 1. 確認 CI 全綠（main  branch 的 ✓）
# 2. 打 tag（語意化版本：主.次.修）
git tag -a v0.5.0 -m "v0.5 CI/CD：ci.yml 三 job + cd.yml 推 GHCR"
git push origin v0.5.0

# 3. 到 GitHub → Releases → Draft from tag，貼上 _doc/v0.5.md 的重點：
#    新增 API、行為變更、已知限制（_doc 就是現成的 release notes 素材）
```

進階：加一個 `release.yml`，在 tag push 時自動把 `:sha` 映像再貼一個 `:v0.5.0` 標籤並建 Release。這樣「版本號」同時出現在 git tag、GHCR tag、Release 頁——**三處同名，追查零歧義**。

版本號紀律（配合第一章的 commit 敘事）：

| 變更類型 |  bump | 例子 |
|----------|-------|------|
| 相容小功能 | minor（0.4→0.5） | 新增 CI/CD |
| 相容修 bug | patch（0.3.0→0.3.1） | 管理員改刪 |
| 不相容 | major（0.x→1.0） | API 路徑大改（本專案尚未發生） |

---

## 7.4　祕密管理：什麼能進 repo、什麼不能

### 原理

CI/CD 跑在雲端，最容易出事的就是祕密（secret）。鐵律：**祕密只活在三處——本機 `.env`（gitignore）、CI 的 Secrets、正式機的 secret manager**。永遠不進 git、不進映像、不進 log。

### 本專案例子：對照表

| 祕密 | 本專案做法 | 反模式 |
|------|------------|--------|
| `JWT_SECRET` | compose 用 `${JWT_SECRET:-…預設…}`，正式機由環境注入；`AGENTS.md` 明令 `.env` gitignore | 寫死在 `config.rs` 並 commit |
| `POSTGRES_PASSWORD` | compose 註明「正式環境請改用 secret 管理」 | 寫真密碼進 `docker-compose.yml` |
| `GITHUB_TOKEN` | GitHub 自動提供，`cd.yml` 只拿它登入 GHCR，不印出來 | `echo ${{ secrets.X }}` 除錯後忘記刪 |
| `GHCR_TOKEN`（正式機） | 只存在目標主機環境變數 | 貼在文件、傳訊息 |

`github.sh` 的敏感檔攔截（第一章 1.4）是最後一道本地防線；CI 的 `permissions: packages: write` 是最小權限原則——**祕密管理不是一個設定，是層層防線**。

> **重點**：第七章的三句話總結——**CI 讓壞碼合不進來，CD 讓好碼隨時可上，Release 讓每次上線有名字**。三者全在本專案跑通，缺一就不算現代交付。

### 本章練習（跟做版）

> 以下每題格式皆為：目標 → 操作步驟 → 預期結果 → 觀察（學到什麼）。
> 說明：CI/CD 跑在 GitHub 雲端，練習 1–4 需 repo 寫入權限（push / PR / tag）；若沒有權限，用 fork 或只做標註「本機可做」的部分。練習 5 全本機可做。

#### 練習 1　讀 CI：三個 job 的耗時解剖

目標：看懂 CI 在「花時間保什麼」，建立成本意識（CI 分鐘數是要錢的）。

步驟（GitHub 網頁）： repo → Actions → 點最近一次 `ci` 執行 → 展開三個 job，看左側耗時：

| job | 預期耗時量級 | 時間花在哪 |
|-----|--------------|------------|
| backend | 最長（數分鐘） | `cargo build` 編譯 + `test.sh` 起 server 跑 72 項 |
| frontend | 中等 | `npm ci` 下載 + `vitest`（秒級）+ Playwright 裝瀏覽器 + E2E |
| docker | 中等偏長 | `docker build` 兩映像 + 23 項 |

步驟（本機可做，對照）：

```bash
cat .github/workflows/ci.yml | grep -E "name:|run:" | head -20
```

預期結果：你數得出 backend 4 步、frontend 5 步（含裝瀏覽器）、docker 1 步——和 7.1 的表對得上。

觀察：最慢的幾乎永遠是 backend（Rust 編譯）。這解釋了 `cd.yml` 為何用 GHA cache（`type=gha`）——**把「編譯層」跨次重用，是 CI 優化第一刀**。以後你設計管線，先問「哪一步最慢、能不能快取」，而不是加機器。

#### 練習 2　弄紅它：故意留一個格式錯誤，看 CI 在哪裡攔

目標：親眼看到關卡發揮作用——壞碼合不進來（7.1 的「關卡」哲學）。

步驟（開 branch 做，不要直接推 main）：

```bash
git checkout -b drill/red-ci
echo 'fn   badly_formatted( ){println!("hi");}' >> src/red_drill.rs
# 注意：還要在 main.rs 加 mod red_drill; 才會被 fmt 檢查到（或直接改亂一個現有檔的一行縮排）
cargo fmt --check 2>&1 | head -5; echo "退出碼：$?"
```

預期結果（本機先驗，推上去之前就知道 CI 會說什麼）：

```
Diff in ... at line ...
-    ...
+    ...
退出碼：1
```

```bash
git push -u origin drill/red-ci   # 開 PR，看 backend job 在「fmt」那步紅
# 看完後：關掉 PR，刪 branch，還原檔案
git checkout main && git branch -D drill/red-ci
rm -f src/red_drill.rs  # 若有動過現有檔：git checkout -- <該檔>
```

觀察：

1. CI 紅的位置精確到「fmt 這一步」——**關卡越細，修越快**（不用猜是測試還是編譯）。
2. 你在本機 `cargo fmt --check` 看到的，和 CI 看到的**是同一個命令**——這就是「在地品質關卡 = CI 的本地版」。本地先跑，CI 就不會紅；CI 紅了，本地重跑同一命令就能重現。**可重現性是 CI 的靈魂**。

#### 練習 3　paths 實驗：改文件不觸發 CD

目標：驗證 `paths` 篩選，理解「省分鐘數」的設計。

步驟：

```bash
# 本機驗證（不需 push）：看 cd.yml 的 paths 有沒有含 _book
grep -A 10 "paths:" .github/workflows/cd.yml
```

預期結果：

```yaml
paths:
  - 'src/**'
  - 'migrations/**'
  - 'Cargo.toml'
  - 'Cargo.lock'
  - 'Dockerfile'
  - 'frontend/**'
  - 'docker-compose.yml'
  - 'docker_run.sh'
  - 'scripts/seed.py'
```

沒有 `_book/**`、`_doc/**`、`*.md`。所以：只改 `_book/工具篇/` 推 main → `ci` 照跑（`on.push.branches: [main]` 無 paths 篩選，品質仍要保），`cd` **不跑**（Actions 頁看不到新的 cd 執行）。

觀察：這是成本與安全的雙贏——文件更新不重建映像（省十幾分鐘 Rust 編譯），但測試照跑（文件裡的命令若寫錯，E2E 照樣會抓？不會——這是文件測試的缺口，值得想一想：**誰來測文件？**答案：讀者，也就是你）。

#### 練習 4　發版本：打 tag + 寫 Release notes

目標：走一次「版本命名」儀式，理解 tag、映像標籤、Release 三者的對應。

步驟：

```bash
# 步驟 1：確認 main 是綠的（Actions 頁 ci 全 ✓）
# 步驟 2：打 tag（先打測試用的，不要污染正式版號）
git tag -a v0.5.0-drill -m "練習：模擬 v0.5 發版"
git push origin v0.5.0-drill
# 步驟 3：到 GitHub → Releases → Draft a new release → 選剛才的 tag，
#   標題寫 v0.5.0-drill，內文貼 _doc/v0.5.md 的「新增內容」段落
# 步驟 4（收尾，重要）：刪掉練習用的 tag 和 draft release
git push origin :refs/tags/v0.5.0-drill
git tag -d v0.5.0-drill
```

預期結果：Release 頁多了一個版本，內文是 `_doc/v0.5.md` 的重點——**版本說明文件就是現成的 Release 素材**（1.3 的版本契約在開花結果）。

接著回答三標籤分工（7.3）：

| 標籤 | 誰用 | 例子 |
|------|------|------|
| `:main` | 人（好記） | 正式機 `docker compose pull` 預設拉它 |
| `:${sha}` | 機器（精確） | 出事回滾到「那個 commit 建的映像」 |
| `:v0.5.0` |  release（里程碑） | 和 git tag、Release 頁三處同名，追查零歧義 |

觀察：**版本號是追查鏈的鑰匙**——使用者報「v0.5.0 有 bug」，你立刻知道是哪個 commit、哪個映像、哪份 notes。沒有 Release 紀律的團隊，on-call 時都在玩猜謎。

#### 練習 5　祕密盤點：預設值方便 vs 正式風險（全本機可做）

目標：親手數出祕密的每個藏身處，做出取捨判斷。

步驟：

```bash
grep -rn "nqu-erp-secret" --include="*.rs" --include="*.yml" --include="*.sh" . 2>/dev/null | grep -v target | head
echo "---- git 裡有沒有真祕密？ ----"
git log -p --all -S "nqu-erp-secret-key-change-in-production" --oneline | head -5
echo "---- .env 在版控裡嗎？ ----"
git ls-files | grep -E "^\.env" || echo ".env 不在版控（正確）"
git check-ignore -v .env
```

預期結果：

1. `nqu-erp-secret-key-change-in-production` 只出現在**預設值**位置（`config.rs` 的 fallback、`docker-compose.yml` 的 `${JWT_SECRET:-…}`）——它是佔位符，不是真祕密。
2. `.env` 不在版控（`git ls-files` 無輸出），且被 `.gitignore` 忽略。
3. `github.sh` 會攔截 `.env`（工具篇 01 練習 2 已驗證）。

論證題（寫下來）：預設值讓新人 `docker compose up` 一把就起（開發體驗），代價是「有人直接把預設搬上正式機，JWT 人人可偽造」。解法分層——開發：保留預設但加註「正式禁用」；正式：secret manager 注入 + 啟動時檢查「若是預設值就拒絕啟動」（fail-fast，設計篇 6.3）。**祕密管理不是一個設定，是層層防線**（7.4），而你剛才數的每一處，都是一層。
