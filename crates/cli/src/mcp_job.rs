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
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{json, Value};

use gitnexus_analysis_engine::cache::{ArtifactCache, CacheExplainEntry, CacheKey, CacheStatus};
use gitnexus_analysis_engine::dag::{AnalysisArtifact, AnalysisPlan, AnalysisStage, AnalysisTask};
use gitnexus_analysis_engine::executor::{
    EngineConfig, ParallelExecutor, SerialExecutor, SerializableResult,
};
use gitnexus_analysis_engine::job::{
    AnalysisJob, JobProgress, JobResultSummary, JobRuntime, JobStatus, PagedResult,
};

/// 全局 McpCache 引用，由 run_mcp_server() 在启动时设置。
/// job worker 完成后通过此引用把分析结果灌入 McpCache 图谱缓存，
/// 实现闭环：job succeeded → 后续 facade 复用已完成分析结果。
static MCP_FACADE_CACHE: OnceLock<Mutex<McpFacadeCacheSlot>> = OnceLock::new();

/// McpCache 的类型擦除槽位：存储 Arc<Mutex<impl FacadeCacheWarmer>>
pub struct McpFacadeCacheSlot {
    inner: Box<dyn FacadeCacheWarmer + Send>,
}

impl McpFacadeCacheSlot {
    pub fn new(inner: Box<dyn FacadeCacheWarmer + Send>) -> Self {
        Self { inner }
    }
}

/// facade 图谱缓存预热 trait：job worker 完成后调用 warm() 把分析结果灌入缓存。
pub trait FacadeCacheWarmer {
    fn warm(&self, root: &str, language: &str) -> Result<(), String>;
    fn warm_from_result(
        &self,
        root: &str,
        language: &str,
        result: &SerializableResult,
    ) -> Result<crate::mcp_server::WarmCacheMeta, String>;
}

/// 由 run_mcp_server() 调用，注册全局 facade 图谱缓存引用
pub fn register_facade_cache(warmer: Box<dyn FacadeCacheWarmer + Send>) {
    let slot = McpFacadeCacheSlot::new(warmer);
    if let Some(existing) = MCP_FACADE_CACHE.get() {
        if let Ok(mut existing) = existing.lock() {
            *existing = slot;
        }
        return;
    }
    let _ = MCP_FACADE_CACHE.set(Mutex::new(slot));
}

/// job worker 完成后调用，尝试预热 facade 图谱缓存
fn try_warm_facade_cache(root: &str, language: &str) -> Result<(), String> {
    let slot = MCP_FACADE_CACHE
        .get()
        .ok_or_else(|| "facade cache warmer not registered".to_string())?;
    let slot = slot
        .lock()
        .map_err(|_| "facade cache warmer lock poisoned".to_string())?;
    slot.inner.warm(root, language)
}

/// Warm facade cache directly from job artifacts (no subprocess re-analysis).
/// This is the fast path used by job workers after analysis completes.
/// 从 job artifacts 预热 facade cache。返回缓存质量元数据。
fn try_warm_facade_cache_from_result(
    root: &str,
    language: &str,
    result: &SerializableResult,
) -> Result<crate::mcp_server::WarmCacheMeta, String> {
    let slot = MCP_FACADE_CACHE
        .get()
        .ok_or_else(|| "facade cache warmer not registered".to_string())?;
    let slot = slot
        .lock()
        .map_err(|_| "facade cache warmer lock poisoned".to_string())?;
    slot.inner.warm_from_result(root, language, result)
}

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
    cancellation_flags: Mutex<HashMap<String, Arc<AtomicBool>>>,
}

fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn is_terminal_status(status: JobStatus) -> bool {
    matches!(
        status,
        JobStatus::Succeeded | JobStatus::Failed | JobStatus::Cancelled
    )
}

fn job_status_str(status: JobStatus) -> &'static str {
    match status {
        JobStatus::Queued => "queued",
        JobStatus::Running => "running",
        JobStatus::Succeeded => "succeeded",
        JobStatus::Failed => "failed",
        JobStatus::Cancelled => "cancelled",
    }
}

