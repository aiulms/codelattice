//! MCP Job Runtime Integration — connects Analysis Engine 1.3
//! to MCP facade layer with non-blocking job submission, progress tracking,
//! compact results, and paged detail.
//!
//! Key design:
//! - Heavy analysis jobs return immediately with job handle + initial progress
//! - Default output is compact (summary only)
//! - Detail available via paged cursor
//! - Same root/language/mode jobs are deduplicated (SingleFlight)
//! - Analysis runs in background worker threads, never blocks the MCP event loop

use std::collections::HashMap;
use std::path::Path;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use gitnexus_analysis_engine::cache::{ArtifactCache, CacheExplainEntry, CacheKey, CacheStatus};
use gitnexus_analysis_engine::dag::{AnalysisArtifact, AnalysisPlan, AnalysisStage, AnalysisTask};
use gitnexus_analysis_engine::executor::{
    EngineConfig, ParallelExecutor, SerialExecutor, SerializableResult,
};
use gitnexus_analysis_engine::job::{
    AnalysisJob, JobProgress, JobResultSummary, JobRuntime, JobStatus, PagedResult,
};

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
    pub reused_existing_job: bool,
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

/// 轻量运行中 job 摘要，用于 backpressure 响应
#[derive(Debug, Clone)]
pub struct RunningJobInfo {
    pub job_id: String,
    pub root: String,
    pub language: String,
    pub mode: String,
    pub elapsed_ms: u64,
}

/// Thread-safe MCP job registry.
pub struct McpJobRegistry {
    jobs: Mutex<HashMap<String, McpJobHandle>>,
    results: Mutex<HashMap<String, SerializableResult>>,
    detail_pages: Mutex<HashMap<String, Vec<Value>>>,
    next_id: AtomicU64,
    active_analysis_count: AtomicUsize,
}

impl McpJobRegistry {
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
            results: Mutex::new(HashMap::new()),
            detail_pages: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            active_analysis_count: AtomicUsize::new(0),
        }
    }

    fn next_job_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        format!("job_engine_{:08x}", id)
    }

    fn dedup_key(root: &str, language: &str, mode: &str) -> String {
        format!("{}:{}:{}", root, language, mode)
    }

    pub fn submit(&self, root: &str, language: &str, mode: &str) -> McpJobHandle {
        let key = Self::dedup_key(root, language, mode);
        let mut jobs = self.jobs.lock().unwrap();

        for job in jobs.values() {
            if Self::dedup_key(&job.root, &job.language, &job.mode) == key {
                if matches!(job.status, JobStatus::Running | JobStatus::Queued) {
                    let mut deduped = job.clone();
                    deduped.reused_existing_job = true;
                    return deduped;
                }
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
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
            reused_existing_job: false,
        };
        jobs.insert(job_id, handle.clone());
        handle
    }

    pub fn update_status(&self, job_id: &str, status: JobStatus) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(j) = jobs.get_mut(job_id) {
                j.status = status;
                if status == JobStatus::Succeeded || status == JobStatus::Failed {
                    j.completed_at_ms = Some(
                        std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_millis() as u64,
                    );
                }
            }
        }
    }

    pub fn update_progress(&self, job_id: &str, progress: McpJobProgress) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(j) = jobs.get_mut(job_id) {
                j.progress = Some(progress);
                j.status = JobStatus::Running;
            }
        }
    }

    pub fn set_summary(&self, job_id: &str, summary: Value) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(j) = jobs.get_mut(job_id) {
                j.summary = Some(summary);
                j.status = JobStatus::Succeeded;
                j.completed_at_ms = Some(
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64,
                );
            }
        }
    }

    pub fn set_error(&self, job_id: &str, error: String) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(j) = jobs.get_mut(job_id) {
                j.error = Some(error);
                j.status = JobStatus::Failed;
            }
        }
    }

    pub fn get(&self, job_id: &str) -> Option<McpJobHandle> {
        self.jobs.lock().ok()?.get(job_id).cloned()
    }

    pub fn store_detail(&self, job_id: &str, items: Vec<Value>) {
        if let Ok(mut dp) = self.detail_pages.lock() {
            dp.insert(job_id.to_string(), items);
        }
    }

    pub fn get_detail_page(&self, job_id: &str, page: usize, page_size: usize) -> Option<Value> {
        let handle = self.get(job_id)?;
        let dp = self.detail_pages.lock().ok()?;
        let items = dp.get(job_id)?;
        let total = items.len();
        let page_size = page_size.clamp(1, 200);
        let total_pages = if total == 0 {
            1
        } else {
            (total + page_size - 1) / page_size
        };
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

    pub fn active_analysis_count(&self) -> usize {
        self.active_analysis_count.load(Ordering::SeqCst)
    }

    pub fn increment_active_analysis(&self) {
        self.active_analysis_count.fetch_add(1, Ordering::SeqCst);
    }

    pub fn decrement_active_analysis(&self) {
        self.active_analysis_count.fetch_sub(1, Ordering::SeqCst);
    }

    pub fn running_jobs_info(&self) -> Vec<RunningJobInfo> {
        let jobs = self.jobs.lock().unwrap();
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        jobs.values()
            .filter(|j| matches!(j.status, JobStatus::Running | JobStatus::Queued))
            .map(|j| RunningJobInfo {
                job_id: j.job_id.clone(),
                root: j.root.clone(),
                language: j.language.clone(),
                mode: j.mode.clone(),
                elapsed_ms: now_ms.saturating_sub(j.created_at_ms),
            })
            .collect()
    }

    pub fn to_response(&self, handle: &McpJobHandle) -> Value {
        let mut resp = serde_json::json!({
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
            "reusedExistingJob": handle.reused_existing_job,
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
            "cacheStats": cache_stats(),
            "nextActions": match handle.status {
                JobStatus::Succeeded => vec![
                    "Use mode=job_detail with this jobId and page/pageSize to fetch paged details; root is not required.".to_string()
                ],
                JobStatus::Running => vec![
                    format!("Check status with mode=job_status and jobId={} (root is not required).", handle.job_id)
                ],
                JobStatus::Failed => vec![
                    "Review error details above".to_string(),
                    "Retry with a specific project root and explicit language if the root/language was ambiguous.".to_string()
                ],
                _ => vec!["Job is queued, check status periodically".to_string()],
            },
            "compactResult": true,
            "detailPageHint": match handle.status {
                JobStatus::Succeeded => Some(format!("Call mode=job_detail with jobId={}, page=0, pageSize=50; root is not required.", handle.job_id)),
                _ => None::<String>,
            },
        });
        if handle.reused_existing_job {
            if let Some(obj) = resp.as_object_mut() {
                obj.insert("deduped".to_string(), Value::Bool(true));
            }
        }
        resp
    }
}

