//! Analysis Engine 1.3 — True Parallelism Runtime
//!
//! Provides DAG-based analysis planning, bounded worker pool execution,
//! deterministic graph merge, and content-addressed intermediate caching.
//!
//! Key principles:
//! - Workers produce immutable intermediate artifacts; reducers merge them
//! - Graph merge is deterministic regardless of worker scheduling order
//! - Per-file failure is isolated — bad files become diagnostics, not crashes
//! - All analysis is static-only (no target code execution, no build scripts)

pub mod adapter;
pub mod cache;
pub mod dag;
pub mod executor;
pub mod job;
pub mod reducer;

pub use adapter::{AdapterCapabilities, FileUnit, LanguageAdapter, ParseOutput, SymbolOutput};
pub use cache::{
    ArtifactCache, CacheExplain, CacheExplainEntry, CacheKey, CacheStatus, FileSnapshot,
    IncrementalPlan,
};
pub use dag::{AnalysisArtifact, AnalysisPlan, AnalysisStage, AnalysisTask};
pub use executor::{
    EngineConfig, ParallelExecutor, ProgressEvent, SerialExecutor, SerializableResult,
};
pub use job::{AnalysisJob, JobProgress, JobStatus, PagedResult};
pub use reducer::{GraphReducer, MergeResult};
