# 全量測試：依序執行後端整合測試與前端自動化測試（Vitest + Playwright E2E）。
bash test.sh

cd frontend
bash test.sh --headed
cd ..