pub static MCP_JOBS: std::sync::LazyLock<McpJobRegistry> =
    std::sync::LazyLock::new(McpJobRegistry::new);

pub static ENGINE_CACHE: std::sync::LazyLock<std::sync::Mutex<ArtifactCache>> =
    std::sync::LazyLock::new(|| {
        let cache_dir = std::env::var("CODELATTICE_CACHE_DIR")
            .ok()
            .map(std::path::PathBuf::from);
        std::sync::Mutex::new(ArtifactCache::new(cache_dir))
    });

pub fn cache_stats() -> serde_json::Value {
    if let Ok(cache) = ENGINE_CACHE.lock() {
        let (hits, misses, stale, rebuilt) = cache.stats();
        serde_json::json!({
            "enabled": true,
            "persistent": cache.persistent_enabled(),
            "cacheDir": std::env::var("CODELATTICE_CACHE_DIR").ok(),
            "totalArtifacts": cache.entry_count(),
            "hits": hits, "misses": misses, "stale": stale, "rebuilt": rebuilt,
            "analysisAvailableWithoutPersistentCache": true,
        })
    } else {
        serde_json::json!({"error": "cache_lock_failed"})
    }
}

pub fn cache_store(key: gitnexus_analysis_engine::cache::CacheKey, artifact: AnalysisArtifact) {
    if let Ok(mut cache) = ENGINE_CACHE.lock() {
        cache.store(key, artifact);
    }
}

