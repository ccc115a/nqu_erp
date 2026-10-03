# 第四章　Node.js + npm：前端建置與單元測試詳解

> 本章以 NQU-ERP 前端為例（`frontend/`）。讀者應先裝 Node 20（`node --version`）。前半講 npm 日常，後半把本專案 26 個 Vitest 測試拆開，一支一支講清楚。

---

## 4.1　npm：前端的 Cargo + 超市

### 原理

npm 做三件事：**下載依賴（install）、跑腳本（run）、鎖版本（lock）**。`package-lock.json` 是「可重現」的契約——沒有它，`npm install` 每次可能裝到不同版，CI 就會出現「本地過、雲端紅」的靈異事件。

### 本專案例子：`package.json` 腳本導覽

```json
{
  "scripts": {
    "dev": "vite",
    "build": "tsc --noEmit && vite build",
    "preview": "vite preview --port 5173",
    "test": "vitest run",
    "test:e2e": "playwright test"
  }
}
```

| 命令 | 作用 | 本專案何時用 |
|------|------|--------------|
| `npm install` / `npm ci` | 裝依賴（`ci` 照 lock 精確裝，CI 用它） | 剛 clone、CI 第一步 |
| `npm run dev` | Vite 開發伺服器（`:5173`，熱重載） | 日常寫畫面 |
| `npm run build` | `tsc` 型別檢查 + 打包（`frontend/test.sh` 會先跑它） | 上線前、測試前 |
| `npm test` | `vitest run`（26 單元測試，一次跑完即退出） | 寫完元件就跑 |
| `npm run test:e2e` | `playwright test`（第 5 章） | 旅程驗證 |

關鍵紀律（`AGENTS.md` + CI 對照）：

- CI 用 `npm ci` 不用 `npm install`——保證與 lock 一致。
- `npm run build` 的第一步是 `tsc --noEmit`：**型別錯誤直接擋下打包**，不要等到瀏覽器開了才發現。
- `node_modules/`、`dist/` 永不進版控（第一章 `.gitignore`）。

---

## 4.2　Vitest 測試環境：jsdom + Testing Library

### 原理

前端單元測試要在「沒有瀏覽器」的地方跑，就需要三件套：

| 件 | 本專案選擇 | 作用 |
|----|------------|------|
| 測試執行器 | **Vitest**（`vitest run`） | 發現測試、跑 `describe/it`、報結果；Vite 原生，速度快 |
| DOM 模擬 | **jsdom** | 在 Node 裡假裝有 `document`、`localStorage` |
| 元件渲染 | **@testing-library/react** | `render`、`screen`、`fireEvent`，用「使用者視角」查元素 |

設定檔是 `frontend/vite.config.ts`（本專案把 vitest 設定收斂在這裡，沒有獨立 `vitest.config.ts`）：

```ts
export default defineConfig({
  plugins: [react()],
  test: {
    environment: 'jsdom',
    globals: true,
    setupFiles: ['src/__tests__/setup.ts'],
    include: ['src/__tests__/**/*.{test,spec}.{ts,tsx}'],
  },
})
```

每一行都是踩過坑後的形狀：

- `environment: 'jsdom'`——沒有它，`render(<RosterTable/>)` 會因沒有 `document` 直接炸。
- `setupFiles: ['src/__tests__/setup.ts']`——載入 `@testing-library/jest-dom`，才有 `toBeInTheDocument()`、`toBeDisabled()`。
- `include: ['src/__tests__/**']`——**限定只收單元測試**，否則會把 `e2e/*.spec.ts` 也抓進來一起跑（`AGENTS.md` 明列的陷阱）。

跑法：

```bash
npx vitest run                 # 跑全部 26 個（CI 用法）
npx vitest run src/__tests__/client.test.ts   # 只跑一檔（開發時）
```

---

## 4.3　範例一：純函式測試（`client.test.ts` 全文解說）

被測函式 `getErrorMessage`（`src/api/client.ts`）：把 axios 錯誤翻譯成可顯示的中文。

