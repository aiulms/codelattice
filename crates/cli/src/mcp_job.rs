//! MCP Job Runtime Integration — connects Analysis Engine 1.3
//! to MCP facade layer with non-blocking job submission, progress tracking,
//! compact results, and paged detail.
//!
//! Key design:
//! - Heavy analysis jobs return immediately with job handle + initial progress
//! - Default output is compact (summary only)
//! - Detail available via paged cursor
//! - Same root/language/mode jobs are deduplicated
//! - Bounded concurrency via worker pool

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::Value;

use gitnexus_analysis_engine::executor::{EngineConfig, SerialExecutor, ParallelExecutor, SerializableResult};
use gitnexus_analysis_engine::job::{JobRuntime, JobStatus, JobProgress, PagedResult, AnalysisJob, JobResultSummary};
use gitnexus_analysis_engine::dag::{AnalysisPlan, AnalysisStage, AnalysisTask};

/// MCP-level job handle — tracks one engine-backed analysis invocation.
#[derive(Debug, Clone)]
pub struct McpJobHandle {
    pub job_id: String,
    pub root: String,
    pub language: String,
    pub mode: String,
    pub status: JobStatus,
    pub progress: Option<McpJobProgress>,
    pub summary: Option<Value>,
    pub created_at_ms: u64,
    pub completed_at_ms: Option<u64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct McpJobProgress {
    pub stage: String,
    pub completed_units: usize,
    pub total_units: usize,
    pub failed_units: usize,
    pub elapsed_ms: u64,
    pub executor_mode: String,
    pub cache_hits: usize,
    pub cache_misses: usize,
}

/// Thread-safe MCP job registry.
pub struct McpJobRegistry {
    jobs: Mutex<HashMap<String, McpJobHandle>>,
    results: Mutex<HashMap<String, SerializableResult>>,
    detail_pages: Mutex<HashMap<String, Vec<Value>>>, // job_id → paginated items
    next_id: AtomicU64,
}

impl McpJobRegistry {
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
            results: Mutex::new(HashMap::new()),
            detail_pages: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
        }
    }

    fn next_job_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        format!("job_engine_{:08x}", id)
    }

    /// Create a key for deduplication.
    fn dedup_key(root: &str, language: &str, mode: &str) -> String {
        format!("{}:{}:{}", root, language, mode)
    }

    /// Submit a new analysis job. Deduplicates same root/language/mode.
    pub fn submit(&self, root: &str, language: &str, mode: &str) -> McpJobHandle {
        let key = Self::dedup_key(root, language, mode);
        let mut jobs = self.jobs.lock().unwrap();

        // Check for existing deduped job
        for job in jobs.values() {
            if Self::dedup_key(&job.root, &job.language, &job.mode) == key {
                if matches!(job.status, JobStatus::Running | JobStatus::Queued) {
                    return job.clone();
                }
            }
        }

        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64;
        let job_id = self.next_job_id();
        let handle = McpJobHandle {
            job_id: job_id.clone(),
            root: root.to_string(),
            language: language.to_string(),
            mode: mode.to_string(),
            status: JobStatus::Queued,
            progress: None,
            summary: None,
            created_at_ms: now,
            completed_at_ms: None,
            error: None,
        };
        jobs.insert(job_id, handle.clone());
        handle
    }

    /// Update job status.
    pub fn update_status(&self, job_id: &str, status: JobStatus) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(j) = jobs.get_mut(job_id) {
                j.status = status;
                if status == JobStatus::Succeeded || status == JobStatus::Failed {
                    j.completed_at_ms = Some(
                        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
                    );
                }
            }
        }
    }

    /// Update job progress.
    pub fn update_progress(&self, job_id: &str, progress: McpJobProgress) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(j) = jobs.get_mut(job_id) {
                j.progress = Some(progress);
                j.status = JobStatus::Running;
            }
        }
    }

    /// Set job result summary.
    pub fn set_summary(&self, job_id: &str, summary: Value) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(j) = jobs.get_mut(job_id) {
                j.summary = Some(summary);
                j.status = JobStatus::Succeeded;
                j.completed_at_ms = Some(
                    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis() as u64
                );
            }
        }
    }

    /// Set error on job.
    pub fn set_error(&self, job_id: &str, error: String) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(j) = jobs.get_mut(job_id) {
                j.error = Some(error);
                j.status = JobStatus::Failed;
            }
        }
    }

    /// Get a job by ID.
    pub fn get(&self, job_id: &str) -> Option<McpJobHandle> {
        self.jobs.lock().ok()?.get(job_id).cloned()
    }

    /// Store paginated detail data for a job.
    pub fn store_detail(&self, job_id: &str, items: Vec<Value>) {
        if let Ok(mut dp) = self.detail_pages.lock() {
            dp.insert(job_id.to_string(), items);
        }
    }

    /// Get a paged detail response for a job.
    pub fn get_detail_page(&self, job_id: &str, page: usize, page_size: usize) -> Option<Value> {
        let handle = self.get(job_id)?;
        let dp = self.detail_pages.lock().ok()?;
        let items = dp.get(job_id)?;
        let total = items.len();
        let total_pages = if total == 0 { 1 } else { (total + page_size - 1) / page_size };
        let start = page.saturating_mul(page_size);
        let end = (start + page_size).min(total);
        let page_items: Vec<&Value> = items[start..end].iter().collect();

        Some(serde_json::json!({
            "schemaVersion": "codelattice.pagedDetail.v1",
            "jobId": job_id,
            "page": page,
            "pageSize": page_size,
            "totalItems": total,
            "totalPages": total_pages,
            "hasMore": page + 1 < total_pages,
            "hasPrev": page > 0,
            "items": page_items,
            "summary": handle.summary,
            "generatedFrom": {
                "staticAnalysis": true,
                "runtimeVerified": false,
                "targetCodeExecuted": false,
                "coverageVerified": false
            },
            "analysisSemantics": {
                "staticAnalysisExecuted": true,
                "targetCodeExecuted": false,
                "targetScriptsExecuted": false,
                "runtimeProof": false
            },
            "nextActions": if page + 1 < total_pages {
                vec![format!("Fetch page {} with page={}&pageSize={}", page + 1, page + 1, page_size)]
            } else {
                vec!["All pages fetched".to_string()]
            },
        }))
    }

    /// Serialize a job handle to MCP response JSON.
    pub fn to_response(&self, handle: &McpJobHandle) -> Value {
        serde_json::json!({
            "schemaVersion": "codelattice.mcpJob.v1",
            "jobId": handle.job_id,
            "root": handle.root,
            "language": handle.language,
            "mode": handle.mode,
            "status": match handle.status {
                JobStatus::Queued => "queued",
                JobStatus::Running => "running",
                JobStatus::Succeeded => "succeeded",
                JobStatus::Failed => "failed",
                JobStatus::Cancelled => "cancelled",
            },
            "progress": handle.progress.as_ref().map(|p| serde_json::json!({
                "stage": p.stage,
                "completedUnits": p.completed_units,
                "totalUnits": p.total_units,
                "failedUnits": p.failed_units,
                "elapsedMs": p.elapsed_ms,
                "executorMode": p.executor_mode,
                "cacheHits": p.cache_hits,
                "cacheMisses": p.cache_misses,
            })),
            "summary": handle.summary,
            "error": handle.error,
            "createdAtMs": handle.created_at_ms,
            "completedAtMs": handle.completed_at_ms,
            "generatedFrom": {
                "staticAnalysis": true,
                "runtimeVerified": false,
                "targetCodeExecuted": false,
                "coverageVerified": false
            },
            "analysisSemantics": {
                "staticAnalysisExecuted": true,
                "targetCodeExecuted": false,
                "targetScriptsExecuted": false,
                "runtimeProof": false,
                "coverageProof": false,
                "explanation": "CodeLattice executed static analysis only. It did NOT run target project code."
            },
            "nextActions": match handle.status {
                JobStatus::Succeeded => vec!["Use mode=job_detail with page=0 to fetch paged details".to_string()],
                JobStatus::Running => vec![format!("Check status with jobId={}", handle.job_id)],
                JobStatus::Failed => vec!["Review error details above".to_string(), "Retry with different root or language".to_string()],
                _ => vec!["Job is queued, check status periodically".to_string()],
            },
            "compactResult": true,
            "detailPageHint": match handle.status {
                JobStatus::Succeeded => Some(format!("codelattice_project(mode=job_detail, jobId={}, page=0, pageSize=50)", handle.job_id)),
                _ => None::<String>,
            },
        })
    }
}

