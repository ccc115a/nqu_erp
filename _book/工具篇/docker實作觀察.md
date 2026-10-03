# HW1　Docker 實作觀察：交互式查看容器內容

> 對應工具篇第六章（Docker）。本作業在 NQU-ERP 已 `up` 的 stack 上實作：db（PostgreSQL）+ api（Rust）+ web（nginx）。

## 1. 前置確認：容器有在跑嗎

```bash
docker compose ps
docker ps --format 'table {{.Names}}\t{{.Image}}\t{{.Status}}'
```

預期看到三個 `Up (healthy)`：`nqu_erp-db-1`、`nqu_erp-api-1`、`nqu_erp-web-1`。
`exec` 只能進**已在跑**的容器；若不是 `Up`，先 `docker compose up -d`。

## 2. 交互式進入容器：`exec -it`

```bash
# 推薦：用 compose + service 名稱（db / api / web）
docker compose exec -it db sh      # postgres:16-alpine，只有 sh
docker compose exec -it api sh     # debian-slim，sh 一定有
docker compose exec -it web sh     # nginx alpine，只有 sh

# 或用容器名稱直接指定（效果相同）
docker exec -it nqu_erp-db-1 sh
docker exec -it nqu_erp-api-1 sh
docker exec -it nqu_erp-web-1 sh
```

要點：

- `-i` 保持標準輸入開啟，`-t` 分配虛擬終端機；兩者合起來才有可交互的 shell。
- 一律先試 `sh`：alpine 映像（db、web）沒有 `bash`，打 `bash` 會報 `not found`。
- 退出打 `exit` 即可，**不會**停掉容器。

## 3. 進去之後看什麼

```bash
ls -la                 # 看檔案
cat /etc/os-release    # 看是哪個 distro
ps aux 2>/dev/null || ps   # 看跑了什麼 process
env | sort             # 看環境變數（DATABASE_URL 等）
```

## 4. 兩個最常用的實戰指令

```bash
# 直接進 Postgres 查資料（不用先 exec 進 shell）
docker compose exec -it db psql postgres://nqu:nqu@db:5432/nqu
-- 進去後：\dt（列出 tables）、SELECT * FROM courses LIMIT 5;

# 只看檔案不進 shell（一行就走）
docker compose exec db ls /var/lib/postgresql/data
docker compose exec api ls /app
```

## 5. 觀察記錄（請填寫）

1. `cat /etc/os-release`：db / api / web 各是什麼 distro？
2. `env | sort`：api 的 `DATABASE_URL` 指向哪裡？為什麼是 `db` 而不是 `localhost`？
3. `ps`：api 容器裡 PID 1 跑的是什麼？和 `Dockerfile` 的 `CMD` 對得上嗎？
4. `SELECT count(*) FROM courses;`：目前有幾門課？和 `seed.sql` 對得上嗎？