```ts
import { AxiosError, AxiosHeaders } from 'axios'
import { describe, expect, it } from 'vitest'
import { getErrorMessage } from '../api/client'

describe('getErrorMessage', () => {
  it('extracts Chinese error message from backend response', () => {
    // 模擬後端回傳 { error: '中文訊息' } 的 AxiosError
    const err = new AxiosError(
      'Request failed',
      'ERR_BAD_REQUEST',
      { headers: new AxiosHeaders(), method: 'get' } as never,
      null,
      {
        status: 401,
        statusText: 'Unauthorized',
        headers: {},
        data: { error: '帳號或密碼錯誤' },
        config: {} as never,
      },
    )
    expect(getErrorMessage(err)).toBe('帳號或密碼錯誤')
  })

  it('falls back to a generic message when no payload', () => {
    const err = new AxiosError('Network Error', 'ERR_NETWORK')
    expect(getErrorMessage(err)).toBe('發生錯誤，請稍後再試')
  })

  it('falls back for non-axios errors', () => {
    expect(getErrorMessage(new Error('boom'))).toBe('發生錯誤，請稍後再試')
  })
})
```

逐支解說（這是讀測試的標準姿勢——**準備 → 動作 → 斷言**）：

| 測試 | 準備 | 斷言鎖住的行為 |
|------|------|----------------|
| 中文訊息透出 | 偽造帶 `{ error: '帳號或密碼錯誤' }` 的 401 | 後端訊息原樣顯示（設計篇 3.3 的 actionable 原則） |
| 無 payload 後備 | 純斷線 `ERR_NETWORK` | 不噴 `undefined`，顯示通用中文 |
| 非 axios 後備 | 原生 `Error('boom')` | 任何怪錯誤都不會炸掉 UI |

三支共保一條規格：**「UI 永遠有話可說，不會空白、不會噴英文堆疊」**。改壞 `getErrorMessage`（例如改成回 `err.message`），第一支立刻紅。

> **重點**：這就是前端版的「純函式優先測」。`getErrorMessage` 不碰網路、不碰 DOM，毫秒級可測——和第三章的 `is_overlapping` 是同一種動物。

---

## 4.4　範例二：元件測試（`RosterTable.test.tsx` 全文解說）

元件測試用「使用者眼睛」驗證：渲染出什麼、按了會怎樣。先準備假資料（一人已送交、一人未送交——**邊界對照組**）：

```tsx
const roster: RosterItem[] = [
  { enrollment_id: 1, student_number: '11303001', full_name: '陳小明',
    midterm_score: 80, final_score: 90, total_score: 86, is_submitted: true, ... },
  { enrollment_id: 2, student_number: '11303002', full_name: '林小華',
    midterm_score: null, final_score: null, total_score: null, is_submitted: false, ... },
]
```

**（1）渲染測試**：名冊出現名字與分數。

```tsx
it('renders student rows with scores', () => {
  render(<RosterTable roster={roster} />)
  expect(screen.getByText('11303001')).toBeInTheDocument()
  expect(screen.getByText('陳小明')).toBeInTheDocument()
  expect(screen.getByText('86.0')).toBeInTheDocument()
})
```

`getByText` 是「使用者找字」——若元件把學號藏進 `title` 屬性不顯示，這支就紅。**測的是「看得見」，不是「存不存在」。**

**（2）狀態測試**：已送交 / 未送交標籤與輸入鎖定。

```tsx
it('disables inputs for submitted rows', () => {
  render(<RosterTable roster={roster} />)
  expect(screen.getByLabelText('期中考 11303001')).toBeDisabled()
  expect(screen.getByLabelText('期中考 11303002')).not.toBeDisabled()
})
```

這支鎖住的是**成績鎖定規格**（設計篇 2.4）：送交後不可改。若有人把 `disabled` 條件寫反，這支立刻抓到。注意 `aria-label="期中考 {學號}"` 的命名約定——E2E（第 5 章）靠同一個 label 找輸入框，**單元與 E2E 共用同一套可測試性設計**。

