# 第一章　git + GitHub：從建專案到多人協作

> 本章用 NQU-ERP 的真實倉庫（`git@github.com:ccc115a/nqu_erp.git`）作案例。讀者應先會 `git status / add / commit / push`，本章重點是「這些命令在一個真實專案裡怎麼被紀律化」。

---

## 1.1　如何創建專案：從零到可跑

### 原理

一個「可協作」的專案倉庫，創建時就要決定四件事：

1. **遠端位置**（GitHub repo）：誰能讀寫。
2. **忽略清單**（`.gitignore`）：什麼永遠不進版控。
3. **最小可跑骨架**：clone 下來後幾步能跑。
4. **提交起點**（Initial commit）：歷史的第一筆。

### 本專案例子：NQU-ERP 的創建軌跡

用 `git log` 倒帶，可以看到專案的出生證明（日期為作者實測環境）：

```
4f0dcad Initial commit
61a7d86 copy from _se115a
75b24e0 add _book/ and code comment
0d752a2 v0.4 docker
4ab0485 v0.5 CI/CD
```

第一步永遠是把本地與遠端連起來，本專案的遠端是：

```
origin  git@github.com:ccc115a/nqu_erp.git (fetch)
origin  git@github.com:ccc115a/nqu_erp.git (push)
```

標準創建流程（本專案當初就是這樣做的）：

```bash
# 1. GitHub 上開空 repo（例如 ccc115a/nqu_erp），不要勾 README
# 2. 本地初始化並綁定
git init
git remote add origin git@github.com:ccc115a/nqu_erp.git

# 3. 放進最小骨架：Cargo.toml + src/main.rs + .gitignore
# 4. 第一次提交並上推
git add -A
git commit -m "Initial commit"
git push -u origin main
```

> **重點**：創建專案不是「先寫很多程式再 commit」，而是「先讓空殼可被 clone、可被跑，再長肉」。NQU-ERP 的 `README.md` 從第一天就寫了 `cargo run` 與帳號，任何人 clone 下來都跑得起來——這就是協作的起點。

---

## 1.2　什麼該進版控、什麼不該：`.gitignore` 即規格

### 原理

版控只收「**人類寫的、可審查的**」東西；「機器產生的、可重現的」一律不收。判斷句：**「刪掉它，能否用一個命令重建？」能，就不該 commit。**

### 本專案例子：NQU-ERP 的 `.gitignore`

| 不收 | 原因 | 重建命令 |
|------|------|----------|
| `/target` | Rust 編譯產物，數百 MB | `cargo build` |
| `node_modules/`、`dist/` | npm 產物 | `npm ci` / `npm run build` |
| `dev.db` | 本地 SQLite，可重種 | `python3 scripts/seed.py … && sqlite3 dev.db < scripts/seed.sql` |
| `.env`、`.env.*` | 密碼與本機設定，外洩即事故 | 照 `AGENTS.md` 手寫三行 |
| `.DS_Store`、`*.log` | 系統/執行垃圾 | 不需重建 |

`AGENTS.md` 明文規定：「不要 commit `dev.db` 或任何 secrets」。而 `github.sh`（1.4 節）把這條規格**變成可執行的攔截器**——規格若只寫在文件，人總會忘；寫進腳本，機器幫你記。

---

## 1.3　版本管理：commit 即歷史敘事

### 原理

好的 commit 歷史是「**可讀的故事線**」：每個 commit 是一個邏輯單位，訊息說明「為什麼」。判準：**半年後的人只看 `git log --oneline`，能否講出專案演進史？**

### 本專案例子：NQU-ERP 的敘事線

```
Initial commit → copy from _se115a → add _book/ and code comment
  → v0.4 docker → v0.5 CI/CD → modify _book for docker/CI/CD
```

每一筆都是一個「版本里程碑」，且與 `_doc/vX.Y.md` 對齊：`v0.4 docker` 的 commit 裡有 `Dockerfile` + `docker-compose.yml` + `_doc/v0.4.md`。**程式碼與版本說明同一次進版控**，後人才能對帳。

本專案的 commit 紀律（見 `AGENTS.md`）：

