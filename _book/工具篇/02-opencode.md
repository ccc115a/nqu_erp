# 第二章　OpenCode：結合 AI 的開發方式

> 本章以 NQU-ERP 的 AI 協作現場為例：這個專案本身就是人 + AI 共同寫出來的。讀者應先讀過 `AGENTS.md`，本章把那份文件背後的「為什麼」講清楚。

---

## 2.1　OpenCode 是什麼：AI 從補全到代理

### 原理

AI 寫程式經歷三代：

| 代 | 形態 | 例子 | 人的工作 |
|----|------|------|----------|
| G1 補全 | 一次補幾行 | Copilot 行內建議 | 逐行接受/拒絕 |
| G2 對話 | 一次給一段 | ChatGPT 貼程式碼 | 複製貼上、自己驗證 |
| G3 代理（Agent） | **多步循環**：讀 repo → 改多檔 → 跑測試 → 看錯 → 再修 | **OpenCode** | 給方向、審查、決定 |

OpenCode 屬於 G3：它住在終端機裡，能直接讀檔、改檔、跑 `cargo` / `npm` / `docker`，並把結果回報給你。關鍵差別：**G2 的 AI 看不到你的 repo；G3 的 AI 和你在同一個工作目錄裡工作。**

### 本專案例子：OpenCode 在本專案的位置

本書寫作時使用的正是 OpenCode（`muse-spark` 模型），工作目錄就是 `/nqu_erp`。它能做的事包括：

- 讀 `_book/設計篇/*.md` 學會本書體例，再寫出風格一致的工具篇。
- 跑 `git log`、`cat Cargo.toml`、`ls frontend/src/__tests__` 查證事實，而不是憑空編。
- 按 `AGENTS.md` 的紀律寫程式：中文錯誤訊息、`AuthUser` 第一行鑑權、transaction 包多表寫入。

> **重點**：OpenCode 的價值不在「寫得快」，在「**在你的紀律裡寫**」。沒有紀律的 repo，Agent 寫出來的就是沒有紀律的程式——所以 2.2 的文件才是本章主角。

---

## 2.2　`AGENTS.md`：寫給 AI 的工程規格

### 原理

Agentic 開發的第一產物不是程式碼，是**給 AI 的規格文件**。人類容易「看一眼就懂」的默契（convention），AI 全部不懂，必須白紙黑字寫下來。寫作原則：**只寫「AI 第一次很可能猜錯」的事**，不寫常識。

### 本專案例子：逐節解說 `AGENTS.md`

| 節 | 內容 | 為什麼 AI 需要 |
|----|------|----------------|
| Build & Run | `cargo run` 自動跑 migration；前端 `:5173`；Docker 三指令 | AI 否則會自己發明啟動方式 |
| 品質關卡 | `cargo fmt && cargo clippy && cargo build && bash test.sh` | 沒有這行，AI 交回來的程式常常連格式都沒過 |
| Testing | 後端 72 / Vitest 26 / E2E 10；`cargo test` 目前為空 | AI 否則會把 `cargo test` 全綠當成「測完了」 |
| E2E 陷阱 | 退選不可重選要用獨立課程；`exact: true`；`dialog.accept()`；`Bearer ` 前綴 | **每一條都是 AI（和人）實測踩過的坑** |
| 環境變數 | `.env` 三行；切 Postgres 零改碼 | AI 否則會把密碼寫死進程式碼 |
| Code Conventions | `Result<Json<T>, AppError>`；中文訊息；`State(state)`；不加不必要註解 | AI 的預設風格與本專案不同，必須明說 |
| Git | 中文 commit；不 commit `dev.db`/secrets | AI 不說就會把 `dev.db` 一起 `git add -A` |

最經典的一條：

> `getByText` / `getByRole('cell')` 匹配不分大小寫：`T001` 會命中 `t001@nqu.edu.tw` → 加 `exact: true`

沒有這條，AI 寫的 E2E 會「本地看起來過、換資料就錯」。**寫 `AGENTS.md` 就是把「經驗」編譯成「AI 可執行的規格」**——這是人在 AI 時代最值錢的工作之一（設計篇 1.5、4.4）。

---

## 2.3　Agentic 開發循環：人機如何分工

### 原理

有效的循環只有七步，多一步都是浪費：