**（3）互動測試**：輸入觸發草稿回呼（`vi.fn()` 當間諜）。

```tsx
it('emits draft entries on input change', () => {
  const onDraftChange = vi.fn()
  render(<RosterTable roster={roster} onDraftChange={onDraftChange} />)
  fireEvent.change(screen.getByLabelText('期中考 11303002'), { target: { value: '77' } })
  expect(onDraftChange).toHaveBeenCalled()
  expect(onDraftChange.mock.calls[0][0]).toEqual(
    [{ enrollment_id: 2, midterm_score: 77, final_score: null }])
})
```

`vi.fn()` 記錄「被呼叫了幾次、參數是什麼」——不測內部 state，只測**對外發出的事件形狀**。這是元件測試的黃金律：**測契約，不測實作**。

**（4）純函式再現**：`toDrafts`（草稿收集）是可獨立測的純函式，本專案直接export出來測：

```tsx
describe('toDrafts', () => {
  it('skips submitted rows and inactive blanks', () => {
    expect(toDrafts(roster, { '2:midterm': '77' })).toEqual(
      [{ enrollment_id: 2, midterm_score: 77, final_score: null }])
  })
  it('returns an empty array when nothing typed', () => {
    expect(toDrafts(roster, {})).toEqual([])
  })
})
```

同一個檔案裡，**元件行為 + 純函式**一起測——26 個 Vitest 就是這樣攢出來的（`UserForm` 編輯預填、`CourseForm`、`GradeTable`、`WeeklySchedule` 同理）。

---

## 4.5　何時寫哪種測試：前端決策表

| 情境 | 寫法 | 本專案例子 |
|------|------|------------|
| 工具函式、格式轉換、驗證 | 純函式 `it`（毫秒級，多寫） | `getErrorMessage`、`toDrafts` |
| 元件顯示邏輯 | `render` + `getBy*` 斷言可見性 | 名冊渲染、空狀態「目前沒有選課學生」 |
| 元件互動 | `fireEvent` + `vi.fn()` 斷言回呼 | 輸入成績發 draft、表單送出值 |
| 跨頁旅程 | 不要寫在 Vitest，去第 5 章 Playwright | 登入→加選→退選 |

> **重點**：Vitest 的敵人是「測實作細節」（例如斷言內部 `useState` 的值）。一換實作就全紅的測試，比沒測試更貴——重構（設計篇 4.2）時會被它綁死。永遠從使用者的角度斷言：**看得見什麼、按了發出什麼**。

### 本章練習（跟做版）

> 以下每題格式皆為：目標 → 操作步驟 → 預期結果 → 觀察（學到什麼）。
> 注意：練習 3–5 會新增檔案（練習用）。做完若不想留下來，刪掉新增的檔案即可（`git status --short` 確認乾淨）。

#### 練習 1　跑起來：26 個測試的戶口普查

目標：建立「測試庫存」觀念——先知道有什麼，再談改什麼。

步驟：

```bash
cd frontend
npx vitest run 2>&1 | tail -12
```

預期結果（本機實測，約 1.4 秒跑完）：

```
 ✓ src/__tests__/client.test.ts (3 tests) 2ms
 ✓ src/__tests__/GradeTable.test.tsx (4 tests) 34ms
 ✓ src/__tests__/RosterTable.test.tsx (7 tests) 60ms
 ✓ src/__tests__/WeeklySchedule.test.tsx (4 tests) 72ms
 ✓ src/__tests__/UserForm.test.tsx (4 tests) 119ms
 ✓ src/__tests__/CourseForm.test.tsx (4 tests) 153ms

 Test Files  6 passed (6)
      Tests  26 passed (26)
```

觀察：