/// 异步提交 project analysis job：spawn 后台 worker thread，立即返回 job handle。
/// 分析在后台执行，通过 job_status/job_detail 查询进度和结果。
pub fn submit_project_job(root: &str, language: &str, mode: &str) -> Result<Value, String> {
    let handle = MCP_JOBS.submit(root, language, mode);

    if handle.reused_existing_job {
        return Ok(MCP_JOBS.to_response(&handle));
    }

    let job_id = handle.job_id.clone();
    let root_owned = root.to_string();
    let lang_owned = language.to_string();
    let mode_owned = mode.to_string();

    let job_id_outer = job_id.clone();
    MCP_JOBS.update_status(&job_id, JobStatus::Running);
    MCP_JOBS.increment_active_analysis();

    std::thread::spawn(move || {
        let _guard = ActiveAnalysisGuard {
            job_id: job_id.clone(),
        };

        let adapter = match crate::engine_bridge::get_adapter_for_language(&lang_owned) {
            Some(a) => a,
            None => {
                MCP_JOBS.set_error(
                    &job_id,
                    format!("No engine adapter for language: {}", lang_owned),
                );
                return;
            }
        };

        let files = match adapter.discover_files(&root_owned) {
            Ok(f) => f,
            Err(e) => {
                MCP_JOBS.set_error(&job_id, format!("discover_files failed: {}", e));
                return;
            }
        };

        if files.is_empty() {
            MCP_JOBS.set_error(&job_id, "No source files found".to_string());
            return;
        }

        let nf = files.len();
        MCP_JOBS.update_progress(
            &job_id,
            McpJobProgress {
                stage: "planning".into(),
                completed_units: 0,
                total_units: nf * 2,
                failed_units: 0,
                elapsed_ms: 0,
                executor_mode: "serial".into(),
                cache_hits: 0,
                cache_misses: 0,
            },
        );

        let tasks: Vec<AnalysisTask> = files
            .iter()
            .flat_map(|f| {
                [AnalysisStage::Parse, AnalysisStage::Symbol]
                    .iter()
                    .map(|s| AnalysisTask {
                        id: format!("{}:{}", s.name(), f.id),
                        stage: *s,
                        root: root_owned.clone(),
                        language: lang_owned.clone(),
                        unit_id: f.id.clone(),
                        depends_on: vec![],
                        cache_key: None,
                        parallelizable: s.is_file_parallelizable(),
                    })
            })
            .collect();

        let plan = AnalysisPlan {
            schema_version: "1.0".into(),
            root: root_owned.clone(),
            language: lang_owned.clone(),
            total_tasks: tasks.len(),
            stages: vec![AnalysisStage::Parse, AnalysisStage::Symbol],
            parallelizable_tasks: tasks.iter().filter(|t| t.parallelizable).count(),
            tasks,
            estimated_stages: [("parse".into(), nf), ("symbol".into(), nf)].into(),
        };

        MCP_JOBS.update_progress(
            &job_id,
            McpJobProgress {
                stage: "executing".into(),
                completed_units: 0,
                total_units: plan.total_tasks,
                failed_units: 0,
                elapsed_ms: 0,
                executor_mode: "serial".into(),
                cache_hits: 0,
                cache_misses: 0,
            },
        );

        let is_parallel = mode_owned.contains("parallel");
        let result = if is_parallel {
            ParallelExecutor::new(4).execute(&plan, Arc::from(adapter))
        } else {
            SerialExecutor.execute(&plan, adapter.as_ref())
        };

        let summary = serde_json::json!({
            "engine": "1.3",
            "mode": mode_owned,
            "root": root_owned,
            "language": lang_owned,
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

        for artifact in &result.artifacts {
            if artifact.error.is_none() {
                let key = CacheKey {
                    path: artifact.unit_id.clone(),
                    content_hash: format!("{:x}", artifact.unit_id.len()),
                    language: lang_owned.clone(),
                    adapter_version: "1.3".into(),
                    parser_version: "1.0".into(),
                    stage: artifact.stage.name().to_string(),
                    engine_version: "1.3".into(),
                };
                cache_store(key, artifact.clone());
            }
        }

        let detail_items: Vec<Value> = result.artifacts.iter().map(|a| serde_json::json!({
            "taskId": a.task_id, "stage": a.stage.name(), "unitId": a.unit_id,
            "error": a.error, "durationMs": a.duration_ms,
            "symbolCount": a.data.get("symbols").and_then(|v| v.as_array()).map(|a| a.len()).unwrap_or(0) as u64,
        })).collect();
        MCP_JOBS.store_detail(&job_id, detail_items);

        MCP_JOBS.set_summary(&job_id, summary);
    });

    let handle = MCP_JOBS.get(&job_id_outer).unwrap();
    Ok(MCP_JOBS.to_response(&handle))
}

/// RAII guard：worker thread 结束时自动递减 active_analysis_count
struct ActiveAnalysisGuard {
    job_id: String,
}

impl Drop for ActiveAnalysisGuard {
    fn drop(&mut self) {
        MCP_JOBS.decrement_active_analysis();
    }
}

/// 获取当前运行中 job 的摘要信息，用于 backpressure 响应
pub fn get_running_jobs_info() -> Vec<RunningJobInfo> {
    MCP_JOBS.running_jobs_info()
}

/// 获取当前活跃分析数
pub fn active_analysis_count() -> usize {
    MCP_JOBS.active_analysis_count()
}

pub fn get_job_status(job_id: &str) -> Option<Value> {
    MCP_JOBS.get(job_id).map(|h| MCP_JOBS.to_response(&h))
}

/// 异步提交 workspace analysis job：spawn 后台 worker thread，立即返回 job handle。
pub fn submit_workspace_job(root: &str, mode: &str) -> Result<Value, String> {
    let handle = MCP_JOBS.submit(root, "auto", mode);

    if handle.reused_existing_job {
        return Ok(MCP_JOBS.to_response(&handle));
    }

    let job_id = handle.job_id.clone();
    let root_owned = root.to_string();
    let mode_owned = mode.to_string();

    let job_id_outer = job_id.clone();
    MCP_JOBS.update_status(&job_id, JobStatus::Running);
    MCP_JOBS.increment_active_analysis();

    std::thread::spawn(move || {
        let _guard = ActiveAnalysisGuard {
            job_id: job_id.clone(),
        };

        let inventory = match gitnexus_workspace_model::scan_workspace_inventory(
            Path::new(&root_owned),
            true,
        ) {
            Ok(inv) => inv,
            Err(e) => {
                MCP_JOBS.set_error(&job_id, format!("Workspace scan: {}", e));
                return;
            }
        };

        let supported: Vec<_> = inventory
            .iter()
            .filter(|p| p.supported && !p.path.is_empty())
            .collect();
        let unsupported: Vec<_> = inventory.iter().filter(|p| !p.supported).collect();

        if supported.is_empty() {
            MCP_JOBS.set_error(&job_id, "No supported projects found".to_string());
            return;
        }

        MCP_JOBS.update_progress(
            &job_id,
            McpJobProgress {
                stage: "analyzing".into(),
                completed_units: 0,
                total_units: supported.len(),
                failed_units: 0,
                elapsed_ms: 0,
                executor_mode: mode_owned.clone(),
                cache_hits: 0,
                cache_misses: 0,
            },
        );

        let worker_limit = 4usize;
        let mut aggregated = Vec::new();
        let mut total_symbols = 0usize;
        let mut total_edges = 0usize;
        let mut completed = 0usize;
        let mut failed = 0usize;

        for chunk in supported.chunks(worker_limit) {
            let chunk_results: Vec<_> = chunk
                .iter()
                .map(|p| {
                    let proj_root = if p.path.starts_with('.') {
                        Path::new(&root_owned)
                            .join(&p.path)
                            .to_string_lossy()
                            .to_string()
                    } else {
                        p.path.clone()
                    };
                    let lang = p.language.clone();
                    let proj_name = p.name.clone();

                    if lang.is_empty() {
                        return serde_json::json!({
                            "project": proj_name, "path": proj_root, "status": "skipped", "reason": "empty language"
                        });
                    }

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
                                        stage: s,
                                        root: pr.clone(),
                                        language: lg.clone(),
                                        unit_id: f.id.clone(),
                                        depends_on: vec![],
                                        cache_key: None,
                                        parallelizable: s.is_file_parallelizable(),
                                    });
                                }
                            }
                            let mut cache_hit_count = 0usize;
                            if let Ok(cache) = ENGINE_CACHE.lock() {
                                for task in &tasks {
                                    if matches!(
                                        cache.check(&CacheKey {
                                            path: task.unit_id.clone(),
                                            content_hash: format!(
                                                "{:x}",
                                                task.unit_id.len()
                                            ),
                                            language: lang.clone(),
                                            adapter_version: "1.3".into(),
                                            parser_version: "1.0".into(),
                                            stage: task.stage.name().to_string(),
                                            engine_version: "1.3".into(),
                                        }),
                                        CacheStatus::Hit
                                    ) {
                                        cache_hit_count += 1;
                                    }
                                }
                            }
                            let all_cached =
                                cache_hit_count > 0 && cache_hit_count == tasks.len();

                            let plan = AnalysisPlan {
                                schema_version: "1.0".into(),
                                root: proj_root.to_string(),
                                language: lang.clone(),
                                total_tasks: tasks.len(),
                                stages: vec![AnalysisStage::Parse, AnalysisStage::Symbol],
                                parallelizable_tasks: tasks
                                    .iter()
                                    .filter(|t| t.parallelizable)
                                    .count(),
                                tasks,
                                estimated_stages: [
                                    ("parse".into(), nf),
                                    ("symbol".into(), nf),
                                ]
                                .into(),
                            };

                            let result = if all_cached {
                                SerializableResult {
                                    total_tasks: plan.total_tasks,
                                    completed: plan.total_tasks,
                                    failed: 0,
                                    total_duration_ms: 0,
                                    artifacts: vec![],
                                    stage_times: Default::default(),
                                    executor_mode: "cache".into(),
                                }
                            } else {
                                SerialExecutor.execute(&plan, adapter.as_ref())
                            };
                            for art in &result.artifacts {
                                if art.error.is_none() {
                                    cache_store(
                                        CacheKey {
                                            path: art.unit_id.clone(),
                                            content_hash: format!(
                                                "{:x}",
                                                art.unit_id.len()
                                            ),
                                            language: lang.clone(),
                                            adapter_version: "1.3".into(),
                                            parser_version: "1.0".into(),
                                            stage: art.stage.name().to_string(),
                                            engine_version: "1.3".into(),
                                        },
                                        art.clone(),
                                    );
                                }
                            }
                            serde_json::json!({
                                "project": proj_name, "path": proj_root,
                                "status": if result.failed > 0 && result.completed == 0 { "failed" } else { "completed" },
                                "files": nf, "tasks": result.total_tasks,
                                "completed": result.completed, "failed": result.failed,
                                "duration_ms": result.total_duration_ms,
                                "cache_hits": cache_hit_count,
                                "language": lang,
                            })
                        }
                        None => serde_json::json!({
                            "project": proj_name, "path": proj_root,
                            "status": "skipped", "reason": "no engine adapter",
                            "language": lang,
                        }),
                    }
                })
                .collect();

            for r in &chunk_results {
                let status = r["status"].as_str().unwrap_or("unknown");
                if status == "completed" {
                    completed += 1;
                } else if status == "failed" {
                    failed += 1;
                }
                aggregated.push(r.clone());
            }

            MCP_JOBS.update_progress(
                &job_id,
                McpJobProgress {
                    stage: "analyzing".into(),
                    completed_units: completed + failed,
                    total_units: supported.len(),
                    failed_units: failed,
                    elapsed_ms: 0,
                    executor_mode: mode_owned.clone(),
                    cache_hits: 0,
                    cache_misses: 0,
                },
            );
        }

        let summary = serde_json::json!({
            "workspace": true,
            "root": root_owned,
            "totalProjects": inventory.len(),
            "supportedCount": supported.len(),
            "unsupportedCount": unsupported.len(),
            "analyzedCount": completed,
            "failedCount": failed,
            "skippedCount": supported.len() - completed - failed,
            "totalSymbols": total_symbols,
            "totalEdges": total_edges,
            "detailItems": aggregated.len(),
            "detailsPaged": true,
            "staticAnalysisOnly": true,
            "targetCodeExecuted": false,
        });

        let detail_items: Vec<Value> = aggregated.clone();
        MCP_JOBS.store_detail(&job_id, detail_items);

        MCP_JOBS.set_summary(&job_id, summary);
    });

    let handle = MCP_JOBS.get(&job_id_outer).unwrap();
    Ok(MCP_JOBS.to_response(&handle))
}

pub fn is_large_project(root: &str, language: &str) -> bool {
    if let Some(adapter) = crate::engine_bridge::get_adapter_for_language(language) {
        if let Ok(files) = adapter.discover_files(root) {
            return files.len() > 10;
        }
    }
    false
}

/// 粗略估时函数：基于文件数、语言、cache probe 状态给出分析耗时估计。
/// 不要求准确，但不能没有。
pub fn estimate_analysis_duration(file_count: usize, language: &str, cache_status: &str) -> Value {
    let (estimated_seconds, estimated_class) = if cache_status == "hit" {
        (0, "cache-hit")
    } else if file_count <= 10 {
        (3, "small-project")
    } else if file_count <= 50 {
        (15, "medium-project")
    } else if file_count <= 200 {
        (45, "large-project")
    } else {
        (120, "very-large-project")
    };

    let retry_after = if cache_status == "hit" {
        0
    } else {
        (estimated_seconds / 4).max(2).min(30)
    };

    serde_json::json!({
        "estimatedSeconds": estimated_seconds,
        "estimatedClass": estimated_class,
        "retryAfterSeconds": retry_after,
        "fileCount": file_count,
        "language": language,
        "cacheStatus": cache_status,
    })
}
