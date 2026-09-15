// Playwright E2E 測試：涵蓋學生（登入／課程搜尋／加退選／課表／成績／登出）、
// 教師（成績登錄與鎖定送交）、管理員（帳號與課程的新增／修改／刪除）。
// 測試會起 Vite（:5173）與後端（:8080），並用 seed 資料驗證。
import { expect, test } from '@playwright/test'
import type { Page } from '@playwright/test'

const API = 'http://localhost:8080/api/v1'
const STUDENT = { username: '11303001', password: 'student123' }
const COURSE_NAME = '計算機網路'
const COURSE_ID = 3
const DUP_COURSE_ID = 8

// 每個測試前先到登入頁並清空 localStorage，避免殘留登入狀態
test.beforeEach(async ({ page }) => {
  await page.goto('/login')
  await page.evaluate(() => localStorage.clear())
})

// 登入公用函式：填學號／密碼並按下登入
async function login(page: Page, username: string, password: string) {
  await page.goto('/login')
  await page.getByPlaceholder('學號').fill(username)
  await page.getByPlaceholder('密碼').fill(password)
  await page.getByRole('button', { name: '登入' }).click()
}

// 依課程名稱取得課程卡片（.rounded-lg.border）
function courseCard(page: Page) {
  return page.locator('.rounded-lg.border', { hasText: COURSE_NAME })
}

test.describe('學生端 E2E', () => {
  test('未登入訪問 /courses 會被導向 /login', async ({ page }) => {
    await page.goto('/courses')
    await page.waitForURL('**/login')
    await expect(page.getByText('選課系統登入')).toBeVisible()
  })

  test('密碼錯誤顯示後端錯誤訊息', async ({ page }) => {
    await page.goto('/login')
    await page.getByPlaceholder('學號').fill(STUDENT.username)
    await page.getByPlaceholder('密碼').fill('wrong-password')
    await page.getByRole('button', { name: '登入' }).click()
    await expect(page.getByText('帳號或密碼錯誤')).toBeVisible()
  })

  test('教師帳號登入導向授課清單', async ({ page }) => {
    await login(page, 'T001', 'teacher123')
    await page.waitForURL('**/teacher/courses')
    await expect(page.getByRole('heading', { name: '我的授課清單' })).toBeVisible()
  })

  test('登入後看到課程清單並可搜尋', async ({ page }) => {
    await login(page, STUDENT.username, STUDENT.password)
    await page.waitForURL('**/courses')
    await expect(page.getByText('課程瀏覽')).toBeVisible()
    await expect(page.locator('.rounded-lg.border').first()).toBeVisible()

    await page.getByPlaceholder('搜尋課程名稱').fill(COURSE_NAME)
    await expect(page.getByText(COURSE_NAME)).toBeVisible()
    await expect(page.getByText('資料結構')).toHaveCount(0)
  })

  test('加選 → 課表與成績 → 退選完整流程', async ({ page }) => {
    await login(page, STUDENT.username, STUDENT.password)
    await page.waitForURL('**/courses')

    await page.getByPlaceholder('搜尋課程名稱').fill(COURSE_NAME)
    const card = courseCard(page)
    await expect(card).toBeVisible()

    // 加選後應出現「已選修」狀態（退選按鈕）
    await card.locator('button', { hasText: '加選' }).click()
    await expect(page.getByText('加選成功！')).toBeVisible()
    await expect(card.locator('button', { hasText: '退選' })).toBeVisible()

    // 我的課表應顯示該課程
    await page.getByRole('link', { name: '我的課表' }).click()
    await expect(page.getByText(COURSE_NAME)).toBeVisible()

    // 我的成績應顯示預設修課紀錄
    await page.getByRole('link', { name: '我的成績' }).click()
    await expect(page.getByText('我的成績')).toBeVisible()
    await expect(page.getByText('資料結構')).toBeVisible()

    // 回課程瀏覽並退選，還原測試前狀態
    await page.getByRole('link', { name: '課程瀏覽' }).click()
    await page.getByPlaceholder('搜尋課程名稱').fill(COURSE_NAME)
    await courseCard(page).locator('button', { hasText: '退選' }).click()
    await page.getByText('退選成功！').waitFor()
  })

  test('重複加選被後端拒絕', async ({ request }) => {
    const token = await studentToken(request)
    const headers = { Authorization: `Bearer ${token}` }

    // 第一次加選成功
    const first = await request.post(`${API}/enrollments`, { headers, data: { course_id: DUP_COURSE_ID } })
    expect((await first.json()).success).toBe(true)

    // 重複加選應被拒絕（後端回傳「您已選修過此課程」）
    const dup = await request.post(`${API}/enrollments`, { headers, data: { course_id: DUP_COURSE_ID } })
    expect((await dup.json()).success).toBe(false)
    expect((await dup.json()).message).toContain('您已選修過此課程')

    // 清理：退選還原初始狀態
    await request.delete(`${API}/enrollments`, { headers, data: { course_id: DUP_COURSE_ID } })
  })

  test('登出後回到登入頁', async ({ page }) => {
    await login(page, STUDENT.username, STUDENT.password)
    await page.waitForURL('**/courses')
    await page.getByRole('button', { name: '登出' }).click()
    await page.waitForURL('**/login')
    await expect(page.getByText('選課系統登入')).toBeVisible()
  })
})

