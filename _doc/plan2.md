# NQU-ERP 2.x 主線　透過 v2「學 k8s 與分散式架構」（教學練習線）

> 一句話願景：**NQU-ERP 是一台展示用、不會上線的系統——但 v2 主線要認真把它當成「學 k8s 與分散式架構」的練習題**：把「要撐住超多人（全校＋校外）所以需要 k8s」這套流程，**在本機親手做一遍**。
>
> **── 本線明確不做的事 ──**
> - ❌ **不量「現在能扛多少」**：那是「真上線之後」才需要回答的問題。展示練習線不必造假、也不必預測——**學會「怎麼擴」本身就是目標**。
> - ❌ **不上線**：整套 k8s 練習只發生在本機（kind）。
> - ❌ **不談 production HA**：read replica／跨機接管這類「上線才需要」的深度，最多**做概念練習**，不做承諾。
> - ✅ **要做的是**：把「image → cluster → 部署 → 水平複製 → 瓶頸概念」整條路徑，**用真工具跑通、留下實驗紀錄**。
>
> 本書 `_book/06.md` 6.6「何時才該考慮 k8s」判準表**依然要讀**——它的角色是**「練習的地圖」**：2.x 每一版就是把判準表每一列，用「在我自己的 cluster 上親眼看到的東西」逐一對應過去。判準不是「否決理由」，是**「練習該練到哪一層」的指引**。

---

## 規模動機（為什麼值得練這一條，但不做為交付）

| 項目 | 假設值 | 角色 |
|------|--------|------|
| 全校學生 | ~9,000 人（金門大學量級） | 背景動機 |
| 高峰同時在線 | 5k–2 萬人 | 背景動機 |
| 這些數字 | —— | **只當「練習要有個目標」的理由**，不是本版要量出來的交付 |

> 換句話說：**「撐住超多人」是 2.x 的 WHY，不是它的 2.0 交付。** 交付是「把過程練會」。

---

## 版本規劃（2.x 教學列車）

| 版號 | 主題 | 練會什麼 | 驗證關卡 |
|------|------|---------|---------|
| **2.0** | 本機長出真 k8s（kind） | image → cluster 的一條龍部署流程 | `kubectl get pods -A` 全 Running；`curl /health` 200 |
| **2.1** | api 水平複製練習 | 為什麼「無狀態 api」可以複製（JWT 無 session） | `kubectl scale --replicas=3`；pod 變 3 |
| **2.2** | DB 讀寫分離概念練習 | 為什麼「有狀態 DB」才是瓶頸、寫不能水平 | api 拆連線池走 replica（讀）／主庫（寫） |
| **2.3** | 可觀測性練習 | 擴張後「拿什麼數據看懂架構」 | `kubectl top` / 連線數有紀錄 |
| **2.4** | 回卷 6.6 判準表 | 判準每一列用「練習經驗」逐一對應 | `_book/06.md` 6.6 判準表附上「本機練習證據」欄 |

> **收斂原則（每版都適用）**：每次改動先跑 `bash test.sh` 確認 **72/72 仍綠**——練習線可以加資產，但**不能弄壞現有功能線**。

---

## 2.0　本機長出真 k8s（kind）——本版交付詳述

### 為什麼是 kind

本機現況只有 docker（無 kubectl / minikube / kind / k3d / helm / k9s）——而 **kind 是唯一「跑在現有 docker 之上、不用 VM 層、最貼近『展示練習』」的選擇**。GHCR 已備 `ghcr.io/ccc115a/nqu_erp-api:main` 映像——**部署不必重編譯**，原封拉下來放進 cluster 就好。

### 落地資產（本版要放進 repo）

```
deploy/k8s/
├── namespace.yaml        # nqu-erp
├── api-deployment.yaml   # 拉 GHCR api 映像
├── api-service.yaml      # ClusterIP + port 8080
├── api-ingress.yaml      # /api → api service
└── kind-config.yaml      # 單機偽多節點（3 nodes 模擬）

scripts/k8s_up.sh         # kind create → apply → port-forward → /health 驗證
```

### 驗收關卡（練習線的第一道綠）

1. `kind create cluster --config kind-config.yaml`
2. `kubectl apply -k deploy/k8s/`
3. `kubectl get pods -A` → api pod **Running**
4. `curl http://localhost:8081/health` → **200**

> 全部做完、綠了，才進入 2.1（api 水平複製）。

---

## 與全書／現況資產的接點

| v2.x 概念 | 接哪一章 / 哪份文件 |
|-----------|---------------------|
| image → cluster（GHCR 資產） | `_book/06.md` 6.2 GHCR / `_doc/v0.5.md` |
| 限流 429（練習 2.1 前的保護） | `_book/06.md` 6.4 L2 |
| 判準表當「練習地圖」 | `_book/06.md` 6.6 |
| 壓測哲學「先量再談」 | `_book/06.md` 6.1（**本線刻意先不量**，回卷時說明） |
| 現況測試 72 項全綠 | `test.sh`（每次改動的防線） |

---

## 建議的執行順序（一次一小步，每步可驗證）

| 步驟 | 做什麼 | 驗證 |
|------|--------|------|
| 1 | 裝 kind + kubectl | `kind version` / `kubectl version` |
| 2 | 寫 `deploy/k8s/` manifests（上面 6 檔） | `kubectl apply` 無 error |
| 3 | `scripts/k8s_up.sh` 一鍵上 cluster | `kubectl get pods` 全 Running |
| 4 | `/health` 200 | `curl` 實測 |
| 5 | （收斂）把練習數據寫回 `_doc/v2.0.md` | 檔案存在、無重複 |

> **v2.0 的「誠實」**：本版**不產出任何「能扛多少 RPS」的數字**——那是阿「真上線」之後才測的。本版的誠實是：**「我在自己長出的 k8s 上，把部署流程走通了」**＋再附上「為什麼 6.6 判準正因為『不會上線』而有別的意義」——不是掛著 k8s 的名義造假容量。
