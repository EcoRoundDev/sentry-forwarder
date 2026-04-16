#![allow(dead_code)]

use axum::{
    Router,
    body::Bytes,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::post,
};
use dotenvy::dotenv;
use flate2::read::GzDecoder;
use serde::Deserialize;
use serde_json::Value;
use std::env;
use std::io::Read;

// ============================================================
// ProxyPin 上报接口
// ============================================================

// HAR Entry 数据结构 (ProxyPin 上报格式)
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct HarEntry {
    started_date_time: Option<String>,
    time: Option<i64>,
    request: Option<HarRequest>,
    response: Option<HarResponse>,
    timings: Option<Value>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct HarRequest {
    method: Option<String>,
    url: Option<String>,
    http_version: Option<String>,
    headers: Option<Vec<HarHeader>>,
    post_data: Option<HarPostData>,
}

#[derive(Deserialize, Debug)]
#[allow(dead_code)]
struct HarHeader {
    name: String,
    value: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct HarPostData {
    mime_type: Option<String>,
    text: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct HarResponse {
    status: Option<i32>,
    status_text: Option<String>,
    content: Option<HarContent>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
struct HarContent {
    size: Option<i64>,
    mime_type: Option<String>,
    text: Option<String>,
}

// 处理 ProxyPin 上报请求
async fn handle_proxypin_report(headers: HeaderMap, body: Bytes) -> impl IntoResponse {
    // 1. 检查是否 gzip 压缩，如果是则解压
    let json_bytes = if headers
        .get("content-encoding")
        .and_then(|v| v.to_str().ok())
        .map(|v| v.to_lowercase().contains("gzip"))
        .unwrap_or(false)
    {
        let mut decoder = GzDecoder::new(&body[..]);
        let mut decompressed = Vec::new();
        if let Err(e) = decoder.read_to_end(&mut decompressed) {
            eprintln!("Failed to decompress gzip body: {}", e);
            return (StatusCode::BAD_REQUEST, "Failed to decompress gzip body");
        }
        decompressed
    } else {
        body.to_vec()
    };

    // 2. 解析 JSON 为 HarEntry
    let entry: HarEntry = match serde_json::from_slice(&json_bytes) {
        Ok(entry) => entry,
        Err(e) => {
            eprintln!("Failed to parse HAR entry: {}", e);
            return (StatusCode::BAD_REQUEST, "Invalid HAR entry JSON");
        }
    };

    // 获取 X-Report-Name 自定义报告名
    let report_name = headers
        .get("x-report-name")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("proxypin-report");

    // 构建 Sentry 消息
    let method = entry
        .request
        .as_ref()
        .and_then(|r| r.method.as_deref())
        .unwrap_or("UNKNOWN");
    let url = entry
        .request
        .as_ref()
        .and_then(|r| r.url.as_deref())
        .unwrap_or("unknown");
    let status = entry.response.as_ref().and_then(|r| r.status).unwrap_or(0);
    let duration = entry.time.unwrap_or(0);

    let message = format!("{} {} -> {} ({:.0}ms)", method, url, status, duration);

    println!("ProxyPin report: {}", message);

    // 根据 HTTP 状态码确定 Sentry 日志级别
    let sentry_level = match status {
        0 => sentry::Level::Warning,         // 无响应
        400..=499 => sentry::Level::Warning, // 客户端错误
        500..=599 => sentry::Level::Error,   // 服务端错误
        _ => sentry::Level::Info,            // 正常请求
    };

    // 3. 发送到 Sentry
    sentry::with_scope(
        |scope| {
            scope.set_tag("source", "proxypin");
            scope.set_tag("report_name", report_name);
            scope.set_tag("http.method", method);
            scope.set_tag("http.status_code", &status.to_string());

            // 将请求详情作为 extra 数据附加
            if let Some(ref req) = entry.request {
                if let Some(ref url) = req.url {
                    scope.set_extra("request.url", Value::String(url.clone()));
                }
                if let Some(ref method) = req.method {
                    scope.set_extra("request.method", Value::String(method.clone()));
                }
                if let Some(ref post_data) = req.post_data {
                    if let Some(ref text) = post_data.text {
                        // 尝试将 postData 解析为 JSON，否则作为字符串
                        let post_value = serde_json::from_str::<Value>(text)
                            .unwrap_or_else(|_| Value::String(text.clone()));
                        scope.set_extra("request.body", post_value);
                    }
                }
                if let Some(ref headers) = req.headers {
                    let headers_obj: Value = headers
                        .iter()
                        .map(|h| (h.name.clone(), Value::String(h.value.clone())))
                        .collect::<serde_json::Map<String, Value>>()
                        .into();
                    scope.set_extra("request.headers", headers_obj);
                }
            }

            // 将响应详情作为 extra 数据附加
            if let Some(ref resp) = entry.response {
                scope.set_extra("response.status", Value::Number(status.into()));
                if let Some(ref status_text) = resp.status_text {
                    scope.set_extra("response.statusText", Value::String(status_text.clone()));
                }
                if let Some(ref content) = resp.content {
                    if let Some(ref text) = content.text {
                        // 尝试将响应内容解析为 JSON，否则作为字符串
                        let resp_value = serde_json::from_str::<Value>(text)
                            .unwrap_or_else(|_| Value::String(text.clone()));
                        scope.set_extra("response.body", resp_value);
                    }
                }
            }

            // 附加耗时信息
            scope.set_extra("duration_ms", Value::from(duration));

            if let Some(ref started) = entry.started_date_time {
                scope.set_extra("started_at", Value::String(started.clone()));
            }

            if let Some(ref timings) = entry.timings {
                scope.set_extra("timings", timings.clone());
            }
        },
        || {
            sentry::capture_message(&message, sentry_level);
        },
    );

    (StatusCode::OK, "ProxyPin report forwarded to Sentry")
}

// ============================================================
// 主函数
// ============================================================

#[tokio::main]
async fn main() {
    dotenv().ok();
    // 初始化 Sentry
    // 建议通过环境变量传入 DSN，避免硬编码
    let sentry_dsn = env::var("SENTRY_DSN").expect("SENTRY_DSN environment variable is required");

    // _guard 必须被保留在 main 函数的生命周期中，丢弃它会导致 flush 并关闭 Sentry 客户端
    let _guard = sentry::init((
        sentry_dsn,
        sentry::ClientOptions {
            release: sentry::release_name!(),
            // 可以开启 debug 模式查看 Sentry SDK 内部的请求日志
            // debug: true,
            ..Default::default()
        },
    ));

    // 配置路由
    let app = Router::new()
        // ProxyPin 上报接口
        .route("/api/report", post(handle_proxypin_report));

    // 绑定端口并启动服务，支持通过环境变量配置
    let host = env::var("HOST").unwrap_or_else(|_| "0.0.0.0".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "3000".to_string());
    let addr = format!("{}:{}", host, port);
    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    println!("Server running on {}", addr);

    axum::serve(listener, app).await.unwrap();
}