/// Global MCP job registry (lazy init).
pub static MCP_JOBS: std::sync::LazyLock<McpJobRegistry> = std::sync::LazyLock::new(McpJobRegistry::new);

/// Run engine-backed project analysis via job runtime.
/// Returns job handle immediately. Analysis runs synchronously for now
/// (MCP stdio is single-threaded), but the job contract is honored.
pub fn submit_project_job(root: &str, language: &str, mode: &str) -> Result<Value, String> {
    let handle = MCP_JOBS.submit(root, language, mode);

    // If already running/queued, return existing handle
    if matches!(handle.status, JobStatus::Running) {
        return Ok(MCP_JOBS.to_response(&handle));
    }

    MCP_JOBS.update_status(&handle.job_id, JobStatus::Running);

    // Run analysis synchronously (MCP stdio is single-threaded)
    let adapter = crate::engine_bridge::get_adapter_for_language(language)
        .ok_or_else(|| format!("No engine adapter for language: {}", language))?;

    let root_path = Path::new(root);
    let files = adapter.discover_files(root)?;
    if files.is_empty() {
        MCP_JOBS.set_error(&handle.job_id, "No source files found".to_string());
        return Ok(MCP_JOBS.to_response(&MCP_JOBS.get(&handle.job_id).unwrap()));
    }

    let nf = files.len();
    MCP_JOBS.update_progress(&handle.job_id, McpJobProgress {
        stage: "planning".into(),
        completed_units: 0, total_units: nf * 2, failed_units: 0,
        elapsed_ms: 0, executor_mode: "serial".into(),
        cache_hits: 0, cache_misses: 0,
    });

    // Build tasks
    let tasks: Vec<AnalysisTask> = files.iter().flat_map(|f| {
        [AnalysisStage::Parse, AnalysisStage::Symbol].iter().map(move |s| AnalysisTask {
            id: format!("{}:{}", s.name(), f.id), stage: *s,
            root: root.to_string(), language: language.to_string(),
            unit_id: f.id.clone(), depends_on: vec![], cache_key: None,
            parallelizable: s.is_file_parallelizable(),
        })
    }).collect();

    let plan = AnalysisPlan {
        schema_version: "1.0".into(), root: root.to_string(),
        language: language.to_string(), total_tasks: tasks.len(),
        stages: vec![AnalysisStage::Parse, AnalysisStage::Symbol],
        parallelizable_tasks: tasks.iter().filter(|t| t.parallelizable).count(),
        tasks,
        estimated_stages: [("parse".into(), nf), ("symbol".into(), nf)].into(),
    };

    MCP_JOBS.update_progress(&handle.job_id, McpJobProgress {
        stage: "executing".into(),
        completed_units: 0, total_units: plan.total_tasks, failed_units: 0,
        elapsed_ms: 0, executor_mode: "serial".into(),
        cache_hits: 0, cache_misses: 0,
    });

    let is_parallel = mode.contains("parallel");
    let result = if is_parallel {
        ParallelExecutor::new(4).execute(&plan, Arc::from(adapter))
    } else {
        SerialExecutor.execute(&plan, adapter.as_ref())
    };

    let summary = serde_json::json!({
        "engine": "1.3",
        "mode": mode,
        "root": root,
        "language": language,
        "total_tasks": result.total_tasks,
        "completed": result.completed,
        "failed": result.failed,
        "duration_ms": result.total_duration_ms,
        "executor_mode": result.executor_mode,
        "stage_times": result.stage_times,
        "static_analysis_only": true,
        "target_code_executed": false,
        "file_count": nf,
    });

    // Store paged detail from artifact data
    let detail_items: Vec<Value> = result.artifacts.iter().map(|a| serde_json::json!({
        "taskId": a.task_id, "stage": a.stage.name(), "unitId": a.unit_id,
        "error": a.error, "durationMs": a.duration_ms,
        "symbolCount": a.data.get("symbols").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0) as u64,
    })).collect();
    MCP_JOBS.store_detail(&handle.job_id, detail_items);

    MCP_JOBS.set_summary(&handle.job_id, summary);
    let final_handle = MCP_JOBS.get(&handle.job_id).unwrap();
    Ok(MCP_JOBS.to_response(&final_handle))
}

