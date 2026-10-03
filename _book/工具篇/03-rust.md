# 第三章　Rust + Cargo：建置與單元測試詳解

> 本章以 NQU-ERP 後端為例。讀者應先裝好 Rust toolchain（`rustc --version` 有反應）。本章前半講 Cargo 日常，後半是全書最長的單元測試實作：把選課的核心規則抽成純函式，一條一條測起來。

---

## 3.1　Cargo：Rust 的建置中樞

### 原理

Cargo 同時是**依賴管理、建置、測試、格式化、靜態檢查**的入口。Node 世界把這些拆給 npm / tsc / eslint，Rust 全部收斂在 `cargo` 一個命令——這是 Rust 新手第一個要適應的心智模型。

### 本專案例子：`Cargo.toml` 與四個命令

`Cargo.toml` 是後端的依賴清單（節錄）：

```toml
[package]
name = "nqu-erp"
version = "0.1.0"
edition = "2021"

[dependencies]
axum = { version = "0.8", features = ["macros"] }
sea-orm = { version = "2", features = ["sqlx-sqlite", "sqlx-postgres", "runtime-tokio-rustls", "macros"] }
serde = { version = "1", features = ["derive"] }
jsonwebtoken = "9"
bcrypt = "0.17"
```

`AGENTS.md` 的品質關卡就是 Cargo 四連擊，順序不能亂：

```bash
cargo fmt        # 1. 格式化：先把排版統一，diff 才乾淨
cargo clippy     # 2. 靜態檢查：零 warnings 為目標（常見如 needless_return、多餘 clone）
cargo build      # 3. 編譯：確認整個 workspace 能編過
bash test.sh     # 4. 整合測試：72 項 API 斷言（第 5 章細講）
```

CI（第七章）跑的是更嚴格版：`cargo fmt --check`（格式不對就紅）與 `cargo clippy --all-targets -- -D warnings`（warning 當 error）。**本地先跑這四步，CI 就不會紅**——這是本專案新人第一課。

常用查詢命令：

```bash
cargo --version && rustc --version   # 確認 toolchain
cargo tree | head -30               # 看依賴樹（ sea-orm 拉了什麼）
cargo clean                         # 清 target/（磁碟爆了時）
```

---

## 3.2　單元測試的第一觀念：測什麼、不測什麼

### 原理

單元測試測的是**純邏輯**（輸入→輸出，無 IO、無 DB、無網路）。判準：**「這個測試需要起 server 或連 DB 嗎？」需要，就不是單元測試**，該去整合層（`test.sh`）。

對照 NQU-ERP：

| 程式 | 純邏輯？ | 該放哪層 |
|------|----------|----------|
| 衝堂判斷（區間重疊） | ✅ 是 | 單元測試（本章） |
| 成績是否及格、GPA 換算 | ✅ 是 | 單元測試 |
| `enroll_course` 整條流程（鑑權+查DB+寫入） | ❌ 否（碰 DB） | 整合測試 `test.sh` |
| JWT 簽發/驗證 | ⚠️ 半（純函式但涉密鑰） | 可單元測（用測試密鑰） |

本專案現況：`cargo test` **目前為空**（`AGENTS.md` 明載）。不是沒寫測試——72 項整合測試在 `test.sh` 裡。選擇的理由（設計篇 5.1）：handler 的價值在「真的跑在 DB 上」，mock 的收益 < 成本。**但純規則函式一旦被抽出來，就值得單元測**——下面三節示範怎麼做。

---

## 3.3　範例一：衝堂判斷（最經典的純函式）

### 步驟 0：把邏輯抽出來

目前衝堂寫在 `src/handlers/enrollment.rs` 的迴圈裡（碰 DB 查課表 + 迴圈比對混在一起）。重構第一步：把「比對」抽成獨立函式。新建 `src/schedule.rs`：

