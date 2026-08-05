//! Cache Manager —— 理解缓存（P0 §6.5）。
//!
//! cacheKey = hash(evidenceHash + modelProvider + modelId + modelRevision +
//! promptVersion + locale + explanationLevel)。
//! snapshot 更新后只失效 evidenceHash 变化的部分，不整仓清空。
//! 缓存独立 schema、零事实副作用、可整体删除并完全重建（验收 20/21）。

use std::collections::HashMap;

pub const CACHE_SCHEMA_VERSION: &str = "codelattice.understandingCache.v1";

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub evidence_hash: String,
    pub model_provider: String,
    pub model_id: String,
    pub model_revision: String,
    pub prompt_version: String,
    pub locale: String,
    pub explanation_level: String,
}

impl CacheKey {
    pub fn new(evidence_hash: impl Into<String>) -> Self {
        Self {
            evidence_hash: evidence_hash.into(),
            model_provider: String::new(),
            model_id: String::new(),
            model_revision: String::new(),
            prompt_version: String::new(),
            locale: "zh-CN".into(),
            explanation_level: "brief".into(),
        }
    }

    pub fn with_model(mut self, provider: &str, model_id: &str, revision: &str) -> Self {
        self.model_provider = provider.to_string();
        self.model_id = model_id.to_string();
        self.model_revision = revision.to_string();
        self
    }

    pub fn with_prompt(mut self, prompt_version: &str, locale: &str, level: &str) -> Self {
        self.prompt_version = prompt_version.to_string();
        self.locale = locale.to_string();
        self.explanation_level = level.to_string();
        self
    }

    pub fn key_string(&self) -> String {
        // 确定性组合哈希（FNV-1a），与 schemaVersion 一起构成 cacheKey
        let payload = format!(
            "{}|{}|{}|{}|{}|{}|{}|{}",
            CACHE_SCHEMA_VERSION,
            self.evidence_hash,
            self.model_provider,
            self.model_id,
            self.model_revision,
            self.prompt_version,
            self.locale,
            self.explanation_level,
        );
        fnv1a(&payload)
    }
}

fn fnv1a(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{:016x}", h)
}

#[derive(Debug, Clone)]
pub struct CachedEntry {
    pub value: serde_json::Value,
    pub created_at: u64,
}

#[derive(Debug, Default)]
pub struct UnderstandingCache {
    entries: HashMap<String, CachedEntry>,
    /// evidenceHash -> 关联的 cacheKey 列表（用于精确失效）
    by_evidence: HashMap<String, Vec<String>>,
}

impl UnderstandingCache {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &CacheKey) -> Option<&serde_json::Value> {
        self.entries.get(&key.key_string()).map(|e| &e.value)
    }

    pub fn put(&mut self, key: CacheKey, value: serde_json::Value, now: u64) {
        let ks = key.key_string();
        self.entries.insert(
            ks.clone(),
            CachedEntry {
                value,
                created_at: now,
            },
        );
        let list = self
            .by_evidence
            .entry(key.evidence_hash.clone())
            .or_default();
        if !list.contains(&ks) {
            list.push(ks);
        }
    }

    /// 精确失效：只清除 evidenceHash 变化的条目（验收 20）。
    pub fn invalidate_evidence(&mut self, evidence_hash: &str) -> usize {
        let keys = self.by_evidence.remove(evidence_hash).unwrap_or_default();
        let n = keys.len();
        for k in &keys {
            self.entries.remove(k);
        }
        n
    }

    /// 删除某模型配置时清理关联缓存；不触碰事实 Store（验收 21）。
    /// cacheKey 不可逆，模型归属索引由 service 层维护并调用 `remove_by_keys`。
    pub fn remove_by_keys(&mut self, keys: &[String]) -> usize {
        let mut n = 0;
        for k in keys {
            if self.entries.remove(k).is_some() {
                n += 1;
            }
        }
        // 重建 by_evidence 索引
        self.by_evidence.clear();
        for (k, e) in &self.entries {
            let ev = k.split('|').nth(1).unwrap_or("").to_string();
            self.by_evidence.entry(ev).or_default().push(k.clone());
            let _ = e;
        }
        n
    }

    /// 整体清空（验收 21：删除理解缓存后事实功能零变化）。
    pub fn clear(&mut self) {
        self.entries.clear();
        self.by_evidence.clear();
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn key(evidence: &str, provider: &str) -> CacheKey {
        CacheKey::new(evidence).with_model(provider, "qwen3:14b", "r1")
    }

    #[test]
    fn same_input_hits_cache_different_input_misses() {
        let mut c = UnderstandingCache::new();
        let k1 = key("eh:1", "ollama");
        let k2 = key("eh:1", "ollama");
        let k3 = key("eh:2", "ollama");
        c.put(k1.clone(), json!({"answer": "a"}), 1);
        assert!(c.get(&k2).is_some(), "相同 evidence/model 必须命中");
        assert!(c.get(&k3).is_none(), "证据改变必须 miss");
    }

    #[test]
    fn evidence_hash_change_invalidates_exactly_that_evidence() {
        let mut c = UnderstandingCache::new();
        c.put(key("eh:1", "ollama"), json!({"a": 1}), 1);
        c.put(key("eh:2", "ollama"), json!({"b": 2}), 1);
        let removed = c.invalidate_evidence("eh:1");
        assert_eq!(removed, 1);
        assert!(c.get(&key("eh:1", "ollama")).is_none());
        assert!(
            c.get(&key("eh:2", "ollama")).is_some(),
            "其他 evidence 不失效"
        );
    }

    #[test]
    fn model_switch_changes_cache_key() {
        let mut c = UnderstandingCache::new();
        let ollama = key("eh:1", "ollama");
        let remote = key("eh:1", "openai-compatible");
        c.put(ollama.clone(), json!({"m": "ollama"}), 1);
        assert!(c.get(&remote).is_none(), "切换模型不命中旧缓存");
    }

    #[test]
    fn clear_removes_everything_and_leaves_no_trace() {
        let mut c = UnderstandingCache::new();
        c.put(key("eh:1", "ollama"), json!({"a": 1}), 1);
        c.put(key("eh:2", "ollama"), json!({"b": 2}), 1);
        c.clear();
        assert_eq!(c.len(), 0);
        assert!(c.get(&key("eh:1", "ollama")).is_none());
    }
}