- **中文訊息，簡述改動**（例如 `v0.5 CI/CD`、`add _book/ and code comment`）。
- 一個 commit 只做一件事；改功能與改文件若是同一版本，就一起進（版本契約）。
- 絕不 `--force` 推 main（會蓋掉別人的歷史）。

分支策略（小團隊務實版）：

```
main（永遠可跑，CI 全綠）
  └── feat/xxx（個人開 branch 做功能，推 PR 回 main）
```

PR 合併前檢查（對應第七章 CI）：`cargo fmt --check`、`clippy`、`bash test.sh` 全過才能 merge。**main 綠 = 隨時可部署**，這是下一章 CD 的前提。

---

## 1.4　一鍵上 GitHub：`github.sh` 的設計

### 原理

把「add → commit → push」的正確步驟寫成腳本，目的是**消滅兩種失誤**：推了不該推的檔、寫了無意義的訊息。

### 本專案例子：腳本逐段解說

```bash
# 用法：
bash github.sh               # 自動訊息：更新（vX.Y）：2026-09-15
bash github.sh "說明文字"     # 指定訊息
bash github.sh --dry-run     # 只列出將被 commit 的檔案
```

關鍵設計有三段，值得逐行讀（完整見 `github.sh`）：

**（1）前置檢查**：不在 repo、沒有 origin、找不到 git，立刻退出。

```bash
git rev-parse --is-inside-work-tree >/dev/null 2>&1 || { echo "❌ 不在 git repo 內"; exit 1; }
git remote get-url origin >/dev/null 2>&1 || { echo "❌ 沒有 origin remote"; exit 1; }
```

**（2）敏感檔攔截**：`.DS_Store`、`dev.db`、`.env`、`target/`、`node_modules/` 任一出現就擋下。

```bash
RISKY=$(printf '%s\n' "$STAGED_ALL" | grep -Ev '^D' | grep -E 'dev\.db|(^| )\.env( |$)|(^| )target/|node_modules/')
[ -n "$RISKY" ] && { echo "⚠️ 偵測到不該 push 的內容"; exit 1; }
```

注意 `grep -Ev '^D'`：已刪除（D）的檔放行，只攔「新增/修改」的風險檔——小細節，大體貼。

**（3）自動訊息**：沒給訊息時，從 `README.md` 抓版本號 + 日期。

```bash
VER=$(grep -m1 '版本 \*\*v' README.md 2>/dev/null | grep -o 'v[0-9.]*' | head -1)
MSG="更新（${VER:-}）：$(date +%Y-%m-%d)"
```

> **重點**：`github.sh` 不是「偷懶工具」，是**把團隊紀律變成程式**。新人第一天就會被它擋下 `dev.db`，等於上了一課 git hygiene。

---

## 1.5　多人協作：pull、衝突、PR

### 原理

多人協作的三個基本動作：**同步（pull --rebase）、解衝突（merge conflict）、審查（PR review）**。鐵律：**先同步再推；衝突在本地解完、測完再推。**

### 本專案例子：在 NQU-ERP 上演練

```bash
# 每天開工第一件事：把 main 拉到最新（rebase 保持線性歷史）
git pull --rebase origin main

# 開 branch 做功能
git checkout -b feat/enrollment-period
# …改 src/handlers/…、補 test.sh 斷言…
cargo fmt && cargo clippy && cargo build && bash test.sh
bash github.sh "新增選課時程 API"
git push -u origin feat/enrollment-period
# → 到 GitHub 開 PR，指回 main，等 CI（第七章）全綠 + 他人 review 後 merge
```

**衝突實例**：兩人同時改 `src/main.rs` 的 `Router`（一人加 `/admin/users/{id}`，一人加 `/admin/enrollment-period`），`git pull --rebase` 會停下並標記：

```
<<<<<<< HEAD
    .route("/api/v1/admin/users/{id}", get(handlers::admin::get_user))
=======
    .route("/api/v1/admin/enrollment-period", post(handlers::admin::set_period))
>>>>>>> feat/enrollment-period
```

解法：兩行都要留，存檔後 `git add src/main.rs && git rebase --continue`，再跑一次品質關卡確認能編能測。

