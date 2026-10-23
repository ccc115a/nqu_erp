# NQU-ERP 測試報告

- **執行日期**：2026-10-23 11:00 ~ 11:05（本機）
- **執行方式**：`bash test_all.sh`（一鍵全量測試，依序跑完 5 個層次）
- **執行環境**：
  - macOS（Apple Silicon）
  - Rust / Cargo 1.94.0
  - Node.js v22.11.0、Vitest 2.1.9、Playwright
  - 後端：SQLite（每次測試跑前重置 + `seed.py --seed 42` 產出 15 門課）
  - 總計結果：**149 / 149 全數通過**，退出碼 0

## 測試架構總覽

`test_all.sh` 依序執行：

| # | 層次 | 指令 | 測點 | 結果 |
|---|------|------|-----:|------|
| 1 | 後端單元測試 | `cargo test --lib` | 15 | ✅ 15/15 |
| 2 | 後端系統測試（Web API） | `cargo test --test system_tests` | 26 | ✅ 26/26 |
| 3 | 後端 HTTP 整合測試 | `bash test.sh` | 72 | ✅ 72/72 |
| 4 | 前端單元測試 | `npm test`（Vitest，jsdom） | 26 | ✅ 26/26 |
| 5 | 前端 E2E | `npx playwright test` | 10 | ✅ 10/10（13.4s） |
| | **合計** | | **149** | **✅ 全過** |

---

## 1. 後端單元測試（Rust，`#[cfg(test)]`，0.00s）

測試目標為純邏輯與小型單元，不涉及網路：

### `handlers::enrollment`（時間衝突判定）
| 測試 | 驗證內容 | 結果 |
|------|----------|------|
| `same_day_overlapping_periods_conflict` | 同一天、節次重疊 / 全等 / 包含 → 衝突 | ✅ |
| `adjacent_periods_do_not_conflict` | 相鄰節次（1~2 vs 3~4）不衝突 | ✅ |
| `different_days_do_not_conflict` | 不同星期不衝突 | ✅ |

### `middleware::auth`（JWT 與角色）
| 測試 | 驗證內容 | 結果 |
|------|----------|------|
| `create_and_decode_token_roundtrip` | 簽發 → 解析 claims 一致 | ✅ |
| `decode_token_with_wrong_secret_fails` | 換 secret 驗證失敗 | ✅ |
| `decode_garbage_token_fails` | 垃圾 token 解析失敗 | ✅ |
| `expired_token_is_rejected` | 過期 token（`encode_with_duration(-24)` 重現過期）被拒 | ✅ |
| `encode_with_duration_sets_expiry` | 指定時數簽發，exp 在未來 | ✅ |
| `role_helpers` | `is_student/is_teacher/is_admin` 互斥判斷 | ✅ |

### `errors`（AppError 統一錯誤形狀）
| 測試 | 驗證內容 | 結果 |
|------|----------|------|
| `builder_sets_expected_status` | bad_request/unauthorized/forbidden/not_found/conflict/internal 對應 400/401/403/404/409/500 | ✅ |
| `into_response_wraps_error_in_json_body` | 回應 body 為 `{"error": "..."}` | ✅ |
| `db_error_becomes_internal_error` | `DbErr` → 500「資料庫錯誤」 | ✅ |
| `jwt_error_becomes_unauthorized` | JWT 錯誤 → 401，訊息以「JWT 錯誤」開頭 | ✅ |

### `config`（環境變數）
| 測試 | 驗證內容 | 結果 |
|------|----------|------|
| `from_env_uses_overridden_values` | 四個環境變數覆蓋生效 | ✅ |
| `from_env_falls_back_to_defaults` | 未設定時使用開發缺省值 | ✅ |

---

## 2. 後端系統測試（Web API，`--test system_tests`，0.88s）

以 in-process Router（`tower oneshot`）跑完整 HTTP 流程，每個測試用**一次性暫存 SQLite**（migration + seed 後即丟），互不污染：

### 認證與權限
| 測試 | 驗證內容 | 結果 |
|------|----------|------|
| `health_check_ok` | `GET /health` → 200 "OK" | ✅ |
| `login_success_returns_token_and_user_info` | 回傳 token / username / role | ✅ |
| `login_wrong_password_is_unauthorized` | 錯密碼 → 401「帳號或密碼錯誤」 | ✅ |
| `login_unknown_account_is_unauthorized` | 未知帳號 → 401 | ✅ |
| `request_without_token_is_unauthorized` | 無 Authorization → 401 | ✅ |
| `request_with_invalid_token_is_unauthorized` | 垃圾 token → 401 | ✅ |
| `teacher_cannot_access_admin_endpoints` | 教師打 admin API → 403 | ✅ |
| `teacher_cannot_enroll` | 教師選課 → 403「只有學生可以選課」 | ✅ |
| `student_cannot_enter_grade` | 學生登錄成績 → 403 | ✅ |

### 課程查詢
| 測試 | 驗證內容 | 結果 |
|------|----------|------|
| `list_courses_filters_by_year_and_semester` | 學年/學期篩選（113-1 有 4 門、113-2 為 0） | ✅ |
| `teacher_can_list_own_courses` | 教師只看到自己的課（TEST004） | ✅ |

### 加退選
| 測試 | 驗證內容 | 結果 |
|------|----------|------|
| `enroll_success_then_duplicate_fails` | 加選成功 → 課表出現 → 重複加選「已選修過此課程」 | ✅ |
| `enroll_into_conflicting_period_fails` | 週一 1~2 與 2~3 節 → 「課程時間衝突」 | ✅ |
| `drop_then_reenroll_is_rejected` | 退選成功 → 重選被拒（已退選課程，請聯繫教務處重新選課） | ✅ |
| `drop_without_enrollment_is_not_found` | 未選就退選 → 404「未找到選課紀錄」 | ✅ |
| `enroll_missing_course_is_not_found` | 加選不存在課程 → 404 | ✅ |
| `enroll_fails_when_capacity_is_full` | 名額 1 選滿 → 「課程名額已滿」 | ✅ |
| `schedule_only_contains_current_semester` | 課表只含 113-1 的課 | ✅ |

