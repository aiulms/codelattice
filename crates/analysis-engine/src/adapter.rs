//! Language adapter concurrency contract — defines the interface every
//! language adapter must implement for the parallel engine.
//!
//! Adapters must be deterministic: same input → same output.
//! Adapters must NOT spawn unmanaged threads.
//! All analysis is static-only (no target code execution, no build scripts).

use serde::{Deserialize, Serialize};

/// A file or module unit that can be analyzed independently.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileUnit {
    pub id: String,
    pub path: String,
    pub language: String,
    pub content_hash: Option<String>,
    pub size_bytes: u64,
}

/// Adapter capabilities — what stages this adapter supports.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdapterCapabilities {
    pub language: String,
    pub adapter_version: String,
    pub parser_version: String,
    pub supports_parse: bool,
    pub supports_symbols: bool,
    pub supports_imports: bool,
    pub supports_references: bool,
    pub supports_calls: bool,
    pub file_granularity: bool,
    pub max_preferred_concurrency: Option<usize>,
    pub notes: Vec<String>,
}

/// Output of parse stage for one file unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParseOutput {
    pub unit_id: String,
    pub filename: String,
    pub ast_node_count: usize,
    pub tree_sitter_success: bool,
    pub diagnostics: Vec<String>,
    pub parse_duration_ms: u64,
}

/// Output of symbol extraction for one file unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolOutput {
    pub unit_id: String,
    pub symbol_count: usize,
    pub symbols: Vec<SymbolEntry>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SymbolEntry {
    pub name: String,
    pub kind: String,
    pub start_line: Option<usize>,
    pub end_line: Option<usize>,
    pub signature: Option<String>,
}

/// Output of import/reference extraction for one file unit.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutput {
    pub unit_id: String,
    pub import_count: usize,
    pub imports: Vec<ImportEntry>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportEntry {
    pub source: String,
    pub target: String,
    pub kind: String,
    pub resolved: bool,
}

/// Reference output (for CALLS edges).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceOutput {
    pub unit_id: String,
    pub call_count: usize,
    pub reference_count: usize,
    pub calls: Vec<CallEntry>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallEntry {
    pub caller: String,
    pub callee: String,
    pub confidence: f64,
    pub reason: String,
}

/// The trait language adapters implement for the parallel engine.
/// 
/// # Safety Contract
/// — All functions are deterministic (same input → same output)
/// — No spawning of unmanaged threads
/// — No target code execution, no build scripts, no package manager calls
/// — Functions may fail individually; the engine handles failure isolation
pub trait LanguageAdapter: Send + Sync {
    fn capabilities(&self) -> AdapterCapabilities;

    fn discover_files(&self, root: &str) -> Result<Vec<FileUnit>, String>;

    fn parse_file(&self, unit: &FileUnit) -> Result<ParseOutput, String>;

    fn extract_symbols(&self, unit: &FileUnit) -> Result<SymbolOutput, String>;

    fn extract_imports(&self, unit: &FileUnit) -> Result<ImportOutput, String>;

    fn extract_references(&self, unit: &FileUnit) -> Result<ReferenceOutput, String>;
}
