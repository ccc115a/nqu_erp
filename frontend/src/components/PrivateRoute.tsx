// 受保護路由：未登入導至登入頁；角色不符導至該角色首頁。
import { Navigate, Outlet } from 'react-router-dom'
import { getUser, isLoggedIn } from '../api/client'
import { ROLE_HOME } from '../roles'
import type { LoginResponse, Role } from '../types'

export default function PrivateRoute({ roles }: { roles?: Role[] }) {
  // 未登入 → 強制回登入頁
  if (!isLoggedIn()) return <Navigate to="/login" replace />

  // 指定了角色限制時，確認目前使用者角色符合
  if (roles) {
    const user = getUser<LoginResponse>()
    if (!user || !roles.includes(user.role)) {
      return <Navigate to={user ? ROLE_HOME[user.role] : '/login'} replace />
    }
  }

  // 通過身分檢查後渲染巢狀路由
  return <Outlet />
}