1. 3+4+7+4+4+4 = 26，對得上。**以後每次跑完先對總數**：26 變 25 代表有人刪了測試（要問為什麼），27 代表有人新增（要看鎖了什麼）。
2. 注意耗時：純函式檔（`client` 2ms） vs 元件檔（`CourseForm` 153ms，含 render）。這就是 4.5 決策表的物理證據——**純函式測試便宜兩個數量級，所以多寫**。
3. 開發時只跑一檔：`npx vitest run src/__tests__/client.test.ts`（3 tests，瞬間完）。提交前才跑全部。

#### 練習 2　讀斷言：找出鎖住「密碼留空」的測試

目標：學會「從規格反查測試」——規格文件說的某句話，到底被哪支測試保護。

步驟：

```bash
# 步驟 1：先看規格怎麼說（_doc/v0.3.1.md 或 AGENTS.md 應有「密碼留空＝不變」）
grep -rn "留空" _doc/v0.3.1.md AGENTS.md frontend/src/__tests__/UserForm.test.tsx | head
# 步驟 2：看測試怎麼鎖
grep -n "it(\|expect" frontend/src/__tests__/UserForm.test.tsx | head -20
```

預期結果：你會找到一支類似「leaves password unchanged when blank」的測試：渲染編輯模式的 `UserForm`、密碼欄留空、按儲存、斷言送出的 payload **不含**新密碼（或 `password` 為空字串由後端忽略）。

接著做「改壞實驗」（思想實驗 + 可實際做）：把元件裡「密碼為空就不送」的條件拿掉 → 那支測試會紅，因為 payload 多了 `password: ''`，後端會把密碼清空——**這是 v0.3.1 最危險的迴歸**（設計篇 5.6：看不見的 bug，只有迴歸測試抓得到）。

觀察：**每條規格都該有一個測試當保鑣**。讀測試的新姿勢：先問「它在保護哪句規格」，再看程式碼。答不出來的測試，可能是假綠燈（見練習 5）。

#### 練習 3　新增純函式測試：前端版 `isOverlapping`（TDD）

目標：在前端完整走一次 TDD，並理解 `include` 設定為什麼重要。

步驟：

```bash
# 步驟 1：先建實作（故意寫錯，回傳 false）
mkdir -p src/lib
```

```ts
// src/lib/schedule.ts
export function isOverlapping(
  a: { day: number; start: number; end: number },
  b: { day: number; start: number; end: number },
): boolean {
  return false // 佔位
}
```

```ts
// src/__tests__/schedule.test.ts（注意：測試檔必須放在 __tests__ 下才會被跑）
import { describe, expect, it } from 'vitest'
import { isOverlapping } from '../lib/schedule'

describe('isOverlapping', () => {
  it('同一天且區間重疊 → true', () => {
    expect(isOverlapping({ day: 1, start: 6, end: 8 }, { day: 1, start: 3, end: 7 })).toBe(true)
  })
  it('同一天但完全不相交 → false', () => {
    expect(isOverlapping({ day: 1, start: 1, end: 2 }, { day: 1, start: 3, end: 4 })).toBe(false)
  })
  it('相接（3 節相接）→ true（<= 語意）', () => {
    expect(isOverlapping({ day: 1, start: 1, end: 3 }, { day: 1, start: 3, end: 4 })).toBe(true)
  })
  it('不同天 → false', () => {
    expect(isOverlapping({ day: 1, start: 6, end: 8 }, { day: 2, start: 6, end: 8 })).toBe(false)
  })
})
```

```bash
npx vitest run src/__tests__/schedule.test.ts 2>&1 | tail -6
# 預期：紅燈，1 passed（不交那支）3 failed
```

```bash
# 步驟 2：轉綠——把實作換成真正的規則
```

```ts
return a.day === b.day && a.start <= b.end && b.start <= a.end
```

```bash
npx vitest run src/__tests__/schedule.test.ts 2>&1 | tail -4
# 預期：4 passed，全綠
```

觀察：

