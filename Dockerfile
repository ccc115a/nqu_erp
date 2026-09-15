# ---------- builder：編譯 release binary ----------
FROM rust:slim-bookworm AS builder
WORKDIR /app

# 先複製 manifest 與 lockfile，善用 Docker layer cache（source 未變動時不必重新抓依賴）
COPY Cargo.toml Cargo.lock ./
COPY migrations/Cargo.toml ./migrations/

# 看不見依賴 crate 的空 source，讓 cargo 先 fetch + 編譯依賴層
COPY migrations/src ./migrations/src
COPY src ./src

RUN cargo build --release

# ---------- runtime：只帶 binary + 系統憑證 ----------
FROM debian:bookworm-slim

# curl 供 /health healthcheck 使用；ca-certificates 供外連 TLS 用
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates curl \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY --from=builder /app/target/release/nqu-erp /usr/local/bin/nqu-erp

EXPOSE 8080

# 容器內健康檢查：確認 HTTP server 已就緒
HEALTHCHECK --interval=10s --timeout=3s --retries=5 \
    CMD curl -sf http://localhost:8080/health || exit 1

# DATABASE_URL / JWT_SECRET / SERVER_ADDR / CORS_ORIGIN 由 docker-compose / 環境注入
CMD ["nqu-erp"]