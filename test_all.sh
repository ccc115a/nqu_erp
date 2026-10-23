set -x
cargo test --lib # 單元測試
cargo test --test system_tests # web api 測試
# 全量測試：依序執行後端整合測試與前端自動化測試（Vitest + Playwright E2E）。
bash test.sh # 案例測試

cd frontend
bash test.sh --headed
cd ..