```
1. 人：把任務寫成「自包含的一小塊」（加一支 API、修一個 edge case）
2. 人：指出「要模仿的既有範例」（照 admin.rs 的 create_user 模式）
3. AI：讀範例 → 改程式 → 跑品質關卡 → 自我修正
4. AI：回報改了什麼 + 測試結果
5. 人：Code Review（設計篇 4.5 檢查表）
6. AI：更新文件（_doc、_book）
7. 人：commit（中文訊息）→ bash github.sh 推上 GitHub
```

### 本專案例子：一次真實的協作長什麼樣

假設任務是「管理員刪除課程」：

- **人下指令**：「照 `admin.rs` 的 `delete_user` 模式（防刪保護 + transaction 清關聯），新增 `delete_course`，錯誤用中文 `AppError`，並在 `test.sh` 補 404/400 斷言。」
- **AI 執行**：讀 `delete_user` → 寫 `delete_course` → `cargo fmt && cargo clippy && cargo build` → `bash test.sh` 看 72 項 → 若有紅燈，讀錯誤再修。
- **人審查**：刪除有選課紀錄的課程該擋嗎？DTO 有洩漏欄位嗎？有沒有順手改別的檔？（`git status --short` 一眼看出範圍）
- **AI 收尾**：寫 `_doc/v0.3.1.md` 的版本說明 → 人 `bash github.sh "管理員刪除課程"`。

三條鐵律（違反任何一條，協作品質立刻崩）：

1. **一次只交代一個自包含任務**——「全部做完」是對 AI 最貴的指令。
2. **給範例，不給抽象描述**——「照 X 的模式」比「寫一個優雅的 Y」快十倍。
3. **沒綠燈不 review**——測試沒過的程式碼不值得人類花時間看。

> **重點**：Agentic 循環能不能轉，取決於設計篇前三章的產物：規格精確（第 2 章）、模組低耦合（第 3 章）、品質關卡可執行（第 4 章）。**紀律是 AI 的跑道，沒有跑道，馬力越大越危險。**

---

## 2.4　AI 的工具箱：讀、改、跑、查

### 原理

OpenCode 這類 Agent 的能力就是四個動詞。理解它們，你就知道什麼任務適合派給 AI：

| 動詞 | 對應操作 | 適合任務 |
|------|----------|----------|
| 讀 | 讀檔、搜尋、看歷史 | 「找出所有回 403 的地方」「這個錯誤訊息在哪裡產生的」 |
| 改 | 精確編輯、新增檔案 | 「照既有模式加一支 API」「把這段迴圈抽成函式」 |
| 跑 | 跑測試、建置、腳本 | 「跑品質關卡並修到全綠」「重現這個 bug」 |
| 查 | 搜尋文件、上網查 | 「SeaORM v2 的 migration 寫法」「Playwright dialog 怎麼處理」 |

### 本專案例子：四個動詞的實際指令

```bash
# 讀：定位問題（人下指令，AI 執行搜尋）
「在 src/handlers/ 找出所有沒檢查 is_teacher() 就讀名冊的地方」

# 改：小步修改（一次一檔，可審查）
「把 course.rs 裡 list_courses 與 get_teacher_courses 共用的組裝抽成 build_course_items，只改這一檔」

# 跑：自我驗證（AI 跑完把結果貼回來）
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo build && bash test.sh

# 查：補知識（AI 先查再寫，避免編造 API）
「查 SeaORM v2 的 pk_auto 與 create_index 正確寫法，再寫 migration」
```

> **反模式**：「幫我把整個系統重構成微服務」（太大、不可驗證）；「修一下測試」（沒說哪一支、期望行為是什麼）。**任務越小、驗收越明確，AI 越可靠。**

---

## 2.5　人的新工作：規格、評審、決策

### 原理

AI 接手「打字」後，人的工作向三處集中（設計篇 1.5）：**寫規格的人、審查的人、拍板的人**。三者缺一，Agentic 開發就會產出「看起來對、實際錯」的程式。

### 本專案例子：對照本專案的三種工作

| 人的工作 | 本專案證據 |
|----------|------------|
| 寫規格 | `AGENTS.md`、每版 `_doc/vX.Y.md`、`test.sh` 的斷言句 |
| 審查 | 設計篇 4.5 檢查表（權限第一行？transaction？中文訊息？測試？範圍外修改？） |
| 拍板 | 是否接受 AI 的解法；`bash github.sh` 按下 push 的那一刻；正式機上 `bash docker_run.sh` 上線（第六章） |