PR 審查重點（呼應設計篇 4.5 的檢查表）：權限檢查第一行了嗎？多表寫入包 transaction 了嗎？錯誤是中文 `AppError` 嗎？測試補了嗎？有沒有範圍外順手改？**看不懂的 diff 不按 Approve**——這是協作的底線。

> **重點**：git 管的是「歷史」，GitHub 管的是「協作」。歷史要線性可讀，協作要「CI 先攔、人才審」。NQU-ERP 用小團隊流程（main + 短 branch + PR）換取最低溝通成本，這正是設計篇 1.3「用流程對抗人月陷阱」的工具版。

### 本章練習（跟做版）

> 以下每題格式皆為：目標 → 操作步驟 → 預期結果 → 觀察（學到什麼）。全部可在本機完成，不需 push。

#### 練習 1　讀歷史：v0.4 與 v0.5 各進了哪些檔案

目標：證明「commit 歷史是可讀的故事線」，學會 `git show --stat` 對帳「版本說明 vs 實際進版檔案」。

步驟：

```bash
git log --oneline -10
git show --stat 0d752a2 | head -25   # v0.4 docker
git show --stat 4ab0485              # v0.5 CI/CD
```

預期結果（本機實測）：

```
0d752a2 v0.4 docker
 Dockerfile            |  33 +++
 docker-compose.yml    |  72 ++++++
 docker_run.sh         | 100 +++++++++
 docker_test.sh        | 245 +++++++++++++++++++++
 _doc/v0.4.md          | 189 ++++++++++++++++
 ...
4ab0485 v0.5 CI/CD
 .github/workflows/cd.yml |  68 +++++++++++++++++++
 .github/workflows/ci.yml |  58 ++++++++++++++++
 _doc/v0.5.md             | 167 +++++++++++++++++++++++++++++++++++++++++++++++
 github.sh                |  48 ++++++++++++++
```

觀察：

1. v0.4 的程式碼（`Dockerfile`、`docker-compose.yml`）與版本說明（`_doc/v0.4.md`）在**同一個 commit**——這就是 1.3 的「版本契約」：後人對帳時，程式與文件永遠同捆。
2. 加分題：仔細看 v0.4 的檔案清單，裡面混進了 `dev.db`（Bin 90112 bytes）和 `.DS_Store`！這正是 1.2 說「不該進版控」的東西。對照 v0.5：`.DS_Store` 被刪掉、`github.sh`（含敏感檔攔截）登場。**連真實專案都會犯 hygiene 錯，重點是有沒有機制把它擋下來並修回去**——這就是練習 2 與練習 5 的動機。

#### 練習 2　攔截實驗：`github.sh` 如何擋下 `.env`

目標：親眼看到「紀律變成程式」的效果（安全、可重複做，不會真的 commit）。

步驟：

```bash
touch .env                       # 假裝不小心建了一個祕密檔
bash github.sh --dry-run         # 只列出、不寫入（安全模式）
echo "dry-run 退出碼：$?"
rm .env                          # 清掉假檔，還原現場
```

預期結果：`--dry-run` 只列出變更檔案就退出（退出碼 0），不會 commit、不會 push。若改跑不帶參數版（`.env` 還在時），會看到：

```
⚠️  偵測到以下暫存內容可能不該 push（secret / 二進位 / 垃圾）：
...
── 請先 git reset / 更新 .gitignore 後再跑 ──
```

且 `echo $?` 顯示退出碼非 0（腳本用 `exit 1` 擋下）。

觀察：攔截器用 `grep -E 'dev\.db|(^| )\.env( |$)|(^| )target/|node_modules/'` 匹配，且先 `grep -Ev '^D'` 放行「已刪除」的檔——**好的防線連「刪除祕密檔」這種無害動作都不誤殺**。軟體工程的紀律，寫成腳本才算數（設計篇 2.6：可執行的規格）。

#### 練習 3　衝突演練：兩條 branch 改同一行（全本地，不 push）

目標：走一次「分支 → 衝突 → 解衝突」的完整循環，理解 rebase 在做什麼。

步驟：

