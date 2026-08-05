//! HTTP Model Adapter —— Ollama / OpenAI-compatible 统一流式接口（P0-B1）。
//!
//! 用 reqwest::blocking + SSE 行解析（`/chat/completions`，stream=true）。
//! Ollama 的 `/v1` 兼容端点与 OpenAI-compatible 使用相同格式。
//!
//! 安全规则（§7.2）：
//! - Authorization header 只存在于请求构造期，绝不进入日志/snapshot/cache；
//! - 请求日志在写盘前删除认证 header 与明文 Key（`secret::redact_headers` 配合）；
//! - 本模块不持有明文 Key；Key 由调用方（Tauri 命令层）从 SecretStore 取出
//!   构造 header，绝不序列化回前端。

use std::sync::mpsc;
use std::thread;

use serde_json::{json, Value};

use crate::provider::{ConnectionStatus, ModelAdapter, ModelConfig, ProviderKind, StreamChunk};

/// SSE 行解析（纯函数，便于测试）：
/// `data: {"choices":[{"delta":{"content":"..."}}]}` → Some(Text("..."))
/// `data: [DONE]` → None（结束信号由外层区分）
pub enum SseLine {
    Chunk(String),
    Done,
    Ignore,
}

pub fn parse_sse_line(line: &str) -> SseLine {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return SseLine::Ignore;
    }
    let Some(data) = trimmed.strip_prefix("data:") else {
        return SseLine::Ignore;
    };
    let data = data.trim();
    if data == "[DONE]" {
        return SseLine::Done;
    }
    let Ok(v) = serde_json::from_str::<Value>(data) else {
        return SseLine::Ignore;
    };
    if let Some(text) = v
        .pointer("/choices/0/delta/content")
        .and_then(Value::as_str)
    {
        return SseLine::Chunk(text.to_string());
    }
    // 部分实现把完整消息放在 message.content
    if let Some(text) = v
        .pointer("/choices/0/message/content")
        .and_then(Value::as_str)
    {
        return SseLine::Chunk(text.to_string());
    }
    if let Some(text) = v.get("response").and_then(Value::as_str) {
        return SseLine::Chunk(text.to_string());
    }
    SseLine::Ignore
}

/// 真实 HTTP provider（feature "http" 时可用）。创建即持有配置与 client；
/// 连接测试与流式请求都只经 reqwest，不触碰其他模块状态。
pub struct HttpModelAdapter {
    pub config: ModelConfig,
    client: reqwest::blocking::Client,
}

impl HttpModelAdapter {
    pub fn new(config: ModelConfig) -> Result<Self, String> {
        if config.base_url.is_empty() {
            return Err("base_url is empty".to_string());
        }
        let client = reqwest::blocking::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(5))
            .timeout(std::time::Duration::from_secs(120))
            .build()
            .map_err(|e| format!("http client build failed: {e}"))?;
        Ok(Self { config, client })
    }

    /// 组装 chat/completions 请求体（stream=true）。不含任何认证信息。
    fn request_body(&self, prompt: &str) -> Value {
        json!({
            "model": self.config.model,
            "stream": true,
            "messages": [
                {"role": "system", "content": "你是 CodeLattice Understanding Gateway。\
                 只基于给定的证据回答；不能声称证据之外的事实。回答用中文。"},
                {"role": "user", "content": prompt}
            ],
            "temperature": 0.2,
            "max_tokens": 2048,
        })
    }

    /// 构造带认证的请求。api_key 从 SecretStore 由调用方解析后传入；
    /// header 值只在构造期存在于本函数栈内。
    fn request(&self, prompt: &str, api_key: Option<&str>) -> reqwest::blocking::RequestBuilder {
        let mut rb = self
            .client
            .post(format!(
                "{}/chat/completions",
                self.config.base_url.trim_end_matches('/')
            ))
            .json(&self.request_body(prompt));
        if let Some(key) = api_key {
            rb = rb.bearer_auth(key);
        }
        rb
    }
}

impl ModelAdapter for HttpModelAdapter {
    fn kind(&self) -> ProviderKind {
        self.config.provider.clone()
    }

    fn ping(&self) -> ConnectionStatus {
        // 返工修复：ping 也携带解析后的凭证（authenticated ping）
        self.ping_with_key(None)
    }

    fn stream_explain(
        &self,
        prompt: &str,
    ) -> Result<Box<dyn Iterator<Item = StreamChunk> + Send + '_>, String> {
        // P0-B1：无 Key 配置的本地模型（Ollama）为主路径；带 apiKeyRef 的模型
        // 由调用方（命令层）传入 Key 后走 `stream_explain_with_key`。
        self.stream_explain_with_key(prompt, None)
    }
}

