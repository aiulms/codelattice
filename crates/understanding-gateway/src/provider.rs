//! Model Adapter —— Ollama / OpenAI-compatible 统一流式接口（P0 §7）。
//!
//! P0 模型池只支持：Ollama、一个通用 OpenAI-compatible adapter、连接测试、
//! 增删、设默认。计费统计、embedding、模型市场后置。
//! Key 绝不进入 snapshot、缓存、日志或 prompt（§7.2）。

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProviderKind {
    Ollama,
    OpenaiCompatible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfig {
    pub id: String,
    pub provider: ProviderKind,
    pub base_url: String,
    pub model: String,
    /// 只保存 secret reference（如 "keychain:codelattice/remote-compatible"），
    /// 绝不明文（§7.1）。
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_key_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionStatus {
    pub ok: bool,
    pub detail: String,
}

/// 统一流式输出片段（模型适配层 → Gateway）。
#[derive(Debug, Clone, PartialEq)]
pub enum StreamChunk {
    Text(String),
    Done,
}

/// 模型适配接口：生产实现（reqwest + SSE / ollama stream）在 `provider_http`
/// 模块（feature "http"）；本 trait 冻结契约。
pub trait ModelAdapter: Send + Sync {
    fn kind(&self) -> ProviderKind;
    /// 连接测试（不发送任何证据）。
    fn ping(&self) -> ConnectionStatus;
    /// 单次解释请求的流式输出。返回 Err 表示请求前失败（网络/鉴权/超时）；
    /// 流式期间的错误以 `StreamChunk::Text` 错误前缀 + `Done` 表达并记录。
    fn stream_explain(
        &self,
        prompt: &str,
    ) -> Result<Box<dyn Iterator<Item = StreamChunk> + Send + '_>, String>;
}

/// 模型池：增删、设默认、连接测试（P0 §7.1）。
#[derive(Debug, Default)]
pub struct ModelPool {
    models: Vec<ModelConfig>,
    default_id: Option<String>,
}

impl ModelPool {
    pub fn new() -> Self {
        Self {
            models: Vec::new(),
            default_id: None,
        }
    }

    pub fn add(&mut self, config: ModelConfig) -> bool {
        if self.models.iter().any(|m| m.id == config.id) {
            return false;
        }
        if self.default_id.is_none() {
            self.default_id = Some(config.id.clone());
        }
        self.models.push(config);
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let before = self.models.len();
        self.models.retain(|m| m.id != id);
        if self.default_id.as_deref() == Some(id) {
            self.default_id = self.models.first().map(|m| m.id.clone());
        }
        self.models.len() != before
    }

    pub fn set_default(&mut self, id: &str) -> bool {
        if self.models.iter().any(|m| m.id == id) {
            self.default_id = Some(id.to_string());
            true
        } else {
            false
        }
    }

    pub fn get(&self, id: &str) -> Option<&ModelConfig> {
        self.models.iter().find(|m| m.id == id)
    }

    pub fn default_model(&self) -> Option<&ModelConfig> {
        self.default_id.as_ref().and_then(|id| self.get(id))
    }

    pub fn list(&self) -> &[ModelConfig] {
        &self.models
    }

    /// 明文 Key 校验：配置中不允许出现明文（§7.2 防守规则）。
    pub fn assert_no_plaintext_keys(&self) -> bool {
        self.models.iter().all(|m| {
            m.api_key_ref.as_ref().map_or(true, |r| {
                r.starts_with("keychain:") || r.starts_with("secret:")
            })
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(id: &str) -> ModelConfig {
        ModelConfig {
            id: id.into(),
            provider: ProviderKind::Ollama,
            base_url: "http://127.0.0.1:11434/v1".into(),
            model: "qwen3:14b".into(),
            api_key_ref: None,
        }
    }

    #[test]
    fn pool_add_remove_default() {
        let mut pool = ModelPool::new();
        assert!(pool.add(config("local")));
        assert!(!pool.add(config("local")), "重复 id 拒绝");
        assert!(pool.add(config("remote")));
        assert!(pool.set_default("remote"));
        assert_eq!(pool.default_model().unwrap().id, "remote");
        assert!(pool.remove("remote"));
        assert_eq!(
            pool.default_model().unwrap().id,
            "local",
            "删除默认后回退到第一个"
        );
        assert!(!pool.remove("ghost"));
    }

    #[test]
    fn plaintext_key_is_rejected_by_policy() {
        let mut pool = ModelPool::new();
        let mut bad = config("bad");
        bad.api_key_ref = Some("sk-plaintext".into());
        pool.add(bad);
        assert!(!pool.assert_no_plaintext_keys(), "明文 Key 必须被策略拒绝");

        let mut pool2 = ModelPool::new();
        let mut good = config("good");
        good.api_key_ref = Some("keychain:codelattice/remote".into());
        pool2.add(good);
        assert!(pool2.assert_no_plaintext_keys());
    }
}