### 名冊與成績
| 測試 | 驗證內容 | 結果 |
|------|----------|------|
| `teacher_sees_roster_of_own_course_only` | 授課教師 200；非本人課程 403；學生 403 | ✅ |
| `grade_batch_locks_after_submit` | 登錄 → 總分 80×0.4+90×0.6=86 → 送交鎖定 → 再改 403 → 學生視角 `is_submitted=true` | ✅ |
| `grade_entry_must_belong_to_course` | 名冊紀錄屬於他課 → 400「選課紀錄不屬於此課程」 | ✅ |

### 管理端 CRUD
| 測試 | 驗證內容 | 結果 |
|------|----------|------|
| `admin_create_user_then_login` | 建帳號成功即可登入 | ✅ |
| `admin_update_user_password` | 改密碼後：新密碼 200、舊密碼 401、列表含新名稱/email/dept | ✅ |
| `admin_delete_user_guards` | 有選課學生 400、有課教師 400、刪自己 400、無紀錄學生刪除成功 | ✅ |
| `admin_course_crud_with_cascade` | 開課 → 改課 → 刪課級聯清選課紀錄（課表變空）；教師開課 403 | ✅ |
| `admin_list_teachers_and_departments` | 教師下拉 2 筆、科系 1 筆（CS） | ✅ |

---

## 3. 後端 HTTP 整合測試（`bash test.sh`，build → 啟動 server → seed → curl 全覆蓋）

72 項斷言全部通過，各區段：

| 區段 | 項數 | 結果 |
|------|-----:|------|
| [3] 健康檢查 | 1 | ✅ |
| [4] 登入測試（學生/教師/管理員錯密碼） | 8 | ✅ |
| [5] 課程查詢（含無 auth → 401） | 3 | ✅ |
| [6] 選課測試（加選/重複/教師禁選） | 5 | ✅ |
| [7] 課表查詢 | 2 | ✅ |
| [8] 教師授課清單 | 2 | ✅ |
| [9] 學生成績查詢 | 2 | ✅ |
| [10] 成績登錄 + 送交鎖定 | 5 | ✅ |
| [11] 退選測試 | 2 | ✅ |
| [12] 管理員功能（建號/建課/下拉清單） | 12 | ✅ |
| [12b] 管理員修改/刪除帳號（密碼改動驗證、防刪自己/有課教師/有選課學生） | 14 | ✅ |
| [12c] 管理員修改/刪除課程（修改學分、再刪 → 404） | 10 | ✅ |
| [13] 教師名冊查詢（本人/學生/非本人課程） | 6 | ✅ |
| **合計** | **72** | **✅ 72/72** |

---

## 4. 前端單元測試（Vitest，jsdom，1.02s）

| 測試檔 | 測點 | 結果 |
|--------|-----:|------|
| `src/__tests__/client.test.ts` | 3 | ✅ |
| `src/__tests__/GradeTable.test.tsx` | 4 | ✅ |
| `src/__tests__/WeeklySchedule.test.tsx` | 4 | ✅ |
| `src/__tests__/RosterTable.test.tsx` | 7 | ✅ |
| `src/__tests__/CourseForm.test.tsx` | 4 | ✅ |
| `src/__tests__/UserForm.test.tsx` | 4 | ✅ |
| **合計**（6 檔案） | **26** | **✅** |

---

## 5. 前端 E2E（Playwright，3 個分鏡 10 個測試，13.4s）

| 場景 | 測試 | 耗時 | 結果 |
|------|------|-----:|------|
| 學生端 E2E | 未登入訪問 /courses 會被導向 /login | 417ms | ✅ |
| 學生端 E2E | 密碼錯誤顯示後端錯誤訊息 | 1.1s | ✅ |
| 學生端 E2E | 教師帳號登入導向授課清單 | 1.1s | ✅ |
| 學生端 E2E | 登入後看到課程清單並可搜尋 | 1.1s | ✅ |
| 學生端 E2E | 加選 → 課表與成績 → 退選完整流程 | 1.4s | ✅ |
| 學生端 E2E | 重複加選被後端拒絕 | 1.0s | ✅ |
| 學生端 E2E | 登出後回到登入頁 | 1.2s | ✅ |
| 教師端 E2E | 登錄成績並鎖定送交 | 2.0s | ✅ |
| 管理員端 E2E | 新增帳號 → 修改 → 刪除 | 1.8s | ✅ |
| 管理員端 E2E | 開設新課程 → 修改 → 刪除 | 1.4s | ✅ |

---

## 已知注意事項（環境陷阱，非測試問題）

1. **port 8080 被外部 process 佔用**會導致整批 404（本次執行前有另一專案的 `serve.mjs` 佔住 8080，需先停掉）。`test.sh` 開頭會 `pkill -f target/debug/nqu-erp`，但不會清掉非本專案 process。
2. `test.sh` / 前端 `test.sh` 每次都會 **刪除並重置 `dev.db`**；跑完後建議 `git checkout dev.db scripts/seed.sql` 還原工作區。
3. E2E 依賴 seed 課數 = 15（`--seed 42`），seed 改動需同步前端測試斷言。
4. 時間衝突判定為**閉區間**：1~2 節與 3~4 節相鄰不算衝突，1~2 與 2~3 算衝突（`periods_conflict()` 單元測試已鎖住此語意）。