```rust
/// 兩時段是否衝突：同星期 且 區間 [start, end] 重疊。
/// 重疊條件：a.start <= b.end && b.start <= a.end
pub fn is_overlapping(a_day: i32, a_start: i32, a_end: i32,
                      b_day: i32, b_start: i32, b_end: i32) -> bool {
    a_day == b_day && a_start <= b_end && b_start <= a_end
}

/// 整張課表的衝突檢查：任一已選時段與目標時段重疊即 true。
pub fn has_conflict(
    current: &[(i32, i32, i32)],
    target: &[(i32, i32, i32)],
) -> bool {
    current.iter().any(|&(d1, s1, e1)| {
        target.iter().any(|&(d2, s2, e2)| {
            is_overlapping(d1, s1, e1, d2, s2, e2)
        })
    })
}
```

並在 `src/main.rs`（或 `lib` broadcast 位置）掛上 `mod schedule;`。原 handler 的三重迴圈改為呼叫 `has_conflict`——行為不變，`bash test.sh` 仍 72 全綠（重構安全網）。

### 步驟 1：寫測試模組（同檔附測試是 Rust 慣例）

在 `src/schedule.rs` 檔尾追加：

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_day_overlap_returns_true() {
        // 星期 1：第 6~8 節 vs 第 3~7 節 → 重疊 6~7
        assert!(is_overlapping(1, 6, 8, 1, 3, 7));
    }

    #[test]
    fn same_day_disjoint_returns_false() {
        // 第 1~2 節 vs 第 3~4 節 → 不相交
        assert!(!is_overlapping(1, 1, 2, 1, 3, 4));
    }

    #[test]
    fn different_day_returns_false_even_if_periods_match() {
        // 節次相同但星期不同 → 不衝突
        assert!(!is_overlapping(1, 6, 8, 2, 6, 8));
    }

    #[test]
    fn touching_boundary_counts_as_conflict() {
        // 語意決定：區間用 <= 比較，第 3 節相接算衝突。
        // 若未來改成 <（相接不算），此測試會紅，逼你有意識地改規格。
        assert!(is_overlapping(1, 1, 3, 1, 3, 4));
    }

    #[test]
    fn empty_schedule_never_conflicts() {
        assert!(!has_conflict(&[], &[(1, 6, 8)]));
        assert!(!has_conflict(&[(1, 6, 8)], &[]));
    }

    #[test]
    fn finds_conflict_among_many_slots() {
        let current = vec![(1, 1, 2), (3, 6, 8)];
        let target = vec![(2, 6, 8), (3, 7, 9)]; // 與 (3,6,8) 重疊 7~8
        assert!(has_conflict(&current, &target));
    }
}
```

### 步驟 2：跑起來

```bash
cargo test                    # 跑全部單元測試
cargo test schedule           # 只跑 schedule 模組
cargo test -- --nocapture     # 顯示 println!（debug 用）
```

全綠輸出長這樣：

```
running 6 tests
test schedule::tests::same_day_overlap_returns_true ... ok
test schedule::tests::touching_boundary_counts_as_conflict ... ok
...
test result: ok. 6 passed; 0 failed
```

> **重點**：`touching_boundary_counts_as_conflict` 是本章最重要的一支測試。它鎖住的不是「程式對不對」，是「**規格的語意決定**」（`<=` vs `<`）。半年後有人想改這行，測試會逼他先想清楚——這就是設計篇 2.6「規格要有可驗證判準」的 Rust 版。

---

## 3.4　範例二：成績規則與錯誤路徑

純函式不只一種。再看兩個本專案真實規則的測試寫法。

**（1）成績驗證**：假設抽出 `validate_score`（0~100 才能登錄）：

```rust
pub fn validate_score(score: f64) -> Result<f64, &'static str> {
    if !(0.0..=100.0).contains(&score) {
        return Err("成績必須在 0~100 之間");
    }
    Ok(score)
}

#[cfg(test)]
mod score_tests {
    use super::*;

    #[test]
    fn accepts_boundary_scores() {
        assert_eq!(validate_score(0.0), Ok(0.0));
        assert_eq!(validate_score(100.0), Ok(100.0));
    }