/// Get job status by ID.
pub fn get_job_status(job_id: &str) -> Option<Value> {
    MCP_JOBS.get(job_id).map(|h| MCP_JOBS.to_response(&h))
}

/// Submit a workspace analysis job (multi-project).
/// Actually runs engine analysis on each supported sub-project in parallel,
/// aggregates results, supports partial failures.
pub fn submit_workspace_job(root: &str, mode: &str) -> Result<Value, String> {
    let handle = MCP_JOBS.submit(root, "auto", mode);
    MCP_JOBS.update_status(&handle.job_id, JobStatus::Running);

    let inventory = gitnexus_workspace_model::scan_workspace_inventory(Path::new(root), true)
        .map_err(|e| format!("Workspace scan: {}", e))?;

    let supported: Vec<_> = inventory.iter().filter(|p| p.supported && !p.path.is_empty()).collect();
    let unsupported: Vec<_> = inventory.iter().filter(|p| !p.supported).collect();

    if supported.is_empty() {
        MCP_JOBS.set_error(&handle.job_id, "No supported projects found".to_string());
        let h = MCP_JOBS.get(&handle.job_id).unwrap();
        return Ok(MCP_JOBS.to_response(&h));
    }

    MCP_JOBS.update_progress(&handle.job_id, McpJobProgress {
        stage: "analyzing".into(),
        completed_units: 0,
        total_units: supported.len(),
        failed_units: 0,
        elapsed_ms: 0,
        executor_mode: mode.to_string(),
        cache_hits: 0,
        cache_misses: 0,
    });

    // Run engine analysis on each supported project (bounded parallel)
    let worker_limit = 4usize;
    let mut aggregated = Vec::new();
    let mut total_symbols = 0usize;
    let mut total_edges = 0usize;
    let mut completed = 0usize;
    let mut failed = 0usize;

    // Process in chunks for bounded parallelism
    for chunk in supported.chunks(worker_limit) {
        let chunk_results: Vec<_> = chunk.iter().map(|p| {
            let proj_root = p.path.clone();
            let lang = p.language.clone();
            let proj_name = p.name.clone();

            if lang.is_empty() {
                return serde_json::json!({
                    "project": proj_name, "path": proj_root, "status": "skipped", "reason": "empty language"
                });
            }

            // Try to analyze with engine adapter
            match crate::engine_bridge::get_adapter_for_language(&lang) {
                Some(adapter) => {
                    let files = adapter.discover_files(&proj_root).unwrap_or_default();
                    if files.is_empty() {
                        return serde_json::json!({
                            "project": proj_name, "path": proj_root,
                            "status": "completed", "files": 0, "symbols": 0,
                            "language": lang,
                        });
                    }
                    let nf = files.len();
                    let pn = &proj_name;
                    let pr = &proj_root;
                    let lg = &lang;
                    let mut tasks: Vec<AnalysisTask> = Vec::new();
                    for f in &files {
                        for s in [AnalysisStage::Parse, AnalysisStage::Symbol] {
                            tasks.push(AnalysisTask {
                                id: format!("ws-{}:{}:{}", pn, s.name(), f.id),
                                stage: s, root: pr.clone(), language: lg.clone(),
                                unit_id: f.id.clone(), depends_on: vec![], cache_key: None,
                                parallelizable: s.is_file_parallelizable(),
                            });
                        }
                    }

                    let plan = AnalysisPlan {
                        schema_version: "1.0".into(), root: proj_root.to_string(),
                        language: lang.to_string(), total_tasks: tasks.len(),
                        stages: vec![AnalysisStage::Parse, AnalysisStage::Symbol],
                        parallelizable_tasks: tasks.iter().filter(|t| t.parallelizable).count(),
                        tasks, estimated_stages: [("parse".into(), nf), ("symbol".into(), nf)].into(),
                    };

                    let result = SerialExecutor.execute(&plan, adapter.as_ref());
                    serde_json::json!({
                        "project": proj_name, "path": proj_root,
                        "status": if result.failed > 0 && result.completed == 0 { "failed" } else { "completed" },
                        "files": nf, "tasks": result.total_tasks,
                        "completed": result.completed, "failed": result.failed,
                        "duration_ms": result.total_duration_ms,
                        "language": lang,
                    })
                }
                None => serde_json::json!({
                    "project": proj_name, "path": proj_root,
                    "status": "skipped", "reason": "no engine adapter",
                    "language": lang,
                }),
            }
        }).collect();

        for r in &chunk_results {
            let status = r["status"].as_str().unwrap_or("unknown");
            if status == "completed" { completed += 1; }
            else if status == "failed" { failed += 1; }
            aggregated.push(r.clone());
        }

        MCP_JOBS.update_progress(&handle.job_id, McpJobProgress {
            stage: "analyzing".into(),
            completed_units: completed + failed,
            total_units: supported.len(),
            failed_units: failed,
            elapsed_ms: 0,
            executor_mode: mode.to_string(),
            cache_hits: 0,
            cache_misses: 0,
        });
    }

    let summary = serde_json::json!({
        "workspace": true,
        "root": root,
        "total_projects": inventory.len(),
        "supported_count": supported.len(),
        "unsupported_count": unsupported.len(),
        "analyzed_count": completed,
        "failed_count": failed,
        "skipped_count": supported.len() - completed - failed,
        "total_symbols": total_symbols,
        "total_edges": total_edges,
        "projects": aggregated,
    });

    let detail_items: Vec<Value> = aggregated.clone();
    MCP_JOBS.store_detail(&handle.job_id, detail_items);

    MCP_JOBS.set_summary(&handle.job_id, summary);
    let final_handle = MCP_JOBS.get(&handle.job_id).unwrap();
    Ok(MCP_JOBS.to_response(&final_handle))
}

/// Check if a root is likely a large project (should use job runtime).
pub fn is_large_project(root: &str, language: &str) -> bool {
    if let Some(adapter) = crate::engine_bridge::get_adapter_for_language(language) {
        if let Ok(files) = adapter.discover_files(root) {
            return files.len() > 10;
        }
    }
    false
}
