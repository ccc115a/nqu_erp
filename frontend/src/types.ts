// 共用 TypeScript 型別：對應後端 API 的回傳／請求結構。

// 使用者角色
export type Role = 'STUDENT' | 'TEACHER' | 'ADMIN'

// 登入成功回傳內容（同時存入 localStorage）
export interface LoginResponse {
  token: string
  user_id: number
  username: string
  full_name: string
  role: Role
}

// 課程的一個上課時段
export interface ScheduleInfo {
  day_of_week: number
  start_period: number
  end_period: number
  location: string
}

// 課程（課程清單卡片使用的資料）
export interface Course {
  course_id: number
  course_code: string
  course_name: string
  teacher_id: number
  teacher_name: string
  credits: number
  capacity: number
  enrolled_count: number
  dept_id: number
  dept_name: string
  schedules: ScheduleInfo[]
}

// 週課表項目（一件事一個時段）
export interface ScheduleItem {
  course_code: string
  course_name: string
  teacher_name: string
  day_of_week: number
  start_period: number
  end_period: number
  location: string
}

// 學生個人成績
export interface Grade {
  course_code: string
  course_name: string
  credits: number
  midterm_score: number | null
  final_score: number | null
  total_score: number | null
  is_submitted: boolean
}

// 加退選等操作的統一回應
export interface ApiMessage {
  success: boolean
  message: string
}

// 教師成績登錄頁的名冊列
export interface RosterItem {
  enrollment_id: number
  student_id: number
  student_number: string
  full_name: string
  midterm_score: number | null
  final_score: number | null
  total_score: number | null
  is_submitted: boolean
}

// 管理員帳號列表列
export interface AdminUser {
  user_id: number
  username: string
  full_name: string
  role: Role
  dept_id: number
  dept_name: string
  email: string
}

// 開課表單的授課教師下拉選項
export interface TeacherOption {
  user_id: number
  username: string
  full_name: string
  dept_name: string
}

// 科系下拉選項
export interface Department {
  dept_id: number
  dept_code: string
  dept_name: string
}