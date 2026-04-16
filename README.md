# Sentry Forwarder

🚀 A lightweight, high-performance intermediate API layer for forwarding app telemetry and crash reports to Sentry, built with Rust.

一个基于 Rust 构建的高性能、轻量级中间层 API。用于接收客户端 App 的上报数据（如崩溃日志、遥测数据）并将其安全地转发至 Sentry。

## ✨ 为什么需要这个中间层？

在使用 Sentry 收集客户端日志时，直接在 App 中集成 Sentry SDK 可能会带来一些痛点。本项目旨在解决以下问题：

* **安全性 (DSN 保护)**：避免在客户端（Android/iOS/Flutter）中硬编码 Sentry DSN，防止 DSN 被逆向提取和滥用（例如恶意刷量）。
* **数据清洗与过滤**：在日志真正进入 Sentry 之前，可以在此中间层对数据进行脱敏、过滤无用报错或重组格式。
* **统一出口**：方便在服务端统一管理所有的遥测数据流，未来如果需要将数据同时双写到其他监控系统（如 Prometheus、Elasticsearch），只需修改此服务即可，无需更新客户端。

## 🛠️ 技术栈

* **语言**: [Rust](https://www.rust-lang.org/)
* **Web 框架**: [Axum](https://github.com/tokio-rs/axum)
* **异步运行时**: [Tokio](https://tokio.rs/)
* **Sentry SDK**: [sentry-rust](https://github.com/getsentry/sentry-rust)

## 🚀 快速开始

### 1. 前置条件

* 安装 [Rust](https://rustup.rs/) (建议 1.70 及以上版本)
* 拥有一个 Sentry 账号并获取你的项目 `DSN`。

### 2. 本地运行

克隆仓库并进入项目目录：

```bash
git clone [https://github.com/你的用户名/sentry-forwarder.git](https://github.com/你的用户名/sentry-forwarder.git)
cd sentry-forwarder