1. 測試檔放錯位置（例如放 `src/lib/`）就不會被跑——因為 `vite.config.ts` 的 `include` 限定 `src/__tests__/**`。**設定即規格**：放錯地方的測試等於不存在。
2. 「相接算衝突」那支和第三章 Rust 版是同一條規格，前後端各鎖一次。若後端改 `<` 而前端仍 `<=`，兩邊測試會打架——這正是前後端一致性要有人管的原因（設計篇 3.3）。
3. 做完清理：`rm src/lib/schedule.ts src/__tests__/schedule.test.ts`（或留著當你的第一個貢獻，記得跑全套 `npx vitest run` 確認仍是 30 項全綠）。

#### 練習 4　間諜練習：`vi.fn()` 抓送出的 payload

目標：學會「測契約不測實作」——斷言元件對外發出的事件形狀。

步驟：先讀範例（`RosterTable.test.tsx` 的 `emits draft entries` 那支，4.4 已全文解說），再仿寫：

```tsx
// src/__tests__/CourseForm.spy.test.tsx（練習用，做完可刪）
import { describe, expect, it, vi } from 'vitest'
import { fireEvent, render, screen } from '@testing-library/react'
import CourseForm from '../components/CourseForm'

describe('CourseForm spy', () => {
  it('送出的 payload 含正確的課程名稱', () => {
    const onSubmit = vi.fn()
    render(<CourseForm onSubmit={onSubmit} />)
    // 依 CourseForm 實際欄位調整：填名稱 → 按建立 → 看間諜收到的第一個參數
    fireEvent.change(screen.getByPlaceholder('課程名稱'), { target: { value: '網路安全' } })
    fireEvent.click(screen.getByRole('button', { name: '建立課程' }))
    expect(onSubmit).toHaveBeenCalled()
    expect(onSubmit.mock.calls[0][0]).toMatchObject({ name: '網路安全' })
  })
})
```

```bash
npx vitest run src/__tests__/CourseForm.spy.test.tsx 2>&1 | tail -6
```

預期結果：有兩種可能，都是好教材——

- 全綠：你的 locator 與元件實際 props 對上了，間諜收到正確 payload。
- 紅燈（找不到元素 / props 名不同）：去讀 `CourseForm.tsx` 的 props 定義與 placeholder，把測試改對。**這個「改到對」的過程，就是在學元件的公開契約**。

觀察：`vi.fn()` 是間諜不是裁判——它只記錄「被叫幾次、參數是什麼」。斷言 `mock.calls[0][0]` 時用 `toMatchObject`（只關心關鍵欄位）而非 `toEqual`（全等），**測試才不會因多一個無關欄位就脆斷**。這是元件測試的耐久性技巧。

#### 練習 5　假綠燈偵測：寫一支爛測試

目標：親手寫出假綠燈，從此對它免疫（設計篇 5.4）。

步驟：新增一支「看起來有測」的測試：

```tsx
// src/__tests__/fake-green.test.tsx（練習用，做完必刪）
import { expect, it } from 'vitest'
import { render } from '@testing-library/react'
import RosterTable from '../components/RosterTable'

it('roster renders something', () => {
  const { container } = render(<RosterTable roster={[]} />)
  expect(container).not.toBeNull() // 永遠成立：render 成功 container 就不為 null
})
```

```bash
npx vitest run src/__tests__/fake-green.test.tsx 2>&1 | tail -4
# 預期：綠燈（1 passed）
```

接著做「改壞實驗」：把 `RosterTable` 的空狀態文字「此課程目前沒有選課學生」改成亂碼（改完記得還原！），再跑一次——**還是綠燈**。因為它只斷言 `container` 非空，UI 壞成怎樣它都不在乎。

```bash
rm src/__tests__/fake-green.test.tsx   # 必刪，不要讓它污染測試庫
```

觀察：假綠燈有三個特徵——**不斷言行為**（只斷言存在性）、**不依賴規格**（空狀態文字改了也不紅）、**永遠綠**（改壞實作照樣過）。以後 review AI 生成的測試（工具篇 02 練習 3），就拿這三條檢驗。記住手感：**綠燈不值得信任，會紅的綠燈才值得**。