    #[test]
    fn rejects_out_of_range() {
        assert!(validate_score(-1.0).is_err());
        assert!(validate_score(100.5).is_err());
    }

    #[test]
    #[should_panic(expected = "必須在")]
    fn unwrap_invalid_score_panics_with_chinese_message() {
        validate_score(150.0).unwrap(); // 展示 should_panic：斷言「會炸」本身
    }
}
```

**（2）`Result` 型別測試**：Rust 測試函式本身可回傳 `Result`，用 `?` 寫更乾淨的多步驗證：

```rust
#[test]
fn chained_validation() -> Result<(), String> {
    let s = validate_score(88.0).map_err(|e| e.to_string())?;
    assert!((s - 88.0).abs() < f64::EPSILON);
    Ok(())
}
```

斷言工具箱（夠用 95% 場景）：

| 巨集 | 用途 | 例子 |
|------|------|------|
| `assert!(cond)` | 布林為真 | `assert!(has_conflict(&a, &b))` |
| `assert_eq!(a, b)` | 相等（含 `Ok`/`Err`） | `assert_eq!(validate_score(0.0), Ok(0.0))` |
| `assert_ne!(a, b)` | 不等 | 防迴歸：改壞的值不該出現 |
| `#[should_panic]` | 預期會 panic | 非法輸入 `.unwrap()` 必炸 |
| `#[ignore]` | 先跳過（重功能、慢測試） | `cargo test -- --ignored` 才跑 |

---

## 3.5　單元 vs 整合：何時停手

### 原理

測試也有邊際報酬。經驗法則：

1. **純函式**（衝堂、驗分、GPA）→ 單元測試，每個邊界一支。
2. **碰 DB/網路/時間** → 整合測試（`test.sh` 打真 API），不要 mock 到失去意義。
3. **UI 旅程**（登入→加選→課表）→ E2E（第 5 章），只保最重要的幾條。

### 本專案例子：三層如何分工

| 層 | 工具 | 本專案實例 | 數量 |
|----|------|------------|------|
| 單元 | `cargo test`（本章新建） | `is_overlapping`、`validate_score` | 6+ 起跳 |
| 整合 | `bash test.sh` | 重複加選 400、衝堂中文訊息、403 權限 | 72 |
| E2E | Playwright（第 5 章） | 加選→課表→退選旅程 | 10 |

工作流程建議：先寫單元（紅→綠→重構，設計篇 5.2），再跑整合確認沒改壞，最後 E2E 保旅程。提交前永遠是四連擊：

```bash
cargo fmt && cargo clippy --all-targets -- -D warnings && cargo build && cargo test && bash test.sh
```

> **重點**：單元測試的真正產物不是「覆蓋率數字」，是「**下次改程式的人敢動手**」。`touching_boundary_counts_as_conflict` 存在，重構就敢重構；不存在，每次改衝堂都是賭博。

### 本章練習（跟做版）

> 以下每題格式皆為：目標 → 操作步驟 → 預期結果 → 觀察（學到什麼）。
> 注意：練習 1–4 會新增 `src/schedule.rs`（練習用）。做完若不想留下來，`rm src/schedule.rs` 並把 `mod schedule;` 那行從 `src/main.rs` 拿掉，再 `cargo build` 確認還原。

#### 練習 1　動手抽函式：TDD 從紅燈開始

目標：親手走一次「紅 → 綠 → 重構」，並用 `bash test.sh` 證明重構沒改壞行為。

步驟：

```bash
# 步驟 0：先看現況——cargo test 是空的（AGENTS.md 明載）
cargo test 2>&1 | tail -4
```

預期結果（本機實測）：

```
running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 filtered out; finished in 0.00s
```

觀察：`ok` 配 `0 tests`——**「全綠」不等於「有測」**。這就是為什麼本書堅持數測試個數（72 / 26 / 10），而不是只看綠燈。

```bash
# 步驟 1：先寫測試（紅燈）——新建 src/schedule.rs，只放測試，不放實作
```

把下面存成 `src/schedule.rs`（故意先不寫函式本體，留一個錯的佔位）：

