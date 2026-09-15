// Vite 環境變數型別宣告：`import.meta.env` 的自訂欄位須在此宣告，`tsc` 才不會報錯。
interface ImportMetaEnv {
  // 後端 API base，缺省為 http://localhost:8080/api/v1（docker build 時設為 /api/v1）
  readonly VITE_API_BASE?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}