test.describe('教師端 E2E', () => {
  test('登錄成績並鎖定送交', async ({ page, request }) => {
    await login(page, 'T001', 'teacher123')
    await page.waitForURL('**/teacher/courses')

    // 動態找一個尚未送交成績的學生當測試標的
    const target = await teacherGradeTarget(request)
    await page.goto(`/teacher/courses/${target.courseId}`)
    await expect(page.getByRole('heading', { name: '成績登錄' })).toBeVisible()

    // 填期中考成績並儲存
    const midInput = page.getByLabel(`期中考 ${target.studentNumber}`)
    await midInput.fill('88')

    await page.getByRole('button', { name: '儲存成績' }).click()
    await expect(page.getByText('成績登錄成功')).toBeVisible()

    // 鎖定送交後輸入框應被禁用
    await page.getByRole('button', { name: '鎖定送交' }).click()
    await expect(page.getByText('確認鎖定送交？')).toBeVisible()
    await page.getByRole('button', { name: '確認送交' }).click()
    await expect(page.getByText('成績已鎖定送交')).toBeVisible()
    await expect(page.getByLabel(`期中考 ${target.studentNumber}`)).toBeDisabled()
  })
})

test.describe('管理員端 E2E', () => {
  test('新增帳號 → 修改 → 刪除', async ({ page }) => {
    await login(page, 'admin', 'admin123')
    await page.waitForURL('**/admin/users')
    await expect(page.getByRole('heading', { name: '帳號管理' })).toBeVisible()
    await expect(page.getByRole('cell', { name: 'T001', exact: true })).toBeVisible()
    await expect(page.getByRole('cell', { name: '系統管理員' })).toBeVisible()

    // 填寫新帳號表單並建立
    await page.getByPlaceholder('學號 / 帳號').fill('S099')
    await page.getByPlaceholder('姓名', { exact: true }).fill('測試新生')
    await page.getByPlaceholder('密碼').fill('test1234')
    await page.getByPlaceholder('email').fill('s099@test.nqu.edu.tw')
    await page.getByRole('button', { name: '建立帳號' }).click()
    await expect(page.getByText('帳號建立成功')).toBeVisible()
    await expect(page.getByText('測試新生')).toBeVisible()

    // 編輯：學號不可改、姓名可改、密碼留空代表不修改
    const s099Row = () => page.getByRole('row', { name: /S099/ })
    await s099Row().getByRole('button', { name: '編輯' }).click()
    await expect(page.getByRole('button', { name: '儲存修改' })).toBeVisible()
    await expect(page.getByPlaceholder('學號 / 帳號')).toBeDisabled()
    await page.getByPlaceholder('姓名', { exact: true }).fill('測試改名')
    await page.getByPlaceholder('留空表示不修改').fill('newpass123')
    await page.getByRole('button', { name: '儲存修改' }).click()
    await expect(page.getByText('帳號更新成功')).toBeVisible()

    // 用改名後的姓名搜尋確認清單已更新
    await page.getByPlaceholder('搜尋帳號 / 姓名 / 角色').fill('測試改名')
    await expect(page.getByText('測試改名')).toBeVisible()

    // 刪除：Playwright 要在 dialog 事件中接受 window.confirm
    page.once('dialog', (dialog) => dialog.accept())
    await page.getByRole('row', { name: /測試改名/ }).getByRole('button', { name: '刪除' }).click()
    await expect(page.getByText('刪除成功')).toBeVisible()
    await expect(page.getByRole('row', { name: /測試改名/ })).toHaveCount(0)
  })

  test('開設新課程 → 修改 → 刪除', async ({ page }) => {
    await login(page, 'admin', 'admin123')
    await page.waitForURL('**/admin/users')
    await page.getByRole('link', { name: '課程管理' }).click()
    await expect(page.getByRole('heading', { name: '課程管理' })).toBeVisible()
    await expect(page.getByText('資料結構')).toBeVisible()

    // 開設新課程（學年學期固定 113-1）
    await page.getByPlaceholder('如 CSIE301').fill('CSIE505')
    await page.getByPlaceholder('課程名稱').fill('網路安全')
    await page.getByRole('button', { name: '建立課程' }).click()
    await expect(page.getByText('課程建立成功')).toBeVisible()
    await expect(page.getByText('網路安全')).toBeVisible()

    // 編輯課程名稱
    const courseCard = () => page.locator('.rounded-lg.border', { hasText: '網路安全' })
    await courseCard().getByRole('button', { name: '編輯' }).click()
    await expect(page.getByRole('button', { name: '儲存修改' })).toBeVisible()
    await page.getByPlaceholder('課程名稱').fill('網路安全改名')
    await page.getByRole('button', { name: '儲存修改' }).click()
    await expect(page.getByText('課程更新成功')).toBeVisible()
    await expect(page.getByText('網路安全改名')).toBeVisible()

    // 刪除課程並確認卡片消失
    page.once('dialog', (dialog) => dialog.accept())
    await page
      .locator('.rounded-lg.border', { hasText: '網路安全改名' })
      .getByRole('button', { name: '刪除' })
      .click()
    await expect(page.getByText('刪除成功')).toBeVisible()
    await expect(page.locator('.rounded-lg.border', { hasText: '網路安全改名' })).toHaveCount(0)
  })
})