impl McpJobRegistry {
    pub fn new() -> Self {
        Self {
            jobs: Mutex::new(HashMap::new()),
            results: Mutex::new(HashMap::new()),
            detail_pages: Mutex::new(HashMap::new()),
            next_id: AtomicU64::new(1),
            active_analysis_count: AtomicUsize::new(0),
            cancellation_flags: Mutex::new(HashMap::new()),
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

        let now = now_ms();
        let job_id = self.next_job_id();
        {
            let mut flags = self.cancellation_flags.lock().unwrap();
            flags.insert(job_id.clone(), Arc::new(AtomicBool::new(false)));
        }
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
                if is_terminal_status(j.status) && j.status != status {
                    return;
                }
                j.status = status;
                if is_terminal_status(status) {
                    j.completed_at_ms = Some(now_ms());
                }
            }
        }
    }

    pub fn update_progress(&self, job_id: &str, progress: McpJobProgress) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(j) = jobs.get_mut(job_id) {
                if is_terminal_status(j.status) {
                    return;
                }
                j.progress = Some(progress);
                j.status = JobStatus::Running;
            }
        }
    }

    pub fn set_summary(&self, job_id: &str, summary: Value) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(j) = jobs.get_mut(job_id) {
                if is_terminal_status(j.status) {
                    return;
                }
                j.summary = Some(summary);
                j.status = JobStatus::Succeeded;
                j.completed_at_ms = Some(now_ms());
            }
        }
    }

    pub fn set_error(&self, job_id: &str, error: String) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(j) = jobs.get_mut(job_id) {
                if is_terminal_status(j.status) {
                    return;
                }
                j.error = Some(error);
                j.status = JobStatus::Failed;
                j.completed_at_ms = Some(now_ms());
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
        let items = self
            .detail_pages
            .lock()
            .ok()
            .and_then(|dp| dp.get(job_id).cloned())
            .unwrap_or_default();
        let total = items.len();
        let page_size = page_size.clamp(1, 200);
        let total_pages = if total == 0 {
            1
        } else {
            (total + page_size - 1) / page_size
        };
        let start = page.saturating_mul(page_size);
        let end = (start + page_size).min(total);
        let page_items: Vec<Value> = if start >= total {
            vec![]
        } else {
            items[start..end].to_vec()
        };
        let partial = matches!(handle.status, JobStatus::Queued | JobStatus::Running);

        Some(serde_json::json!({
            "schemaVersion": "codelattice.pagedDetail.v1",
            "jobId": job_id,
            "status": job_status_str(handle.status),
            "partial": partial,
            "page": page,
            "pageSize": page_size,
            "totalItems": total,
            "totalPages": total_pages,
            "hasMore": page + 1 < total_pages,
            "hasPrev": page > 0,
            "items": page_items,
            "progress": handle.progress,
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

    pub fn submit_queue(&self, root: &str, language: &str, mode: &str) -> Value {
        let handle = self.submit(root, language, mode);
        // If it's a new queued job (not singleflight), mark it as queued
        if !handle.reused_existing_job {
            let job_id = handle.job_id.clone();
            self.update_status(&job_id, JobStatus::Queued);
            // Return queued response with position info
            let active = self.active_analysis_count();
            let running = self.running_jobs_info();
            return serde_json::json!({
                "schemaVersion": "codelattice.queuedJob.v1",
                "jobId": job_id,
                "status": "queued",
                "queuePosition": running.len() + 1,
                "activeAnalysisCount": active + 1,
                "root": root,
                "language": language,
                "message": format!("Analysis job queued ({} active job(s)). Poll job_status until running.", active),
                "recommendedNextCalls": [
                    {"tool": "codelattice_project", "mode": "job_status", "arguments": {"jobId": job_id}},
                    {"tool": "codelattice_project", "mode": "job_cancel", "arguments": {"jobId": job_id}}
                ]
            });
        }
        self.to_response(&handle)
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

    pub fn cancel_job(&self, job_id: &str) -> Value {
        let handle = match self.get(job_id) {
            Some(h) => h,
            None => {
                return serde_json::json!({
                    "schemaVersion": "codelattice.cancelJob.v1",
                    "jobId": job_id, "cancelled": false,
                    "reason": "Job not found",
                    "analysisSemantics": {
                        "staticAnalysisExecuted": false, "targetCodeExecuted": false,
                        "targetScriptsExecuted": false, "runtimeProof": false, "coverageProof": false
                    }
                })
            }
        };
        let (cancelled, reason) = match handle.status {
            JobStatus::Succeeded | JobStatus::Failed | JobStatus::Cancelled => (
                false,
                format!("Job already in terminal state: {:?}", handle.status),
            ),
            _ => {
                if let Ok(flags) = self.cancellation_flags.lock() {
                    if let Some(flag) = flags.get(job_id) {
                        flag.store(true, Ordering::SeqCst);
                    }
                }
                self.update_status(job_id, JobStatus::Cancelled);
                (true, "Job cancelled by user request".to_string())
            }
        };
        serde_json::json!({
            "schemaVersion": "codelattice.cancelJob.v1",
            "jobId": job_id, "cancelled": cancelled, "reason": reason,
            "status": match self.get(job_id).map(|h| h.status) {
                Some(JobStatus::Cancelled) => "cancelled", Some(JobStatus::Succeeded) => "succeeded",
                Some(JobStatus::Failed) => "failed", Some(JobStatus::Running) => "running",
                Some(JobStatus::Queued) => "queued", _ => "unknown",
            },
            "analysisSemantics": {
                "staticAnalysisExecuted": true, "targetCodeExecuted": false,
                "targetScriptsExecuted": false, "runtimeProof": false, "coverageProof": false
            }
        })
    }

    pub fn is_cancelled(&self, job_id: &str) -> bool {
        if let Ok(flags) = self.cancellation_flags.lock() {
            if let Some(flag) = flags.get(job_id) {
                return flag.load(Ordering::SeqCst);
            }
        }
        false
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

    /// 列出匹配 root/language 的已 succeeded job，用于闭环检查
    pub fn list_succeeded_jobs(&self, root: &str, language: &str) -> Vec<McpJobHandle> {
        let jobs = self.jobs.lock().unwrap();
        jobs.values()
            .filter(|j| {
                j.status == JobStatus::Succeeded
                    && j.root == root
                    && (j.language == language || j.language == "auto")
            })
            .cloned()
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
            "progress": handle.progress.as_ref().map(|p| {
                let observed_end_ms = handle.completed_at_ms.unwrap_or_else(|| {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_millis() as u64
                });
                let wall_ms = std::cmp::max(
                    p.elapsed_ms,
                    observed_end_ms.saturating_sub(handle.created_at_ms),
                );
                serde_json::json!({
                    "stage": p.stage, "completedUnits": p.completed_units,
                    "totalUnits": p.total_units, "failedUnits": p.failed_units,
                    "elapsedMs": wall_ms, "executorMode": p.executor_mode,
                    "cacheHits": p.cache_hits, "cacheMisses": p.cache_misses,
                    "engineElapsedMs": p.elapsed_ms,
                    "wallClockMs": wall_ms,
                })
            }),
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

fn effective_persistent_cache_dir() -> Option<std::path::PathBuf> {
    if std::env::var("CODELATTICE_CACHE").as_deref() == Ok("off") {
        return None;
    }
    if let Ok(custom) = std::env::var("CODELATTICE_CACHE_DIR") {
        return Some(std::path::PathBuf::from(custom));
    }
    let home = std::env::var("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    Some(home.join(".cache").join("codelattice"))
}

pub static ENGINE_CACHE: std::sync::LazyLock<std::sync::Mutex<ArtifactCache>> =
    std::sync::LazyLock::new(|| {
        let cache_dir = effective_persistent_cache_dir();
        std::sync::Mutex::new(ArtifactCache::new(cache_dir))
    });

pub fn cache_stats() -> serde_json::Value {
    if let Ok(cache) = ENGINE_CACHE.lock() {
        let (hits, misses, stale, rebuilt) = cache.stats();
        let cache_dir = effective_persistent_cache_dir().map(|p| p.display().to_string());
        serde_json::json!({
            "enabled": true,
            "persistent": cache.persistent_enabled(),
            "cacheDir": cache_dir,
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

    // warm 阶段可能调用 run_rust_analysis（进程内完整 project-model 分析），
    // 大项目（43K+ nodes）会深度递归，需要更大的栈空间以避免 stack overflow。
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024) // 16 MB
        .spawn(move || {
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

        if MCP_JOBS.is_cancelled(&job_id) {
            return;
        }
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

        if MCP_JOBS.is_cancelled(&job_id) {
            return;
        }
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
            // 逐 task 执行，每完成一个更新 progress，避免 completedUnits 长期为 0
            let start = std::time::Instant::now();
            let mut artifacts = Vec::new();
            let mut completed = 0usize;
            let mut failed = 0usize;
            let mut stage_times = std::collections::HashMap::new();
            let total = plan.total_tasks;
            for (idx, task) in plan.tasks.iter().enumerate() {
                if MCP_JOBS.is_cancelled(&job_id) {
                    break;
                }
                match gitnexus_analysis_engine::executor::run_single_task_public(
                    task,
                    adapter.as_ref(),
                ) {
                    Ok(art) => {
                        *stage_times
                            .entry(task.stage.name().to_string())
                            .or_insert(0u64) += art.duration_ms;
                        artifacts.push(art);
                        completed += 1;
                    }
                    Err(e) => {
                        artifacts.push(
                            gitnexus_analysis_engine::executor::make_error_artifact_public(task, e),
                        );
                        failed += 1;
                    }
                }
                // 每完成一个 task 更新 progress（至少每 10 个 task 更新一次，减少锁争用）
                if idx % 10 == 0 || idx == total - 1 {
                    let elapsed = start.elapsed().as_millis() as u64;
                    MCP_JOBS.update_progress(
                        &job_id,
                        McpJobProgress {
                            stage: task.stage.name().to_string(),
                            completed_units: completed + failed,
                            total_units: total,
                            failed_units: failed,
                            elapsed_ms: elapsed,
                            executor_mode: "serial".into(),
                            cache_hits: 0,
                            cache_misses: 0,
                        },
                    );
                }
            }
            gitnexus_analysis_engine::executor::SerializableResult {
                total_tasks: total,
                completed,
                failed,
                total_duration_ms: start.elapsed().as_millis() as u64,
                artifacts,
                stage_times,
                executor_mode: "serial".into(),
            }
        };

        let mut summary = serde_json::json!({
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

        // P1: 生成 compact AI digest，提高 job_detail 信息密度
        let mut ai_digest = serde_json::json!({
            "sourceFileCount": nf,
            "symbolCount": 0u64,
            "failedUnits": result.failed,
            "callEdgeCount": 0u64,
        });
        // 从 symbol artifacts 提取 top symbols
        let mut all_symbols: Vec<Value> = Vec::new();
        let mut file_symbol_counts: HashMap<String, usize> = HashMap::new();
        for art in &result.artifacts {
            if art.stage == gitnexus_analysis_engine::dag::AnalysisStage::Symbol
                && art.error.is_none()
            {
                if let Some(syms) = art.data.get("symbols").and_then(|s| s.as_array()) {
                    let file_id = art
                        .data
                        .get("unitId")
                        .or_else(|| art.data.get("unit_id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or(&art.unit_id);
                    *file_symbol_counts.entry(file_id.to_string()).or_insert(0) += syms.len();
                    for sym in syms.iter().take(50) {
                        all_symbols.push(sym.clone());
                    }
                }
            }
            if art.stage == gitnexus_analysis_engine::dag::AnalysisStage::Reference
                && art.error.is_none()
            {
                if let Some(calls) = art.data.get("calls").and_then(|c| c.as_array()) {
                    if let Some(obj) = ai_digest.as_object_mut() {
                        obj.insert("callEdgeCount".to_string(), json!(calls.len() as u64));
                    }
                }
            }
        }
        if let Some(obj) = ai_digest.as_object_mut() {
            obj.insert("symbolCount".to_string(), json!(all_symbols.len() as u64));
        }
        // top files by symbol count
        let mut top_files: Vec<(&String, &usize)> = file_symbol_counts.iter().collect();
        top_files.sort_by(|a, b| b.1.cmp(a.1));
        if let Some(obj) = ai_digest.as_object_mut() {
            obj.insert(
                "topFiles".to_string(),
                json!(top_files
                    .iter()
                    .take(10)
                    .map(|(f, c)| json!({
                        "file": f, "symbolCount": c
                    }))
                    .collect::<Vec<_>>()),
            );
        }
        // top symbols（按 kind 分组计数）
        let kind_counts: HashMap<String, usize> = all_symbols
            .iter()
            .filter_map(|s| {
                s.get("kind")
                    .and_then(|k| k.as_str())
                    .map(|k| k.to_string())
            })
            .fold(HashMap::new(), |mut acc, k| {
                *acc.entry(k).or_insert(0) += 1;
                acc
            });
        if let Some(obj) = ai_digest.as_object_mut() {
            obj.insert("symbolKinds".to_string(), json!(kind_counts));
            // 只保留前 20 个符号名称作为预览
            let preview: Vec<Value> = all_symbols
                .iter()
                .take(20)
                .filter_map(|s| {
                    let name = s.get("name").and_then(|n| n.as_str())?;
                    let kind = s.get("kind").and_then(|k| k.as_str()).unwrap_or("?");
                    Some(json!({"name": name, "kind": kind}))
                })
                .collect();
            obj.insert("topSymbolsPreview".to_string(), json!(preview));
        }
        let engine_digest = ai_digest.clone();

        MCP_JOBS.store_detail(&job_id, detail_items);
        // 在 job summary 中先嵌入 engine digest；facade warm 成功后会用真实 GraphView digest 覆盖 aiDigest。
        if let Some(obj) = summary.as_object_mut() {
            obj.insert("engineDigest".to_string(), engine_digest);
            obj.insert("aiDigest".to_string(), ai_digest);
        }

        MCP_JOBS.update_progress(
            &job_id,
            McpJobProgress {
                stage: "warming_facade_cache".into(),
                completed_units: result.completed,
                total_units: result.total_tasks,
                failed_units: result.failed,
                elapsed_ms: result.total_duration_ms,
                executor_mode: result.executor_mode.clone(),
                cache_hits: 0,
                cache_misses: 0,
            },
        );

        // 闭环：只有 facade 图谱缓存完成预热后（且有真实符号），job 才能对外宣称 succeeded。
        let warm_result = try_warm_facade_cache_from_result(&root_owned, &lang_owned, &result);

        // 从 warm_from_result 获取真实缓存质量数据
        let facade_symbol_count = warm_result.as_ref().map(|m| m.symbol_count).unwrap_or(0);
        let facade_call_edge_count = warm_result
            .as_ref()
            .map(|m| m.call_edge_count)
            .unwrap_or(0);
        let facade_warm_duration_ms = warm_result
            .as_ref()
            .map(|m| m.warm_duration_ms)
            .unwrap_or(0);
        let used_cli_fallback = warm_result
            .as_ref()
            .map(|m| m.used_cli_fallback)
            .unwrap_or(false);
        let facade_cache_ready = warm_result.is_ok() && facade_symbol_count > 0;
        let job_wall_clock_ms = now_ms().saturating_sub(handle.created_at_ms);

        if let Some(obj) = summary.as_object_mut() {
            obj.insert(
                "facadeCacheReady".to_string(),
                serde_json::json!(facade_cache_ready),
            );
            obj.insert(
                "facadeCacheWarmStatus".to_string(),
                serde_json::json!(if facade_cache_ready {
                    if used_cli_fallback {
                        "ready_via_cli_fallback"
                    } else {
                        "ready"
                    }
                } else if warm_result.is_err() {
                    "warm_failed"
                } else {
                    "warm_ok_empty_graph"
                }),
            );
            if let Err(e) = &warm_result {
                obj.insert("facadeCacheWarmError".to_string(), serde_json::json!(e));
            }
            obj.insert("facadeWarmDurationMs".to_string(), json!(facade_warm_duration_ms));
            obj.insert("wallClockMs".to_string(), json!(job_wall_clock_ms));
            if let Ok(meta) = warm_result.as_ref() {
                let mut facade_digest = meta.facade_digest.clone();
                if let Some(digest_obj) = facade_digest.as_object_mut() {
                    digest_obj.insert("facadeSymbolCount".to_string(), json!(facade_symbol_count));
                    digest_obj.insert("facadeCacheReady".to_string(), json!(facade_cache_ready));
                    digest_obj.insert("usedCliFallback".to_string(), json!(used_cli_fallback));
                    digest_obj.insert("callEdgeCount".to_string(), json!(facade_call_edge_count));
                }
                obj.insert("facadeDigest".to_string(), facade_digest.clone());
                obj.insert("aiDigest".to_string(), facade_digest);
                let warm_trace = serde_json::to_value(&meta.warm_trace).unwrap_or_default();
                obj.insert("warmTrace".to_string(), warm_trace);
            } else if let Some(digest) = obj.get_mut("aiDigest") {
                if let Some(digest_obj) = digest.as_object_mut() {
                    digest_obj.insert("facadeSymbolCount".to_string(), json!(facade_symbol_count));
                    digest_obj.insert("facadeCacheReady".to_string(), json!(facade_cache_ready));
                    digest_obj.insert("usedCliFallback".to_string(), json!(used_cli_fallback));
                }
            }
        }
        MCP_JOBS.update_progress(
            &job_id,
            McpJobProgress {
                stage: if facade_cache_ready {
                    "facade_cache_ready".into()
                } else {
                    "warming_facade_cache".into()
                },
                completed_units: result.completed,
                total_units: result.total_tasks,
                failed_units: result.failed,
                elapsed_ms: job_wall_clock_ms,
                executor_mode: result.executor_mode.clone(),
                cache_hits: 0,
                cache_misses: 0,
            },
        );
        MCP_JOBS.set_summary(&job_id, summary);
    }).expect("workspace job thread spawn failed");

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

/// 获取 job 状态
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

    // workspace job 的 warm 阶段也可能触发 run_rust_analysis，需要大栈
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
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

        let mut summary = serde_json::json!({
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

        MCP_JOBS.update_progress(
            &job_id,
            McpJobProgress {
                stage: "warming_facade_cache".into(),
                completed_units: completed + failed,
                total_units: supported.len(),
                failed_units: failed,
                elapsed_ms: 0,
                executor_mode: mode_owned.clone(),
                cache_hits: 0,
                cache_misses: 0,
            },
        );

        // workspace 闭环：预热 facade 图谱缓存（workspace 无单一 result，暂用慢路径）
        let warm_result = try_warm_facade_cache(&root_owned, "auto");
        if let Some(obj) = summary.as_object_mut() {
            obj.insert(
                "facadeCacheReady".to_string(),
                serde_json::json!(warm_result.is_ok()),
            );
            obj.insert(
                "facadeCacheWarmStatus".to_string(),
                serde_json::json!(if warm_result.is_ok() {
                    "ready"
                } else {
                    "failed"
                }),
            );
            if let Err(e) = &warm_result {
                obj.insert("facadeCacheWarmError".to_string(), serde_json::json!(e));
            }
        }
        MCP_JOBS.set_summary(&job_id, summary);
    }).expect("project job thread spawn failed");

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

#[cfg(test)]
mod tests {
    use super::*;

    fn progress(stage: &str) -> McpJobProgress {
        McpJobProgress {
            stage: stage.to_string(),
            completed_units: 0,
            total_units: 10,
            failed_units: 0,
            elapsed_ms: 0,
            executor_mode: "serial".to_string(),
            cache_hits: 0,
            cache_misses: 0,
        }
    }

    #[test]
    fn cancelled_job_ignores_late_worker_updates() {
        let registry = McpJobRegistry::new();
        let job = registry.submit("/tmp/codelattice-cancel", "rust", "serial");
        let job_id = job.job_id;

        let cancel = registry.cancel_job(&job_id);
        assert_eq!(cancel["cancelled"], true);

        registry.update_progress(&job_id, progress("late-progress"));
        registry.set_summary(&job_id, serde_json::json!({"late": "summary"}));
        registry.set_error(&job_id, "late error".to_string());

        let handle = registry.get(&job_id).expect("job should still exist");
        assert_eq!(handle.status, JobStatus::Cancelled);
        assert!(
            handle.summary.is_none(),
            "late worker summary must not overwrite a cancelled job"
        );
        assert!(
            handle.error.is_none(),
            "late worker error must not overwrite a cancelled job"
        );
    }

    #[test]
    fn running_detail_page_returns_partial_without_items() {
        let registry = McpJobRegistry::new();
        let job = registry.submit("/tmp/codelattice-partial", "rust", "serial");
        let job_id = job.job_id;
        registry.update_progress(&job_id, progress("executing"));

        let detail = registry
            .get_detail_page(&job_id, 0, 50)
            .expect("running job should expose a partial detail page");

        assert_eq!(detail["schemaVersion"], "codelattice.pagedDetail.v1");
        assert_eq!(detail["jobId"], job_id);
        assert_eq!(detail["status"], "running");
        assert_eq!(detail["partial"], true);
        assert_eq!(detail["totalItems"], 0);
        assert!(detail["items"]
            .as_array()
            .is_some_and(|items| items.is_empty()));
        assert_eq!(detail["progress"]["stage"], "executing");
    }
}