```bash
git checkout -b drill/alice
echo "alice 的版本" >> /tmp/drill.txt && cp /tmp/drill.txt CONFLICT_DEMO.txt
git add CONFLICT_DEMO.txt && git commit -m "alice 新增示範檔"
git checkout main
git checkout -b drill/bob
echo "bob 的版本" >> /tmp/drill2.txt 2>/dev/null; echo "bob 的版本" > CONFLICT_DEMO.txt
git add CONFLICT_DEMO.txt && git commit -m "bob 新增示範檔"
git rebase drill/alice     # 把 bob 接到 alice 後面 → 停住報衝突
```

預期結果：rebase 停下，`git status` 顯示 `both modified: CONFLICT_DEMO.txt`，檔案內容出現：

```
<<<<<<< HEAD
alice 的版本
=======
bob 的版本
>>>>>>> bob 的版本...
```

解衝突並收尾：

```bash
echo "合併後的版本（alice + bob）" > CONFLICT_DEMO.txt
git add CONFLICT_DEMO.txt
git rebase --continue
git log --oneline -3          # 看到 alice、bob 排成一直線
git checkout main
git branch -D drill/alice drill/bob
rm -f CONFLICT_DEMO.txt /tmp/drill.txt /tmp/drill2.txt
```

觀察：`<<<<<<<` / `=======` / `>>>>>>>` 三段分別是「我的 / 分隔線 / 對方的」。rebase 解完後歷史是**線性**的（沒有 merge bubble）——這就是 1.5「先同步再推、衝突本地解完再推」的日常版。記住手感：**衝突不可怕，可怕的是不看 diff 就 `checkout --theirs` 蓋掉別人的工作**。

#### 練習 4　PR 審查：用檢查表審一段「危險 diff」

目標：練習 1.5 的審查視角——看不懂的不按 Approve。

步驟：假裝收到這段 PR diff（把它存成 `/tmp/fake.diff` 閱讀，不用真的改程式）：

```diff
# 假 PR：「退選 API 支援已送交成績的課程」
-    if g.is_submitted {
-        return Err(AppError::forbidden("此課程成績已送交，無法修改"));
-    }
+    // 送交後也允許退選，方便學生
```

對照 1.5 檢查表逐條打勾，寫出 review 意見（範例）：

```markdown
## Review：Request changes
- [x] 權限：退選本身是學生操作，無角色問題
- [ ] 業務規則：拿掉送交鎖定後，「退選會刪除已送交成績」（見 enrollment.rs 退選副作用），
      等於開了一個抹掉正式成績的後門 → 必須擋下
- [ ] 測試：diff 沒附任何 test.sh / E2E 更新，無法證明行為
結論：Request changes。正確做法是回 400「成績已送交，不可退選，請洽教務處」，
並補一條 test.sh 斷言。
```

觀察：審查抓的永遠是同一幾類——**語意正確性、範圍控制、測試證據**。AI 時代 review 的對象不是「程式碼對不對」，是「規格與程式碼對不對得上」（設計篇 4.5）。

#### 練習 5　hygiene 檢查：`dev.db` 去哪了

目標：驗證 `.gitignore` 真的有作用，學會 `git check-ignore`。

步驟：

```bash
ls -la dev.db .env 2>&1            # dev.db 存在（~100KB），.env 可能不存在
git status --short | head          # 應該看不到 dev.db
git check-ignore -v dev.db .env   # 問 git：誰忽略了它們？
```

預期結果：

```
$ git status --short | head
?? "_book/工具篇/"                  # 只有未追蹤的新目錄，沒有 dev.db
$ git check-ignore -v dev.db .env
.gitignore:xx:dev.db   dev.db
.gitignore:xx:.env*    .env
```

（行號依你的 `.gitignore` 而定，重點是它回答了「哪一條規則」。）

觀察：`dev.db` 明明躺在工作目錄，`git status` 卻看不見——**忽略清單是「版控的規格」**（1.2）。若某天它真的出現在 status 裡（像 v0.4 那樣），正確處理是：`git rm --cached dev.db`（只從版控移除、不刪本地檔）+ 確認 `.gitignore` 有該條目。呼應練習 1 的加分題：歷史上的 hygiene 錯，就是這樣修回去的。