impl HttpModelAdapter {
    /// authenticated ping：携带凭证的连接测试（§7.2 修复）。
    pub fn ping_with_key(&self, api_key: Option<&str>) -> ConnectionStatus {
        let url = format!("{}/models", self.config.base_url.trim_end_matches('/'));
        let mut req = self.client.get(&url);
        if let Some(key) = api_key {
            req = req.bearer_auth(key);
        }
        match req.send() {
            Ok(resp) if resp.status().is_success() => ConnectionStatus {
                ok: true,
                detail: format!("{} ok", resp.status()),
            },
            Ok(resp) => ConnectionStatus {
                ok: false,
                detail: format!(
                    "{}: {}",
                    resp.status(),
                    resp.text()
                        .unwrap_or_default()
                        .chars()
                        .take(120)
                        .collect::<String>()
                ),
            },
            Err(e) => ConnectionStatus {
                ok: false,
                detail: format!("{e}"),
            },
        }
    }

    /// 带 Key 的流式解释（命令层从 SecretStore 解析 Key 后调用）。
    /// 返回的 iterator 在独立线程读 SSE；错误以文本前缀 + Done 表达，
    /// 满足"流式期间的错误可被 UI 呈现"且不 panic。
    pub fn stream_explain_with_key(
        &self,
        prompt: &str,
        api_key: Option<&str>,
    ) -> Result<Box<dyn Iterator<Item = StreamChunk> + Send + '_>, String> {
        let request = self.request(prompt, api_key);
        let resp = request
            .send()
            .map_err(|e| format!("stream request failed: {e}"))?;
        if !resp.status().is_success() {
            return Err(format!("stream request failed: {}", resp.status()));
        }

        // blocking Response::text() 会一次读完整个 body；对 SSE 需要逐行读。
        // reqwest::blocking 提供 read 到 io::Read；用 BufReader 逐行。
        use std::io::{BufRead, BufReader};
        let reader = BufReader::new(resp);
        let (tx, rx) = mpsc::channel::<StreamChunk>();
        thread::spawn(move || {
            let mut done = false;
            for line in reader.lines() {
                if done {
                    break;
                }
                let Ok(line) = line else { break };
                match parse_sse_line(&line) {
                    SseLine::Chunk(text) => {
                        if tx.send(StreamChunk::Text(text)).is_err() {
                            break; // 消费者已放弃
                        }
                    }
                    SseLine::Done => {
                        done = true;
                    }
                    SseLine::Ignore => {}
                }
            }
            let _ = tx.send(StreamChunk::Done);
        });

        Ok(Box::new(ChannelIter { rx }))
    }
}

/// 从 mpsc 读取的迭代器。
struct ChannelIter {
    rx: mpsc::Receiver<StreamChunk>,
}