特別注意「**假綠燈**」：AI 生成的測試有時「什麼都沒鎖住卻全綠」（設計篇 5.4）。審查測試時要問：「把實作改壞，這支測試會紅嗎？」不會紅的測試就是裝飾品。

> **重點**：AI 讓「產出」變便宜，讓「判斷」變貴重。本章的終極建議只有一句：**把你希望 AI 遵守的每一件事，寫成文件、腳本或測試**——寫不下來的，就還不是真正的紀律。

### 本章練習（跟做版）

> 以下每題格式皆為：目標 → 操作步驟 → 預期結果 → 觀察（學到什麼）。練習 2–5 需要一個 AI Agent（OpenCode 或任何能讀寫本 repo 的工具）。

#### 練習 1　讀規格：把一條 E2E 陷阱翻譯成人話

目標：體會「默契必須寫下來」，並理解 AI 為什麼需要它。

步驟：

```bash
grep -n "exact" AGENTS.md          # 找到那條 T001 的規格
grep -rn "exact: true" frontend/e2e/app.spec.ts | head -5
```

預期結果：第一個命令命中 `AGENTS.md` 的陷阱行；第二個命令看到 E2E 裡至少 3 處 `exact: true`（例如 `getByRole('cell', { name: 'T001', exact: true })`、`getByPlaceholder('姓名', { exact: true })`）。

接著動手驗證「沒有規格會怎樣」——打開瀏覽器思維實驗：管理員頁同時顯示 `T001`（帳號格）與 `t001@nqu.edu.tw`（email 格），`getByRole('cell', { name: 'T001' })`（不加 exact）在 Playwright 的大小寫不敏感匹配下會命中**兩個**格子 → 測試報 `strict mode violation` 失敗。

觀察：用自己的話寫下這條規格的「人話版」，例如：「Playwright 認字不認大小寫，查代號一定要加 `exact: true`，否則帳號會撞到 email」。**你剛剛做的，就是把經驗編譯成規格**——這是人在 AI 時代最值錢的工作（2.2）。

#### 練習 2　下指令：寫一條「給範例」的 AI 指令

目標：練習 2.3 鐵律第 2 條——給範例，不給抽象描述。

步驟：先讀範例，再寫指令：

```bash
grep -n "pub async fn create_user" src/handlers/admin.rs
sed -n '1,60p' src/handlers/admin.rs   # 看它的權限檢查、DTO、回傳形狀
```

然後把下面這條指令貼給你的 AI（OpenCode / 任何 Agent）：

```markdown
請照 `src/handlers/admin.rs` 中 `create_user` 的模式，
新增 `GET /api/v1/admin/users/{id}`（單筆查詢）：
1. 第一行用 `auth.is_admin()` 檢查，非管理員回 403（照既有中文訊息風格）
2. DTO 只回 username/full_name/email/role，不要洩漏 password_hash
3. 找不到回 404「使用者不存在」
4. 在 `src/main.rs` 加一行 `.route`，不要動其他路由
5. 完成後跑 `cargo fmt && cargo clippy --all-targets -- -D warnings && cargo build`
先不要寫測試，我 review 完再補。
```

預期結果：AI 讀完範例後，交回一個「長得很像 `create_user`」的函式。你預先寫下它可能犯的三個錯，再對答案：

| 你預測的錯 | 常見實際狀況 |
|------------|--------------|
| 忘了第一行鑑權 | AI 常把 `auth: AuthUser` 宣告了卻沒調 `is_admin()` |
| DTO 多帶了敏感欄位 | 直接把整個 `users::Model` 轉 JSON，含 `password_hash` |
| 路由動詞/路徑不對 | 用 `post` 而非 `get`，或路徑少了 `/api/v1` 前綴 |

觀察：**指令的品質決定產出的品質**。「照 X 的模式 + 五條約束」比「幫我加一個查詢 API」省掉至少兩輪來回。預測錯誤這件事本身，就是一次 Code Review 預演（2.5）。

#### 練習 3　審查實戰：用「改壞法」抓假綠燈

