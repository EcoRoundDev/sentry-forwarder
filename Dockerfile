# 编译阶段
FROM rust:latest as builder
WORKDIR /usr/src/app
COPY . .
RUN cargo install --path .

# 运行阶段
FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*
COPY --from=builder /usr/local/cargo/bin/sentry_forwarder /usr/local/bin/sentry_forwarder
ENV SENTRY_DSN=""
ENV HOST="0.0.0.0"
ENV PORT="3000"
EXPOSE 3000
CMD ["sentry_forwarder"]