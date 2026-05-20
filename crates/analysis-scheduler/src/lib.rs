//! CodeLattice analysis scheduler foundation.
//!
//! This crate does not execute analysis. It normalizes an analysis request,
//! computes a cheap filesystem fingerprint, and produces a deterministic phase
//! plan that higher layers can use for cache and stale-decision reporting.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnalysisScope {
    Summary,
    Quality,
    Graph,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CacheIntent {
    ReusePreferred,
    FreshRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisRequest {
    pub root: String,
    pub language: String,
    pub strict: bool,
    pub scope: AnalysisScope,
    pub cache_intent: CacheIntent,
    pub previous_fingerprint: Option<String>,
    generated_at_ms: Option<u64>,
}

impl AnalysisRequest {
    pub fn new(root: impl AsRef<Path>, language: impl Into<String>) -> Self {
        Self {
            root: root.as_ref().to_string_lossy().to_string(),
            language: language.into(),
            strict: false,
            scope: AnalysisScope::Graph,
            cache_intent: CacheIntent::ReusePreferred,
            previous_fingerprint: None,
            generated_at_ms: None,
        }
    }

    pub fn with_scope(mut self, scope: AnalysisScope) -> Self {
        self.scope = scope;
        self
    }

    pub fn with_strict(mut self, strict: bool) -> Self {
        self.strict = strict;
        self
    }

    pub fn with_cache_intent(mut self, intent: CacheIntent) -> Self {
        self.cache_intent = intent;
        self
    }

    pub fn with_previous_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.previous_fingerprint = Some(fingerprint.into());
        self
    }

    pub fn with_generated_at(mut self, generated_at: SystemTime) -> Self {
        self.generated_at_ms = Some(system_time_ms(generated_at));
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisFingerprint {
    pub fingerprint: String,
    pub tracked_file_count: usize,
    pub tracked_extensions: Vec<String>,
    pub total_bytes: u64,
    pub latest_modified_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisPhase {
    pub ordinal: u8,
    pub name: String,
    pub cacheable: bool,
    pub input_kinds: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CacheDecision {
    pub cache_intent: CacheIntent,
    pub action: String,
    pub reason: Option<String>,
    pub previous_fingerprint: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisSchedule {
    pub request: AnalysisRequest,
    pub fingerprint: AnalysisFingerprint,
    pub phases: Vec<AnalysisPhase>,
    pub decision: CacheDecision,
    pub generated_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleError {
    RootNotFound(PathBuf),
    Io(String),
}

impl std::fmt::Display for ScheduleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RootNotFound(path) => write!(f, "analysis root not found: {}", path.display()),
            Self::Io(message) => write!(f, "{message}"),
        }
    }
}

impl std::error::Error for ScheduleError {}

pub fn build_schedule(request: &AnalysisRequest) -> Result<AnalysisSchedule, ScheduleError> {
    let root = Path::new(&request.root);
    let canonical = canonical_root(root)?;
    let mut normalized = request.clone();
    normalized.root = canonical.to_string_lossy().to_string();

    let fingerprint = fingerprint_root(&canonical)?;
    let decision = cache_decision(
        request.cache_intent,
        &request.previous_fingerprint,
        &fingerprint,
    );
    let generated_at_ms = request
        .generated_at_ms
        .unwrap_or_else(|| system_time_ms(SystemTime::now()));

    Ok(AnalysisSchedule {
        request: normalized,
        fingerprint,
        phases: phases_for_scope(request.scope),
        decision,
        generated_at_ms,
    })
}

pub fn fingerprint_root(root: impl AsRef<Path>) -> Result<AnalysisFingerprint, ScheduleError> {
    let root = canonical_root(root.as_ref())?;
    let mut files = Vec::new();
    collect_files(&root, &root, &mut files)?;
    files.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));

    let mut state = FNV_OFFSET;
    let mut extensions = BTreeSet::new();
    let mut total_bytes = 0u64;
    let mut latest_modified_ms = 0u64;

    for file in &files {
        hash_bytes(&mut state, file.rel_path.as_bytes());
        hash_u64(&mut state, file.len);
        hash_u64(&mut state, file.modified_ms);
        total_bytes = total_bytes.saturating_add(file.len);
        latest_modified_ms = latest_modified_ms.max(file.modified_ms);
        if let Some(ext) = Path::new(&file.rel_path)
            .extension()
            .and_then(|s| s.to_str())
        {
            extensions.insert(ext.to_ascii_lowercase());
        }
    }

    Ok(AnalysisFingerprint {
        fingerprint: format!("{state:016x}"),
        tracked_file_count: files.len(),
        tracked_extensions: extensions.into_iter().collect(),
        total_bytes,
        latest_modified_ms,
    })
}

fn cache_decision(
    intent: CacheIntent,
    previous: &Option<String>,
    fingerprint: &AnalysisFingerprint,
) -> CacheDecision {
    let (action, reason) = match intent {
        CacheIntent::FreshRequired => ("fresh", Some("cache-bypass")),
        CacheIntent::ReusePreferred => match previous {
            Some(prev) if prev == &fingerprint.fingerprint => ("reuse", Some("fingerprint-match")),
            Some(_) => ("fresh", Some("fingerprint-changed")),
            None => ("fresh", Some("no-previous-fingerprint")),
        },
    };
    CacheDecision {
        cache_intent: intent,
        action: action.to_string(),
        reason: reason.map(str::to_string),
        previous_fingerprint: previous.clone(),
    }
}

fn phases_for_scope(scope: AnalysisScope) -> Vec<AnalysisPhase> {
    let names: &[(&str, &[&str])] = match scope {
        AnalysisScope::Summary => &[
            ("discover", &["manifest", "source-tree"]),
            ("fingerprint", &["source-metadata"]),
            ("parse", &["source"]),
            ("symbols", &["ast"]),
            ("diagnostics", &["graph-facts"]),
        ],
        AnalysisScope::Quality => &[
            ("discover", &["manifest", "source-tree"]),
            ("fingerprint", &["source-metadata"]),
            ("parse", &["source"]),
            ("symbols", &["ast"]),
            ("imports", &["ast", "manifest"]),
            ("calls", &["ast", "symbols"]),
            ("diagnostics", &["graph-facts"]),
        ],
        AnalysisScope::Graph => &[
            ("discover", &["manifest", "source-tree"]),
            ("fingerprint", &["source-metadata"]),
            ("parse", &["source"]),
            ("symbols", &["ast"]),
            ("imports", &["ast", "manifest"]),
            ("calls", &["ast", "symbols"]),
            ("diagnostics", &["graph-facts"]),
            ("graph", &["nodes", "edges", "quality-metrics"]),
        ],
    };

    names
        .iter()
        .enumerate()
        .map(|(idx, (name, inputs))| AnalysisPhase {
            ordinal: idx as u8 + 1,
            name: (*name).to_string(),
            cacheable: *name != "diagnostics",
            input_kinds: inputs.iter().map(|s| (*s).to_string()).collect(),
        })
        .collect()
}

#[derive(Debug, Clone)]
struct FileFact {
    rel_path: String,
    len: u64,
    modified_ms: u64,
}

fn canonical_root(root: &Path) -> Result<PathBuf, ScheduleError> {
    if !root.exists() {
        return Err(ScheduleError::RootNotFound(root.to_path_buf()));
    }
    root.canonicalize()
        .map_err(|e| ScheduleError::Io(format!("cannot canonicalize {}: {e}", root.display())))
}

fn collect_files(root: &Path, dir: &Path, out: &mut Vec<FileFact>) -> Result<(), ScheduleError> {
    let entries = fs::read_dir(dir)
        .map_err(|e| ScheduleError::Io(format!("cannot read {}: {e}", dir.display())))?;
    for entry in entries {
        let entry =
            entry.map_err(|e| ScheduleError::Io(format!("cannot read directory entry: {e}")))?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if should_skip_name(&name) {
            continue;
        }
        let metadata = entry
            .metadata()
            .map_err(|e| ScheduleError::Io(format!("cannot stat {}: {e}", path.display())))?;
        if metadata.is_dir() {
            collect_files(root, &path, out)?;
        } else if metadata.is_file() {
            let rel = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            out.push(FileFact {
                rel_path: rel,
                len: metadata.len(),
                modified_ms: metadata
                    .modified()
                    .ok()
                    .map(system_time_ms)
                    .unwrap_or_default(),
            });
        }
    }
    Ok(())
}

fn should_skip_name(name: &str) -> bool {
    name.starts_with('.')
        || matches!(
            name,
            "target"
                | "dist"
                | "node_modules"
                | "__pycache__"
                | ".pytest_cache"
                | ".mypy_cache"
                | ".ruff_cache"
        )
}

fn system_time_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

const FNV_OFFSET: u64 = 0xcbf29ce484222325;
const FNV_PRIME: u64 = 0x00000100000001b3;

fn hash_bytes(state: &mut u64, bytes: &[u8]) {
    for byte in bytes {
        *state ^= u64::from(*byte);
        *state = state.wrapping_mul(FNV_PRIME);
    }
}

fn hash_u64(state: &mut u64, value: u64) {
    hash_bytes(state, &value.to_le_bytes());
}