目標：學會設計篇 5.4 的「假綠燈偵測」——好測試的定義是「實作壞掉它會紅」。

步驟：

```bash
# 1. 請 AI 生成測試（spec-first：只給規格，不給實作）
```

把這段貼給 AI：

```
請為 DELETE /api/v1/enrollments 生成 3 條整合測試案例（bash + curl 形式，
仿照 test.sh 的 assert_contains 風格）。只參考這份規格，不要看實作：
- 成功：ENROLLED → DROPPED，enrolled_count - 1，成績紀錄刪除，回「退選成功！」
- 未選過的課 → 404「未找到選課紀錄」；非學生身分 → 403
每條寫明：準備狀態 → 動作 → 斷言。
```

```bash
# 2. 拿到案例後，執行「改壞實驗」（思想實驗即可，不用真改）：
#    假設把退選的 `enrolled_count - 1` 拿掉，AI 的測試會紅嗎？
# 3. 真實驗證（可選）：挑一條 AI 案例，實際打 curl 跑一次，看它測的是 status 還是中文 substring
```

預期結果：你會發現 AI 案例分兩種命運——斷言 `enrolled_count` 變化的會紅（好測試，鎖住副作用）；只斷言「回 200」的，拿掉 `-1` 照樣綠（**假綠燈**，什麼都沒鎖住）。

觀察：審查測試只有一個問題：「**把實作改壞，這支會紅嗎？**」不會紅的就是裝飾品。這個問題以後要變成你的肌肉記憶——每次 AI 交測試都問一次（2.5）。

#### 練習 4　補規格：把口頭提醒寫成紀律

目標：練習「經驗 → 文件」的轉換，這是 `AGENTS.md` 的生長方式。

步驟：

```bash
# 1. 回想你最近一次對 AI 說的話，例如：
#    「不要一次改三個檔案」「先跑測試再給我看」「不要用 unwrap」
# 2. 照 AGENTS.md 的格式，把它寫成一條：
```

```markdown
- **一次只動一個檔案**：除非我明說，否則每個回覆最多改一個來源檔，
  改完跑 `cargo build` 確認能編，再問我要不要繼續。
```

```bash
# 3. 把它加進 AGENTS.md（或先記在自己的筆記），下次開新任務時觀察 AI 是否遵守
```

預期結果：下一次你下指令時，AI 的回覆從「一次丟 5 個檔的 diff」變成「改一檔、貼 build 結果、問你」。

觀察：一條好紀律有三個特徵——**可執行**（能用命令驗證，如 `cargo build`）、**有範圍**（最多一檔）、**有觸發條件**（除非明說）。寫不出這三者的，就還不是紀律，只是願望（2.2、2.5）。

#### 練習 5　循環計時：完整走一次七步（15 分鐘小任務）

目標：用碼錶理解 Agentic 循環的時間分配，破除「AI 什麼都快」的迷思。

步驟：選一個真的小任務，例如「把 `GET /health` 的回傳從 `"OK"` 改成 JSON `{ "status": "OK" }`」：

```bash
# 人（計時開始）：寫規格 + 範例（2 分鐘）
#   「照 errors.rs 的成功回傳形狀 { success, message }，把 /health 改成 JSON，
#    並更新 test.sh 裡打 /health 的斷言」
# AI：實作 + 跑測試（記錄分鐘數）
# 人：review diff（`git diff --stat` 先看範圍，再看內容）
# AI：更新 _doc 或註解（若有）
# 人：bash github.sh "訊息"（或決定不 commit，先留著）
```

記錄表（範例，填你自己的數字）：

| 階段 | 花費 | 誰的時間 |
|------|------|----------|
| 寫規格 | 2 min | 人 |
| AI 實作 + 跑測試 | 4 min | AI + 機器 |
| 等 `cargo build` + `test.sh` | 3 min | 機器（人可做別的事） |
| 人 review | 5 min | 人 |
| 修來回一輪 | 3 min | 人 + AI |

觀察：多數人第一次計時會驚訝——**人（寫規格 + review）花的時間比 AI 多**。這正是 2.5 的結論：AI 讓產出便宜、判斷昂貴。循環轉得順的關鍵從來不是「AI 多快」，是「規格多清楚、測試多自動」。把這張表留著，三個月後再測一次，看哪一段變快了。