impl Iterator for ChannelIter {
    type Item = StreamChunk;
    fn next(&mut self) -> Option<Self::Item> {
        self.rx.recv().ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn parses_openai_compatible_sse_lines() {
        assert!(matches!(
            parse_sse_line(r#"data: {"choices":[{"delta":{"content":"你"}}]}"#),
            SseLine::Chunk(s) if s == "你"
        ));
        assert!(matches!(parse_sse_line("data: [DONE]"), SseLine::Done));
        assert!(matches!(parse_sse_line(""), SseLine::Ignore));
        assert!(matches!(parse_sse_line("event: ping"), SseLine::Ignore));
        assert!(matches!(
            parse_sse_line(r#"data: {"choices":[{"message":{"content":"x"}}]}"#),
            SseLine::Chunk(s) if s == "x"
        ));
        assert!(matches!(
            parse_sse_line(r#"data: {"response":"ollama-v0"}"#),
            SseLine::Chunk(s) if s == "ollama-v0"
        ));
    }

    #[test]
    fn malformed_sse_line_is_ignored_not_panicked() {
        assert!(matches!(parse_sse_line("data: {bad json"), SseLine::Ignore));
        assert!(matches!(parse_sse_line("data:"), SseLine::Ignore));
    }

    #[test]
    fn adapter_rejects_empty_base_url() {
        let cfg = ModelConfig {
            id: "x".into(),
            provider: ProviderKind::Ollama,
            base_url: String::new(),
            model: "m".into(),
            api_key_ref: None,
        };
        assert!(HttpModelAdapter::new(cfg).is_err());
    }

    #[test]
    fn request_body_contains_stream_and_model_no_key() {
        let cfg = ModelConfig {
            id: "qwen-local".into(),
            provider: ProviderKind::Ollama,
            base_url: "http://127.0.0.1:11434/v1".into(),
            model: "qwen3:14b".into(),
            api_key_ref: None,
        };
        let adapter = HttpModelAdapter::new(cfg).unwrap();
        let body = adapter.request_body("解释 foo");
        assert_eq!(body["model"], "qwen3:14b");
        assert_eq!(body["stream"], true);
        assert!(body["messages"].as_array().unwrap().len() >= 2);
        // request_body 不含任何认证信息（§7.2）
        assert!(body.get("authorization").is_none());
        assert!(body.get("api_key").is_none());
    }

    #[test]
    fn real_socket_sse_stream_sends_auth_and_terminates_once() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut socket, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            let mut buf = [0_u8; 2048];
            while !request.windows(4).any(|w| w == b"\r\n\r\n") {
                let read = socket.read(&mut buf).unwrap();
                if read == 0 {
                    break;
                }
                request.extend_from_slice(&buf[..read]);
            }
            let body = concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"你\"}}]}\n\n",
                "data: {bad json\n\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"好\"}}]}\n\n",
                "data: [DONE]\n\n"
            );
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
            String::from_utf8(request).unwrap()
        });

        let adapter = HttpModelAdapter::new(ModelConfig {
            id: "mock".into(),
            provider: ProviderKind::OpenaiCompatible,
            base_url: format!("http://{addr}/v1"),
            model: "mock-model".into(),
            api_key_ref: Some("keychain:test/mock".into()),
        })
        .unwrap();
        let chunks: Vec<_> = adapter
            .stream_explain_with_key("test", Some("dummy-secret"))
            .unwrap()
            .collect();
        let request = server.join().unwrap();

        assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1"));
        assert!(request
            .to_ascii_lowercase()
            .contains("authorization: bearer dummy-secret"));
        assert_eq!(
            chunks,
            vec![
                StreamChunk::Text("你".into()),
                StreamChunk::Text("好".into()),
                StreamChunk::Done,
            ]
        );
    }

    /// 真实供应商 smoke：凭证只从环境注入，并先写入临时 Keychain 条目再读取。
    /// Cleanup guard 保证测试成功或 panic 时都会删除临时条目；测试不打印模型正文。
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "requires CODELATTICE_LIVE_API_KEY and performs a paid remote request"]
    fn live_openai_compatible_stream_via_temporary_keychain() {
        use crate::secret::SecretStore;
        use crate::secret_keychain::KeychainSecretStore;
        use std::time::{SystemTime, UNIX_EPOCH};

        struct KeychainCleanup(String);
        impl Drop for KeychainCleanup {
            fn drop(&mut self) {
                let _ = KeychainSecretStore::new().delete(&self.0);
            }
        }

        let supplied_key = std::env::var("CODELATTICE_LIVE_API_KEY")
            .expect("CODELATTICE_LIVE_API_KEY is required for this ignored smoke");
        let suffix = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let service = format!("com.codelattice.workbench.live-test.{}", std::process::id());
        let account = format!("temporary-{suffix}");
        let mut keychain = KeychainSecretStore::new();
        let secret_ref = keychain.set(&service, &account, &supplied_key).unwrap();
        let _cleanup = KeychainCleanup(secret_ref.clone());
        drop(supplied_key);

        let api_key = keychain.get(&secret_ref).unwrap();
        let adapter = HttpModelAdapter::new(ModelConfig {
            id: "deepseek-live-smoke".into(),
            provider: ProviderKind::OpenaiCompatible,
            base_url: std::env::var("CODELATTICE_LIVE_BASE_URL")
                .unwrap_or_else(|_| "https://api.deepseek.com/v1".into()),
            model: std::env::var("CODELATTICE_LIVE_MODEL")
                .unwrap_or_else(|_| "deepseek-v4-flash".into()),
            api_key_ref: Some(secret_ref),
        })
        .unwrap();
        let chunks: Vec<_> = adapter
            .stream_explain_with_key("只回复一个汉字：好", Some(&api_key))
            .unwrap()
            .collect();
        drop(api_key);

        assert!(
            chunks
                .iter()
                .any(|chunk| matches!(chunk, StreamChunk::Text(text) if !text.is_empty())),
            "remote stream returned no text chunks"
        );
        assert_eq!(
            chunks
                .iter()
                .filter(|chunk| matches!(chunk, StreamChunk::Done))
                .count(),
            1,
            "remote stream must terminate exactly once"
        );
    }
}