```rust
pub fn is_overlapping(a_day: i32, a_start: i32, a_end: i32,
                      b_day: i32, b_start: i32, b_end: i32) -> bool {
    false // 佔位：全部回 false，等測試來打臉
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_day_overlap_returns_true() {
        assert!(is_overlapping(1, 6, 8, 1, 3, 7));
    }
}
```

在 `src/main.rs` 加一行 `mod schedule;`（找其他 `mod` 宣告的位置），然後：

```bash
cargo test schedule 2>&1 | tail -6
```

預期結果：**紅燈**，且失敗訊息指名道姓：

```
test schedule::tests::same_day_overlap_returns_true ... FAILED
assertion failed: is_overlapping(1, 6, 8, 1, 3, 7)
test result: FAILED. 0 passed; 1 failed
```

```bash
# 步驟 2：寫實作（轉綠）——把佔位換成真正的規則（抄 enrollment.rs:92-94）
```

```rust
pub fn is_overlapping(a_day: i32, a_start: i32, a_end: i32,
                      b_day: i32, b_start: i32, b_end: i32) -> bool {
    a_day == b_day && a_start <= b_end && b_start <= a_end
}
```

```bash
cargo test schedule 2>&1 | tail -4
```

預期結果：**綠燈**：

```
test schedule::tests::same_day_overlap_returns_true ... ok
test result: ok. 1 passed; 0 failed
```

```bash
# 步驟 3：證明沒改壞——跑整合測試（後端需 :8080 空閒，約 1–2 分鐘）
bash test.sh 2>&1 | tail -5
```

觀察：你剛才完整經歷了 TDD 三步（紅→綠→重構），以及重構的安全網（`test.sh` 72 項）。記住這個順序：**先有會失敗的測試，才有資格寫實作**。`cargo test schedule` 只跑該模組（開發時省時間），`bash test.sh` 跑全部（提交前求安心）——兩層各有分工。

#### 練習 2　補邊界：「只有第三段衝突」的測試

目標：體會「邊界案例必須揪出來」，並學會 `has_conflict` 這種組合函式的測法。

步驟：在 `src/schedule.rs` 追加實作與測試：

```rust
pub fn has_conflict(
    current: &[(i32, i32, i32)],
    target: &[(i32, i32, i32)],
) -> bool {
    current.iter().any(|&(d1, s1, e1)| {
        target.iter().any(|&(d2, s2, e2)| {
            is_overlapping(d1, s1, e1, d2, s2, e2)
        })
    })
}
```

```rust
#[test]
fn only_third_slot_conflicts() {
    // 已選三段：星期 1 第 1~2 節、星期 2 第 6~8 節、星期 3 第 6~8 節
    let current = vec![(1, 1, 2), (2, 6, 8), (3, 6, 8)];
    // 目標：星期 3 第 7~9 節 → 只跟第三段重疊（7~8）
    assert!(has_conflict(&current, &[(3, 7, 9)]));
    // 目標：星期 4 第 7~9 節 → 全不相干
    assert!(!has_conflict(&current, &[(4, 7, 9)]));
}

#[test]
fn empty_schedule_never_conflicts() {
    assert!(!has_conflict(&[], &[(1, 6, 8)]));
    assert!(!has_conflict(&[(1, 6, 8)], &[]));
}
```

```bash
cargo test schedule 2>&1 | tail -8
```

預期結果：全部 `ok`，含練習 1 的舊測試（共 4 項）。然後故意把實作改壞驗證測試有效：把 `<=` 改成 `<` 再跑一次——`touching_boundary_counts_as_conflict`（若你照 3.3 加了它）會紅，因為第 3 節相接不再算衝突。

觀察：**好測試的特徵是「改壞實作會紅」**。`empty_schedule` 那支鎖的是「空集合的語意」（`any` 在空集合回 false）；`only_third_slot` 鎖的是「多時段掃描不會漏」。每一支都在回答一個規格問題，而不是湊覆蓋率。

#### 練習 3　驗分擴充：`NaN` 是什麼鬼

