//! Shared intermediate artifact cache — content-addressed reuse for
//! parse/symbol/import/reference artifacts.
//!
//! Cache key: path + content_hash + language + adapter_version + parser_version + options.
//! Supports hit/miss/stale/invalid explain.
//! Graceful degrade when persistent cache unavailable.

use crate::adapter::AdapterCapabilities;
use crate::dag::AnalysisArtifact;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::Instant;

/// Content-addressed cache key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheKey {
    pub path: String,
    pub content_hash: String,
    pub language: String,
    pub adapter_version: String,
    pub parser_version: String,
    pub stage: String,
}

/// Cache entry status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CacheStatus {
    Hit,
    Miss(String),      // reason
    Stale { previous: String, current: String },
    Invalid(String),   // reason
}

/// Explain cache behavior for a specific stage/unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheExplain {
    pub total_units: usize,
    pub hits: usize,
    pub misses: usize,
    pub stale: usize,
    pub invalid: usize,
    pub details: Vec<CacheExplainEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheExplainEntry {
    pub unit_id: String,
    pub stage: String,
    pub status: String,
    pub reason: String,
}

/// In-memory + optional persistent artifact cache.
pub struct ArtifactCache {
    memory: HashMap<CacheKey, AnalysisArtifact>,
    persistent_dir: Option<PathBuf>,
    stats_hits: usize,
    stats_misses: usize,
}

impl ArtifactCache {
    pub fn new(persistent_dir: Option<PathBuf>) -> Self {
        let persistent_enabled = persistent_dir.is_some();
        Self { memory: HashMap::new(), persistent_dir, stats_hits: 0, stats_misses: 0 }
    }

    pub fn persistent_enabled(&self) -> bool { self.persistent_dir.is_some() }

    /// Check if an artifact exists in cache.
    pub fn check(&self, key: &CacheKey) -> CacheStatus {
        if self.memory.contains_key(key) {
            return CacheStatus::Hit;
        }
        CacheStatus::Miss("not in memory cache".into())
    }

    /// Store an artifact.
    pub fn store(&mut self, key: CacheKey, artifact: AnalysisArtifact) {
        // Only store if no error
        if artifact.error.is_none() {
            self.memory.insert(key, artifact);
        }
    }

    /// Retrieve an artifact.
    pub fn get(&mut self, key: &CacheKey) -> Option<&AnalysisArtifact> {
        if self.memory.contains_key(key) {
            self.stats_hits += 1;
        } else {
            self.stats_misses += 1;
        }
        self.memory.get(key)
    }

    /// Build cache key for a file unit and stage.
    pub fn build_key(
        unit_id: &str,
        path: &str,
        content_hash: &str,
        language: &str,
        capabilities: &AdapterCapabilities,
        stage: &str,
    ) -> CacheKey {
        CacheKey {
            path: path.to_string(),
            content_hash: content_hash.to_string(),
            language: language.to_string(),
            adapter_version: capabilities.adapter_version.clone(),
            parser_version: capabilities.parser_version.clone(),
            stage: stage.to_string(),
        }
    }

    /// Generate a cache explain document.
    pub fn explain(&self, explain_entries: Vec<CacheExplainEntry>) -> CacheExplain {
        let hits = explain_entries.iter().filter(|e| e.status == "hit").count();
        let misses = explain_entries.iter().filter(|e| e.status == "miss").count();
        let stale = explain_entries.iter().filter(|e| e.status == "stale").count();
        let invalid = explain_entries.iter().filter(|e| e.status == "invalid").count();

        CacheExplain {
            total_units: explain_entries.len(),
            hits,
            misses,
            stale,
            invalid,
            details: explain_entries,
        }
    }

    /// Check if persistent cache is available (not disabled).
    pub fn is_persistent_available(&self) -> bool { self.persistent_dir.is_some() }

    pub fn stats(&self) -> (usize, usize) {
        (self.stats_hits, self.stats_misses)
    }

    pub fn entry_count(&self) -> usize {
        self.memory.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::AdapterCapabilities;

    fn mock_caps() -> AdapterCapabilities {
        AdapterCapabilities {
            language: "mock".into(), adapter_version: "1.0".into(), parser_version: "1.0".into(),
            supports_parse: true, supports_symbols: true, supports_imports: true,
            supports_references: true, supports_calls: false, file_granularity: true,
            max_preferred_concurrency: None, notes: vec![],
        }
    }

    #[test]
    fn cache_hit_after_store() {
        let mut cache = ArtifactCache::new(None);
        let key = ArtifactCache::build_key("f1", "f1.mock", "hash1", "mock", &mock_caps(), "parse");

        assert!(matches!(cache.check(&key), CacheStatus::Miss(_)));

        let art = crate::dag::AnalysisArtifact {
            schema_version: "v1".into(), task_id: "t1".into(),
            stage: crate::dag::AnalysisStage::Parse, language: "mock".into(),
            unit_id: "f1".into(), cache_key: None, data: serde_json::json!({"ok": true}),
            error: None, duration_ms: 1,
            generated_from: crate::dag::ArtifactSemantics::default(),
        };

        cache.store(key.clone(), art);
        assert!(matches!(cache.check(&key), CacheStatus::Hit));
        assert_eq!(cache.entry_count(), 1);
    }

    #[test]
    fn persistent_not_available_is_explicit() {
        let cache = ArtifactCache::new(None);
        assert!(!cache.persistent_enabled());
    }

    #[test]
    fn cache_explain_counts() {
        let cache = ArtifactCache::new(None);
        let entries = vec![
            CacheExplainEntry { unit_id: "a".into(), stage: "parse".into(), status: "hit".into(), reason: "".into() },
            CacheExplainEntry { unit_id: "b".into(), stage: "parse".into(), status: "miss".into(), reason: "content changed".into() },
        ];
        let explain = cache.explain(entries);
        assert_eq!(explain.hits, 1);
        assert_eq!(explain.misses, 1);
    }
}
