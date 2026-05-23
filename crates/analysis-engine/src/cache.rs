//! Shared intermediate artifact cache — content-addressed with persistence.
//!
//! Supports: hit/miss/stale/invalid explain, file snapshot comparison,
//! incremental rebuild planning, and persistent disk storage.

use crate::adapter::AdapterCapabilities;
use crate::dag::AnalysisArtifact;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;

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
    pub engine_version: String,
}

/// Cache entry status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CacheStatus {
    Hit,
    Miss(String),
    Stale { previous: String, current: String },
    Invalid(String),
}

/// File snapshot for incremental detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileSnapshot {
    pub path: String,
    pub hash: String,
    pub size: u64,
    pub modified_ms: u64,
}

/// Rich cache explain output.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheExplain {
    pub enabled: bool,
    pub persistent: bool,
    pub cache_dir: Option<String>,
    pub total_artifacts: usize,
    pub hits: usize,
    pub misses: usize,
    pub stale: usize,
    pub rebuilt: usize,
    pub skipped: usize,
    pub reasons: Vec<String>,
    pub details: Vec<CacheExplainEntry>,
    pub analysis_available_without_persistent_cache: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheExplainEntry {
    pub unit_id: String,
    pub stage: String,
    pub status: String,
    pub reason: String,
    pub cache_hit: bool,
}

/// Previous run state for incremental detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IncrementalPlan {
    pub root: String,
    pub language: String,
    pub previous_snapshots: Vec<FileSnapshot>,
    pub current_snapshots: Vec<FileSnapshot>,
    pub unchanged: usize,
    pub modified: usize,
    pub added: usize,
    pub removed: usize,
    pub total_files: usize,
}

/// In-memory + optional persistent artifact cache.
pub struct ArtifactCache {
    memory: HashMap<CacheKey, AnalysisArtifact>,
    persistent_dir: Option<PathBuf>,
    stats_hits: usize,
    stats_misses: usize,
    stats_stale: usize,
    stats_rebuilt: usize,
    previous_snapshots: Vec<FileSnapshot>,
}

impl ArtifactCache {
    pub fn new(persistent_dir: Option<PathBuf>) -> Self {
        if let Some(ref dir) = persistent_dir {
            let _ = fs::create_dir_all(dir);
        }
        let mut cache = Self {
            memory: HashMap::new(),
            persistent_dir,
            stats_hits: 0,
            stats_misses: 0,
            stats_stale: 0,
            stats_rebuilt: 0,
            previous_snapshots: Vec::new(),
        };
        cache.load_from_disk();
        cache
    }

    pub fn persistent_enabled(&self) -> bool { self.persistent_dir.is_some() }

    pub fn check(&self, key: &CacheKey) -> CacheStatus {
        if self.memory.contains_key(key) { return CacheStatus::Hit; }
        if self.persistent_dir.is_some() {
            if let Some(path) = self.persistent_path(key) {
                if path.exists() { return CacheStatus::Hit; }
            }
        }
        CacheStatus::Miss("not in cache".into())
    }

    pub fn store(&mut self, key: CacheKey, artifact: AnalysisArtifact) {
        if artifact.error.is_some() { return; }
        // Persist if dir is set
        if let Some(dir) = &self.persistent_dir {
            if let Some(pp) = self.persistent_path(&key) {
                if let Some(parent) = pp.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                if let Ok(json) = serde_json::to_string(&artifact) {
                    let _ = fs::write(&pp, json);
                }
            }
        }
        self.memory.insert(key, artifact);
    }

    pub fn get(&mut self, key: &CacheKey) -> Option<&AnalysisArtifact> {
        if self.memory.contains_key(key) { self.stats_hits += 1; return self.memory.get(key); }
        if let Some(pp) = self.persistent_dir.as_ref().and_then(|_| self.persistent_path(key)) {
            if pp.exists() {
                if let Ok(content) = fs::read_to_string(&pp) {
                    if let Ok(artifact) = serde_json::from_str::<AnalysisArtifact>(&content) {
                        self.stats_hits += 1;
                        self.memory.insert(key.clone(), artifact);
                        return self.memory.get(key);
                    }
                }
            }
        }
        self.stats_misses += 1;
        None
    }

    fn persistent_path(&self, key: &CacheKey) -> Option<PathBuf> {
        self.persistent_dir.as_ref().map(|dir| {
            let safe_path = key.path.replace('/', "_").replace('\\', "_");
            dir.join(format!("{}_{}_{}.json", key.language, key.stage, &safe_path[..safe_path.len().min(80)]))
        })
    }

    fn load_from_disk(&mut self) {
        if let Some(dir) = &self.persistent_dir {
            if let Ok(entries) = fs::read_dir(dir) {
                for e in entries.flatten() {
                    let p = e.path();
                    if p.extension().and_then(|e| e.to_str()) != Some("json") { continue; }
                    if let Ok(content) = fs::read_to_string(&p) {
                        if let Ok(artifact) = serde_json::from_str::<AnalysisArtifact>(&content) {
                            // Keep in persistent only (load on demand)
                        }
                    }
                }
            }
        }
    }

    /// Build a cache key for a file unit and stage.
    pub fn build_key(
        unit_id: &str, path: &str, content_hash: &str,
        language: &str, capabilities: &AdapterCapabilities, stage: &str,
    ) -> CacheKey {
        CacheKey {
            path: path.to_string(),
            content_hash: content_hash.to_string(),
            language: language.to_string(),
            adapter_version: capabilities.adapter_version.clone(),
            parser_version: capabilities.parser_version.clone(),
            stage: stage.to_string(),
            engine_version: "1.3".into(),
        }
    }