// 依教師身分列出所有授課課程的名冊，找出第一個「未送交」的學生做為成績測試標的
async function teacherGradeTarget(request: import('@playwright/test').APIRequestContext) {
  const token = await teacherToken(request)
  const headers = { Authorization: `Bearer ${token}` }
  const courses = (await (await request.get(`${API}/teachers/me/courses`, { headers })).json()) as {
    course_id: number
  }[]

  for (const c of courses) {
    const roster = (await (
      await request.get(`${API}/teachers/me/courses/${c.course_id}/students`, { headers })
    ).json()) as Array<{ enrollment_id: number; student_number: string; is_submitted: boolean }>
    const target = roster.find((r) => !r.is_submitted)
    if (target) return { courseId: c.course_id, studentNumber: target.student_number }
  }
  throw new Error('no unsubmitted roster found in seed')
}

// 教師登入取得 token
async function teacherToken(request: import('@playwright/test').APIRequestContext) {
  const res = await request.post(`${API}/auth/login`, {
    data: { username: 'T001', password: 'teacher123' },
  })
  return (await res.json()).token
}

// 學生登入取得 token
async function studentToken(request: import('@playwright/test').APIRequestContext) {
  const res = await request.post(`${API}/auth/login`, {
    data: { username: STUDENT.username, password: STUDENT.password },
  })
  return (await res.json()).token
}