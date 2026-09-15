// 依角色決定登入後的首頁路徑。
import type { Role } from './types'

export const ROLE_HOME: Record<Role, string> = {
  STUDENT: '/courses',
  TEACHER: '/teacher/courses',
  ADMIN: '/admin/users',
}