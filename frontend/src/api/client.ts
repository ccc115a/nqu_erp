// Axios 設定：統一 Base URL、自動帶 JWT、401 自動登出。
import axios from 'axios'
import type { LoginResponse } from '../types'

// localStorage 的 key
export const TOKEN_KEY = 'nqu_token'
export const USER_KEY = 'nqu_user'

// 共用 axios 實例：後端 API 前綴（VITE_API_BASE 可在 .env 或 docker build ARG 設定，
// 缺省為本機 dev server 位址）
export const api = axios.create({
  baseURL: import.meta.env.VITE_API_BASE ?? 'http://localhost:8080/api/v1',
})

// 請求攔截：若有 token 自動加上 Authorization 標頭
api.interceptors.request.use((config) => {
  const token = localStorage.getItem(TOKEN_KEY)
  if (token) config.headers.Authorization = `Bearer ${token}`
  return config
})

// 回應攔截：401（token 過期/無效）時清除登入狀態並導回登入頁
api.interceptors.response.use(
  (res) => res,
  (err) => {
    if (err.response?.status === 401 && !window.location.pathname.startsWith('/login')) {
      logout()
      window.location.href = '/login'
    }
    return Promise.reject(err)
  },
)

// 從 Axios 錯誤中取出後端的中文錯誤訊息，取不到時回傳通用訊息
export function getErrorMessage(err: unknown): string {
  if (axios.isAxiosError(err)) {
    const data = err.response?.data as { error?: string } | undefined
    return data?.error ?? '發生錯誤，請稍後再試'
  }
  return '發生錯誤，請稍後再試'
}

// 是否已登入（localStorage 有 token）
export function isLoggedIn(): boolean {
  return Boolean(localStorage.getItem(TOKEN_KEY))
}

// 讀取目前登入使用者資訊
export function getUser<T = LoginResponse>(): T | null {
  const raw = localStorage.getItem(USER_KEY)
  return raw ? (JSON.parse(raw) as T) : null
}

// 儲存登入使用者資訊
export function saveUser<T>(user: T): void {
  localStorage.setItem(USER_KEY, JSON.stringify(user))
}

// 登出：清除 token 與使用者資訊
export function logout(): void {
  localStorage.removeItem(TOKEN_KEY)
  localStorage.removeItem(USER_KEY)
}