目標：用測試發現「連規格都沒想過」的輸入，體會測試驅動規格。

步驟：在 `src/schedule.rs`（或另開 `src/score.rs`）加：

```rust
pub fn validate_score(score: f64) -> Result<f64, &'static str> {
    if !(0.0..=100.0).contains(&score) {
        return Err("成績必須在 0~100 之間");
    }
    Ok(score)
}
```

```bash
# 先預測，再驗證：NaN 會進 Ok 還是 Err？
```

```rust
#[test]
fn nan_is_rejected() {
    assert!(validate_score(f64::NAN).is_err());
}
```

```bash
cargo test nan 2>&1 | tail -4
```

預期結果：**綠燈**——`(0.0..=100.0).contains(&NAN)` 回 false（浮點比較中 `NaN` 與任何值比較皆 false，連 `NaN <= 100` 都是 false），所以進 `Err`。不寫這支測試，你永遠不會發現「`contains` 順手擋掉了 NaN」。

觀察：這支測試的價值不在「程式對」，在「**規格被補上**」——以後有人問「NaN 成績會怎樣」，答案不是「不知道」，是「有測試：擋下」。再想一步：`validate_score(f64::INFINITY)` 呢？也擋（`contains` 同理）。**測試是規格的活文件**（設計篇 2.6）。

#### 練習 4　GPA 函式：級距邊界全覆蓋

目標：練習「每個級距至少測三個點：下界、上界、界外一點」。

步驟：新增函式 + 測試：

```rust
pub fn grade_to_point(score: f64) -> f64 {
    if score >= 90.0 { 4.0 }
    else if score >= 80.0 { 3.0 }
    else if score >= 70.0 { 2.0 }
    else if score >= 60.0 { 1.0 }
    else { 0.0 }
}

#[cfg(test)]
mod gpa_tests {
    use super::*;

    #[test]
    fn boundaries() {
        assert_eq!(grade_to_point(100.0), 4.0);
        assert_eq!(grade_to_point(90.0), 4.0);   // 界上
        assert_eq!(grade_to_point(89.9), 3.0);   // 界下一點
        assert_eq!(grade_to_point(60.0), 1.0);
        assert_eq!(grade_to_point(59.9), 0.0);
        assert_eq!(grade_to_point(0.0), 0.0);
    }
}
```

```bash
cargo test gpa 2>&1 | tail -4
```

預期結果：全綠。若把 `>= 90.0` 誤寫成 `> 90.0`，`grade_to_point(90.0)` 掉到 3.0，測試立刻紅——**邊界值是 off-by-one 的照妖鏡**。

觀察：浮點斷言用 `assert_eq!` 要小心（`89.9` 這種字面量是精確可表示的才安全；算出來的分數改用 `(a - b).abs() < 1e-9`）。把這條寫進你的測試習慣：**字面量可 `eq`，計算值用 epsilon**。

#### 練習 5　停手判斷：`enroll_course` 該單元測嗎

目標：學會 3.5 的「停手」——不是所有程式都值得單元測。

步驟：打開 `src/handlers/enrollment.rs`，數它碰了哪些外部資源：

```bash
grep -n "Entity::find\|begin()\|commit()\|auth\." src/handlers/enrollment.rs | head -20
```

預期結果：你會數到至少 5 次 DB 查詢（`enrollments`、`class_schedules` ×2、`courses`、寫入）、1 次 transaction（`begin/commit`）、1 次 JWT 身分（`auth.user_id`）。要單元測它，得 mock DB + transaction + AuthUser——mock 的程式碼比本體還長，且 mock 的行為還是你自己編的（**測自己編的 mock 等於沒測**）。

觀察：決策表——純規則（衝堂、驗分、GPA）→ `cargo test`；碰 DB 的流程 → `test.sh` 72 項打真 DB；跨頁旅程 → Playwright。**測試策略是取捨，不是清單**（設計篇 5.1）。這題沒有要你寫程式，要你寫出判斷——這正是 AI 時代工程師的日常：決定「什麼值得測」比「寫出測試」更重要。
