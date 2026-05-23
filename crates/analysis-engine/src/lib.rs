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

pub mod dag;
pub mod executor;
pub mod reducer;
pub mod cache;
pub mod job;
pub mod adapter;

pub use dag::{AnalysisPlan, AnalysisTask, AnalysisStage, AnalysisArtifact};
pub use executor::{EngineConfig, SerialExecutor, ParallelExecutor, ProgressEvent, SerializableResult};
pub use reducer::{GraphReducer, MergeResult};
pub use cache::{ArtifactCache, CacheKey, CacheStatus, CacheExplain};
pub use job::{AnalysisJob, JobStatus, JobProgress, PagedResult};
pub use adapter::{LanguageAdapter, FileUnit, ParseOutput, SymbolOutput, AdapterCapabilities};