    /// Take a file snapshot for incremental comparison.
    pub fn snapshot_files(root: &Path, extensions: &[&str]) -> Vec<FileSnapshot> {
        let mut snaps = Vec::new();
        let _ = Self::snapshot_dir(root, extensions, &mut snaps, 4);
        snaps
    }

    fn snapshot_dir(dir: &Path, exts: &[&str], out: &mut Vec<FileSnapshot>, depth: usize) -> std::io::Result<()> {
        if depth == 0 { return Ok(()); }
        if let Ok(entries) = fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                let fname = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if p.is_dir() && !fname.starts_with('.') && fname != "node_modules" && fname != "target" {
                    let _ = Self::snapshot_dir(&p, exts, out, depth - 1);
                } else if p.is_file() {
                    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
                        if exts.contains(&ext) {
                            let meta = fs::metadata(&p).ok();
                            let hash = format!("{:x}", meta.as_ref().map(|m| m.len()).unwrap_or(0));
                            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
                            let modified = meta.as_ref().and_then(|m| m.modified().ok())
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_millis() as u64).unwrap_or(0);
                            out.push(FileSnapshot {
                                path: p.to_string_lossy().to_string(),
                                hash, size, modified_ms: modified,
                            });
                        }
                    }
                }
            }
        }
        Ok(())
    }

    /// Set previous snapshots for incremental detection.
    pub fn set_previous_snapshots(&mut self, snaps: Vec<FileSnapshot>) {
        self.previous_snapshots = snaps;
    }

    /// Build an incremental plan by comparing current vs previous snapshots.
    pub fn incremental_plan(&mut self, current: &[FileSnapshot]) -> IncrementalPlan {
        let prev_map: HashMap<&str, &FileSnapshot> = self.previous_snapshots.iter()
            .map(|s| (s.path.as_str(), s)).collect();
        let curr_map: HashMap<&str, &FileSnapshot> = current.iter()
            .map(|s| (s.path.as_str(), s)).collect();

        let mut unchanged = 0usize;
        let mut modified = 0usize;
        let mut added = 0usize;
        let mut removed = 0usize;

        for (path, curr) in &curr_map {
            if let Some(prev) = prev_map.get(path) {
                if prev.hash == curr.hash { unchanged += 1; }
                else { modified += 1; self.stats_stale += 1; }
            } else {
                added += 1;
            }
        }
        for path in prev_map.keys() {
            if !curr_map.contains_key(path) { removed += 1; }
        }

        IncrementalPlan {
            root: String::new(), language: String::new(),
            previous_snapshots: self.previous_snapshots.clone(),
            current_snapshots: current.to_vec(),
            unchanged, modified, added, removed,
            total_files: current.len(),
        }
    }

    /// Build a cache explain document with rich statistics.
    pub fn explain(&self, entries: Vec<CacheExplainEntry>, plan: Option<&IncrementalPlan>) -> CacheExplain {
        let hits = entries.iter().filter(|e| e.status == "hit").count();
        let misses = entries.iter().filter(|e| e.status == "miss").count();
        let stale = entries.iter().filter(|e| e.status == "stale").count();
        let rebuilt = entries.iter().filter(|e| e.status == "rebuilt").count();

        let mut reasons = Vec::new();
        if let Some(p) = plan {
            if p.added > 0 { reasons.push(format!("{} new files detected", p.added)); }
            if p.modified > 0 { reasons.push(format!("{} files modified", p.modified)); }
            if p.removed > 0 { reasons.push(format!("{} files removed", p.removed)); }
            if p.unchanged > 0 { reasons.push(format!("{} files unchanged", p.unchanged)); }
        }
        if hits > 0 { reasons.push(format!("{} cache hits from previous analysis", hits)); }
        if misses > 0 { reasons.push("First run or cache miss — artifacts will be built".into()); }

        CacheExplain {
            enabled: true,
            persistent: self.persistent_dir.is_some(),
            cache_dir: self.persistent_dir.as_ref().map(|d| d.to_string_lossy().to_string()),
            total_artifacts: self.memory.len(),
            hits, misses, stale,
            rebuilt: rebuilt + hits, // hits = reused
            skipped: 0,
            reasons,
            details: entries,
            analysis_available_without_persistent_cache: true,
        }
    }

    pub fn is_persistent_available(&self) -> bool { self.persistent_dir.is_some() }
    pub fn stats(&self) -> (usize, usize, usize, usize) {
        (self.stats_hits, self.stats_misses, self.stats_stale, self.stats_rebuilt)
    }
    pub fn entry_count(&self) -> usize { self.memory.len() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn incremental_detection() {
        let s1 = vec![
            FileSnapshot { path: "a.rs".into(), hash: "h1".into(), size: 100, modified_ms: 1 },
            FileSnapshot { path: "b.rs".into(), hash: "h2".into(), size: 200, modified_ms: 2 },
        ];
        let s2 = vec![
            FileSnapshot { path: "a.rs".into(), hash: "h1".into(), size: 100, modified_ms: 1 },
            FileSnapshot { path: "b.rs".into(), hash: "h3".into(), size: 201, modified_ms: 3 },
            FileSnapshot { path: "c.rs".into(), hash: "h4".into(), size: 300, modified_ms: 4 },
        ];
        let mut cache = ArtifactCache::new(None);
        cache.set_previous_snapshots(s1);
        let plan = cache.incremental_plan(&s2);
        assert_eq!(plan.unchanged, 1); // a.rs
        assert_eq!(plan.modified, 1);  // b.rs
        assert_eq!(plan.added, 1);     // c.rs
        assert_eq!(plan.removed, 0);
    }
}
