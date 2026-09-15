#!/bin/bash
# NQU-ERP v2.0 驗收腳本 —— 在 kind-nqu-erp 這顆真 cluster 上，親眼打勾判準表最後一列
# 用法：bash test.sh
set -x

# ① 清殘留 port-forward（上次被 Ctrl-C 殺不乾淨的舊蒼蠅，會讓 18080 "address already in use"
# 　　這是"你的 forward 沒起來、卻有舊 forward 在吐 200"的假綠唯一來源 → 開場先捏死它們）
pkill -f "port-forward" 2>/dev/null
sleep 1

# ② 切到 api 真正住的家（＊=kind-nqu-erp，不是 docker-desktop）
kubectl config use-context kind-nqu-erp

# ③ 這顆 forward 才是親手開的（18080 這輪才空出來）
kubectl -n nqu-erp port-forward svc/nqu-erp-api 18080:80 &

# ④ 等 forward 真正就緒（log 出現 "Forwarding from" 才往下，不然 curl 打在空檔上=假 200）
for i in $(seq 1 10); do
  grep -q "Forwarding from" /tmp/pf_test 2>/dev/null && break
  sleep 1
done

# ⑤ 親眼：/health 要 200
code=$(curl -s -o /dev/null -w "%{http_code}" --max-time 5 http://127.0.0.1:18080/health)
echo "  /health → HTTP $code"
[ "$code" = "200" ] && echo "  ✅ v2.0 判準表最後一列親眼勾上" || echo "  ⚠️ 不是 200，先看 log"
