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

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::Path;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, OnceLock};

use serde_json::{json, Value};

use gitnexus_analysis_engine::cache::{ArtifactCache, CacheKey};
use gitnexus_analysis_engine::dag::{AnalysisArtifact, AnalysisStage, ArtifactSemantics};
use gitnexus_analysis_engine::executor::SerializableResult;
use gitnexus_analysis_engine::job::JobStatus;
use gitnexus_workspace_model::ProjectInfo;

const MAX_CONCURRENT_ANALYSIS_JOBS: usize = 2;

fn max_concurrent_analysis_jobs() -> usize {
    std::env::var("CODELATTICE_MCP_MAX_ANALYSIS_JOBS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(MAX_CONCURRENT_ANALYSIS_JOBS)
}

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

/// facade 图谱缓存预热 trait：job worker 完成后直接从 job result 灌入缓存。
pub trait FacadeCacheWarmer {
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
    queued_jobs: Mutex<VecDeque<String>>,
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
            queued_jobs: Mutex::new(VecDeque::new()),
        }
    }

    fn next_job_id(&self) -> String {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        format!("job_engine_{:08x}", id)
    }

    fn dedup_key(root: &str, language: &str, mode: &str) -> String {
        format!("{}:{}:{}", root, language, mode)
    }

    pub fn enqueue_job(&self, job_id: &str) {
        if let Ok(mut queue) = self.queued_jobs.lock() {
            if !queue.iter().any(|id| id == job_id) {
                queue.push_back(job_id.to_string());
            }
        }
    }

    pub fn remove_from_queue(&self, job_id: &str) {
        if let Ok(mut queue) = self.queued_jobs.lock() {
            queue.retain(|id| id != job_id);
        }
    }

    pub fn queue_position(&self, job_id: &str) -> Option<usize> {
        let queue = self.queued_jobs.lock().ok()?;
        queue.iter().position(|id| id == job_id)
    }

    pub fn dequeue_next_eligible(&self) -> Option<String> {
        let mut queue = self.queued_jobs.lock().ok()?;
        while let Some(job_id) = queue.pop_front() {
            let handle = match self.get(&job_id) {
                Some(h) => h,
                None => continue,
            };
            if !matches!(handle.status, JobStatus::Cancelled) {
                return Some(job_id);
            }
        }
        None
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
                let keep_queued = progress.stage == "queued" && j.status == JobStatus::Queued;
                j.progress = Some(progress);
                if !keep_queued {
                    j.status = JobStatus::Running;
                }
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
            self.enqueue_job(&job_id);
            self.update_status(&job_id, JobStatus::Queued);
            // Return queued response with position info
            let active = self.active_analysis_count();
            return serde_json::json!({
                "schemaVersion": "codelattice.queuedJob.v1",
                "jobId": job_id,
                "status": "queued",
                "queuePosition": self.queue_position(&job_id).map(|p| p + 1).unwrap_or(1),
                "activeAnalysisCount": active,
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
        let _ = self.active_analysis_count.fetch_update(
            Ordering::SeqCst,
            Ordering::SeqCst,
            |current| Some(current.saturating_sub(1)),
        );
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
                self.remove_from_queue(job_id);
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

    /// 短轮询等待 job 完成，直到终态或超时。
    /// timeout_ms 最大 clamp 到 30000ms。
    /// 返回 (终态时的 job status Value, 是否超时)
    pub fn wait_for_job(&self, job_id: &str, timeout_ms: u64) -> (Option<Value>, bool) {
        let clamped = timeout_ms.clamp(0, 30000);
        if clamped == 0 {
            return (self.get_status_value(job_id), false);
        }
        let start = std::time::Instant::now();
        let poll_interval = std::time::Duration::from_millis(200);
        loop {
            if let Some(handle) = self.get(job_id) {
                if is_terminal_status(handle.status) {
                    return (self.get_status_value(job_id), false);
                }
            } else {
                // job 不存在
                return (None, false);
            }
            if start.elapsed().as_millis() as u64 >= clamped {
                return (self.get_status_value(job_id), true);
            }
            std::thread::sleep(poll_interval);
        }
    }

    fn get_status_value(&self, job_id: &str) -> Option<Value> {
        // 使用本地 registry 而非全局 MCP_JOBS，确保测试隔离
        self.to_response_json(job_id)
    }

    /// 获取 job 的 status JSON（从本地 registry）
    fn to_response_json(&self, job_id: &str) -> Option<Value> {
        let handle = self.get(job_id)?;
        let mut resp = serde_json::json!({
            "schemaVersion": "codelattice.jobStatus.v1",
            "jobId": job_id,
            "status": job_status_str(handle.status),
            "root": handle.root,
            "language": handle.language,
            "progress": handle.progress,
            "summary": handle.summary,
            "deduped": handle.reused_existing_job,
        });
        if handle.reused_existing_job {
            if let Some(obj) = resp.as_object_mut() {
                obj.insert("deduped".to_string(), Value::Bool(true));
            }
        }
        Some(resp)
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
    let created_at_ms = handle.created_at_ms;

    if MCP_JOBS.active_analysis_count() >= max_concurrent_analysis_jobs() {
        MCP_JOBS.enqueue_job(&job_id);
        MCP_JOBS.update_status(&job_id, JobStatus::Queued);
        MCP_JOBS.update_progress(
            &job_id,
            McpJobProgress {
                stage: "queued".into(),
                completed_units: 0,
                total_units: 1,
                failed_units: 0,
                elapsed_ms: 0,
                executor_mode: "project-once".into(),
                cache_hits: 0,
                cache_misses: 0,
            },
        );
        let handle = MCP_JOBS.get(&job_id).unwrap_or(handle);
        return Ok(MCP_JOBS.to_response(&handle));
    }

    spawn_project_job_worker(
        job_id.clone(),
        root_owned,
        lang_owned,
        mode_owned,
        created_at_ms,
    )?;
    let handle = MCP_JOBS.get(&job_id).unwrap();
    Ok(MCP_JOBS.to_response(&handle))
}

fn spawn_project_job_worker(
    job_id: String,
    root_owned: String,
    lang_owned: String,
    mode_owned: String,
    created_at_ms: u64,
) -> Result<(), String> {
    MCP_JOBS.update_status(&job_id, JobStatus::Running);
    MCP_JOBS.increment_active_analysis();
    MCP_JOBS.update_progress(
        &job_id,
        McpJobProgress {
            stage: "starting".into(),
            completed_units: 0,
            total_units: 1,
            failed_units: 0,
            elapsed_ms: 0,
            executor_mode: "project-once".into(),
            cache_hits: 0,
            cache_misses: 0,
        },
    );

    // warm 阶段可能调用 run_rust_analysis（进程内完整 project-model 分析），
    // 大项目（43K+ nodes）会深度递归，需要更大的栈空间以避免 stack overflow。
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024) // 16 MB
        .spawn(move || {
        let _guard = ActiveAnalysisGuard;

        if let Some(delay_ms) = std::env::var("CODELATTICE_MCP_TEST_JOB_DELAY_MS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
        {
            std::thread::sleep(std::time::Duration::from_millis(delay_ms));
        }

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
                total_units: 1,
                failed_units: 0,
                elapsed_ms: 0,
                executor_mode: "project-once".into(),
                cache_hits: 0,
                cache_misses: 0,
            },
        );

        if MCP_JOBS.is_cancelled(&job_id) {
            return;
        }

        MCP_JOBS.update_progress(
            &job_id,
            McpJobProgress {
                stage: "executing".into(),
                completed_units: 0,
                total_units: 1,
                failed_units: 0,
                elapsed_ms: 0,
                executor_mode: "project-once".into(),
                cache_hits: 0,
                cache_misses: 0,
            },
        );

        let start = std::time::Instant::now();
        let project_run =
            match crate::engine_bridge::run_project_analysis_once(Path::new(&root_owned), &lang_owned)
            {
                Ok(run) => run,
                Err(e) => {
                    MCP_JOBS.set_error(&job_id, format!("project analysis failed: {}", e));
                    return;
                }
            };
        let duration_ms = start.elapsed().as_millis() as u64;

        if MCP_JOBS.is_cancelled(&job_id) {
            return;
        };

        let artifact = AnalysisArtifact {
            schema_version: "1.0".into(),
            task_id: format!("project:{}:{}", lang_owned, mode_owned),
            stage: AnalysisStage::Merge,
            language: lang_owned.clone(),
            unit_id: "__project__".into(),
            cache_key: None,
            data: json!({
                "analyzeValue": project_run.analyze_value,
                "nodeCount": project_run.node_count,
                "edgeCount": project_run.edge_count,
                "symbolCount": project_run.symbol_count,
                "callEdgeCount": project_run.call_edge_count,
                "sourceFileCount": nf,
                "analysisTrace": project_run.analysis_trace,
            }),
            error: None,
            duration_ms,
            generated_from: ArtifactSemantics::default(),
        };
        let mut stage_times = std::collections::HashMap::new();
        stage_times.insert("project".to_string(), duration_ms);
        let result = SerializableResult {
            total_tasks: 1,
            completed: 1,
            failed: 0,
            total_duration_ms: duration_ms,
            artifacts: vec![artifact],
            stage_times,
            executor_mode: "project-once".into(),
        };

        MCP_JOBS.update_progress(
            &job_id,
            McpJobProgress {
                stage: "project".into(),
                completed_units: 1,
                total_units: 1,
                failed_units: 0,
                elapsed_ms: duration_ms,
                executor_mode: "project-once".into(),
                cache_hits: 0,
                cache_misses: 0,
            },
        );

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

        let key = CacheKey {
            path: "__project__".into(),
            content_hash: format!("{:x}", result.total_duration_ms),
            language: lang_owned.clone(),
            adapter_version: "1.3".into(),
            parser_version: "project-once".into(),
            stage: AnalysisStage::Merge.name().to_string(),
            engine_version: "1.3".into(),
        };
        cache_store(key, result.artifacts[0].clone());

        let detail_items: Vec<Value> = result.artifacts.iter().map(|a| serde_json::json!({
            "taskId": a.task_id, "stage": a.stage.name(), "unitId": a.unit_id,
            "error": a.error, "durationMs": a.duration_ms,
            "symbolCount": a.data.get("symbolCount").and_then(|v| v.as_u64()).unwrap_or(0),
            "callEdgeCount": a.data.get("callEdgeCount").and_then(|v| v.as_u64()).unwrap_or(0),
            "nodeCount": a.data.get("nodeCount").and_then(|v| v.as_u64()).unwrap_or(0),
            "edgeCount": a.data.get("edgeCount").and_then(|v| v.as_u64()).unwrap_or(0),
            "executorMode": "project-once",
        })).collect();

        // P1: 生成 compact AI digest，提高 job_detail 信息密度
        let ai_digest = serde_json::json!({
            "sourceFileCount": nf,
            "symbolCount": result.artifacts[0].data.get("symbolCount").and_then(|v| v.as_u64()).unwrap_or(0),
            "failedUnits": result.failed,
            "callEdgeCount": result.artifacts[0].data.get("callEdgeCount").and_then(|v| v.as_u64()).unwrap_or(0),
            "nodeCount": result.artifacts[0].data.get("nodeCount").and_then(|v| v.as_u64()).unwrap_or(0),
            "edgeCount": result.artifacts[0].data.get("edgeCount").and_then(|v| v.as_u64()).unwrap_or(0),
            "executorMode": "project-once",
        });
        let engine_digest = ai_digest.clone();

        MCP_JOBS.store_detail(&job_id, detail_items);
        // 在 job summary 中先嵌入 engine digest；facade warm 成功后会用真实 GraphView digest 覆盖 aiDigest。
        if let Some(obj) = summary.as_object_mut() {
            obj.insert("engineDigest".to_string(), engine_digest);
            obj.insert("aiDigest".to_string(), ai_digest);
            if let Some(trace) = result.artifacts[0]
                .data
                .get("analysisTrace")
                .filter(|v| !v.is_null())
            {
                obj.insert("analysisTrace".to_string(), trace.clone());
            }
            obj.insert(
                "runtimeCapabilities".to_string(),
                crate::mcp_facade::facade_language_runtime_capabilities(&lang_owned),
            );
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
        let job_wall_clock_ms = now_ms().saturating_sub(created_at_ms);

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
    }).map_err(|e| format!("project job thread spawn failed: {e}"))?;

    Ok(())
}

/// RAII guard：worker thread 结束时自动递减 active_analysis_count
struct ActiveAnalysisGuard;

impl Drop for ActiveAnalysisGuard {
    fn drop(&mut self) {
        MCP_JOBS.decrement_active_analysis();
        start_next_queued_jobs();
    }
}

fn start_next_queued_jobs() {
    loop {
        if MCP_JOBS.active_analysis_count() >= max_concurrent_analysis_jobs() {
            return;
        }
        let Some(job_id) = MCP_JOBS.dequeue_next_eligible() else {
            return;
        };
        let Some(handle) = MCP_JOBS.get(&job_id) else {
            continue;
        };
        if !matches!(handle.status, JobStatus::Queued) {
            continue;
        }

        let started = if handle.language == "auto" {
            spawn_workspace_job_worker(
                handle.job_id.clone(),
                handle.root.clone(),
                handle.mode.clone(),
                handle.created_at_ms,
            )
        } else {
            spawn_project_job_worker(
                handle.job_id.clone(),
                handle.root.clone(),
                handle.language.clone(),
                handle.mode.clone(),
                handle.created_at_ms,
            )
        };

        if let Err(e) = started {
            MCP_JOBS.set_error(&job_id, e);
        }
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
    let created_at_ms = handle.created_at_ms;

    if MCP_JOBS.active_analysis_count() >= max_concurrent_analysis_jobs() {
        MCP_JOBS.enqueue_job(&job_id);
        MCP_JOBS.update_status(&job_id, JobStatus::Queued);
        MCP_JOBS.update_progress(
            &job_id,
            McpJobProgress {
                stage: "queued".into(),
                completed_units: 0,
                total_units: 1,
                failed_units: 0,
                elapsed_ms: 0,
                executor_mode: mode_owned.clone(),
                cache_hits: 0,
                cache_misses: 0,
            },
        );
        let handle = MCP_JOBS.get(&job_id).unwrap_or(handle);
        return Ok(MCP_JOBS.to_response(&handle));
    }

    spawn_workspace_job_worker(job_id.clone(), root_owned, mode_owned, created_at_ms)?;
    let handle = MCP_JOBS.get(&job_id).unwrap();
    Ok(MCP_JOBS.to_response(&handle))
}

fn spawn_workspace_job_worker(
    job_id: String,
    root_owned: String,
    mode_owned: String,
    _created_at_ms: u64,
) -> Result<(), String> {
    MCP_JOBS.update_status(&job_id, JobStatus::Running);
    MCP_JOBS.increment_active_analysis();
    MCP_JOBS.update_progress(
        &job_id,
        McpJobProgress {
            stage: "starting".into(),
            completed_units: 0,
            total_units: 1,
            failed_units: 0,
            elapsed_ms: 0,
            executor_mode: mode_owned.clone(),
            cache_hits: 0,
            cache_misses: 0,
        },
    );

    // workspace job 会在内部按项目运行 project-once analyzer。每个项目 worker
    // 都使用大栈，避免 Rust 大图分析递归时栈溢出。
    std::thread::Builder::new()
        .stack_size(16 * 1024 * 1024)
        .spawn(move || {
            let _guard = ActiveAnalysisGuard;

            let inventory =
                match gitnexus_workspace_model::scan_workspace_inventory(Path::new(&root_owned), true)
                {
                    Ok(inv) => inv,
                    Err(e) => {
                        MCP_JOBS.set_error(&job_id, format!("Workspace scan: {}", e));
                        return;
                    }
                };

            let supported: Vec<ProjectInfo> = inventory
                .iter()
                .filter(|p| p.supported && !p.path.is_empty())
                .cloned()
                .collect();
            let unsupported: Vec<ProjectInfo> =
                inventory.iter().filter(|p| !p.supported).cloned().collect();
            let manifest_projects: Vec<ProjectInfo> = supported
                .iter()
                .filter(|p| p.is_manifest_backed)
                .cloned()
                .collect();
            let source_only_projects: Vec<ProjectInfo> = supported
                .iter()
                .filter(|p| !p.is_manifest_backed)
                .cloned()
                .collect();

            let (selected_projects, project_selection_strategy, source_only_skipped_count) =
                if manifest_projects.is_empty() {
                    (
                        source_only_projects.clone(),
                        "source_only_fallback",
                        0usize,
                    )
                } else {
                    (
                        manifest_projects.clone(),
                        "manifest_backed",
                        source_only_projects.len(),
                    )
                };

            if selected_projects.is_empty() {
                MCP_JOBS.set_error(&job_id, "No supported projects found".to_string());
                return;
            }

            MCP_JOBS.update_progress(
                &job_id,
                McpJobProgress {
                    stage: "analyzing".into(),
                    completed_units: 0,
                    total_units: selected_projects.len(),
                    failed_units: 0,
                    elapsed_ms: 0,
                    executor_mode: "workspace-project-once".into(),
                    cache_hits: 0,
                    cache_misses: 0,
                },
            );

            let worker_limit = std::env::var("CODELATTICE_WORKSPACE_PROJECT_FANOUT")
                .ok()
                .and_then(|v| v.parse::<usize>().ok())
                .filter(|v| *v > 0)
                .unwrap_or(2)
                .min(8);
            let mut aggregated = Vec::new();
            let mut total_symbols = 0usize;
            let mut total_call_edges = 0usize;
            let mut total_nodes = 0usize;
            let mut total_edges = 0usize;
            let mut completed = 0usize;
            let mut failed = 0usize;

            for chunk in selected_projects.chunks(worker_limit) {
                if MCP_JOBS.is_cancelled(&job_id) {
                    return;
                }

                let chunk_results: Vec<Value> = chunk
                    .iter()
                    .cloned()
                    .map(|project| {
                        let root_for_project = root_owned.clone();
                        std::thread::Builder::new()
                            .stack_size(16 * 1024 * 1024)
                            .spawn(move || {
                                analyze_workspace_project_once(&root_for_project, project)
                            })
                    })
                    .map(|spawned| match spawned {
                        Ok(handle) => match handle.join() {
                            Ok(value) => value,
                            Err(_) => json!({
                                "status": "failed",
                                "error": "workspace project worker panicked",
                                "executorMode": "project-once"
                            }),
                        },
                        Err(e) => json!({
                            "status": "failed",
                            "error": format!("workspace project worker spawn failed: {e}"),
                            "executorMode": "project-once"
                        }),
                    })
                    .collect();

                for r in &chunk_results {
                    let status = r["status"].as_str().unwrap_or("unknown");
                    if status == "completed" {
                        completed += 1;
                        total_symbols += r["symbolCount"].as_u64().unwrap_or(0) as usize;
                        total_call_edges += r["callEdgeCount"].as_u64().unwrap_or(0) as usize;
                        total_nodes += r["nodeCount"].as_u64().unwrap_or(0) as usize;
                        total_edges += r["edgeCount"].as_u64().unwrap_or(0) as usize;
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
                        total_units: selected_projects.len(),
                        failed_units: failed,
                        elapsed_ms: 0,
                        executor_mode: "workspace-project-once".into(),
                        cache_hits: 0,
                        cache_misses: 0,
                    },
                );
            }

            let summary = serde_json::json!({
                "workspace": true,
                "root": root_owned,
                "totalProjects": inventory.len(),
                "projectSelectionStrategy": project_selection_strategy,
                "boundedFanoutLimit": worker_limit,
                "supportedCount": supported.len(),
                "unsupportedCount": unsupported.len(),
                "manifestBackedProjectCount": manifest_projects.len(),
                "analyzedManifestProjectCount": if project_selection_strategy == "manifest_backed" { completed } else { 0 },
                "sourceOnlyEntryCount": source_only_projects.len(),
                "sourceOnlySkippedCount": source_only_skipped_count,
                "sourceOnlySummary": workspace_source_only_summary(&source_only_projects),
                "analyzedCount": completed,
                "failedCount": failed,
                "skippedCount": selected_projects.len().saturating_sub(completed + failed) + source_only_skipped_count,
                "totalSymbols": total_symbols,
                "totalCallEdges": total_call_edges,
                "totalNodes": total_nodes,
                "totalEdges": total_edges,
                "detailItems": aggregated.len(),
                "detailsPaged": true,
                "facadeCacheReady": false,
                "facadeCacheWarmStatus": "not_applicable_workspace_digest",
                "staticAnalysisOnly": true,
                "targetCodeExecuted": false,
            });

            MCP_JOBS.store_detail(&job_id, aggregated);
            MCP_JOBS.set_summary(&job_id, summary);
        })
        .map_err(|e| format!("workspace job thread spawn failed: {e}"))?;

    Ok(())
}

fn workspace_project_root(root: &str, project: &ProjectInfo) -> String {
    if project.path == "." {
        return root.to_string();
    }
    if project.path.starts_with('.') {
        Path::new(root)
            .join(&project.path)
            .to_string_lossy()
            .to_string()
    } else {
        project.path.clone()
    }
}

fn workspace_source_only_summary(source_only_projects: &[ProjectInfo]) -> Value {
    let mut by_language: BTreeMap<String, usize> = BTreeMap::new();
    for project in source_only_projects {
        *by_language.entry(project.language.clone()).or_insert(0) += 1;
    }
    json!({
        "total": source_only_projects.len(),
        "byLanguage": by_language
            .into_iter()
            .map(|(language, count)| json!({"language": language, "count": count}))
            .collect::<Vec<_>>(),
        "preview": source_only_projects
            .iter()
            .take(5)
            .map(|project| json!({
                "name": project.name,
                "path": project.path,
                "relativePath": project.relative_path,
                "language": project.language,
                "reason": "source files without a supported project manifest"
            }))
            .collect::<Vec<_>>()
    })
}

fn analyze_workspace_project_once(root: &str, project: ProjectInfo) -> Value {
    let project_root = workspace_project_root(root, &project);
    let language = project.language.clone();
    let project_name = project.name.clone();

    if language.is_empty() {
        return json!({
            "project": project_name,
            "path": project_root,
            "relativePath": project.relative_path,
            "language": language,
            "manifestFile": project.manifest_file,
            "manifestBacked": project.is_manifest_backed,
            "status": "failed",
            "error": "empty language",
            "executorMode": "project-once"
        });
    }

    let Some(adapter) = crate::engine_bridge::get_adapter_for_language(&language) else {
        return json!({
            "project": project_name,
            "path": project_root,
            "relativePath": project.relative_path,
            "language": language,
            "manifestFile": project.manifest_file,
            "manifestBacked": project.is_manifest_backed,
            "status": "failed",
            "error": "no project-level engine adapter",
            "executorMode": "project-once"
        });
    };

    let source_file_count = adapter
        .discover_files(&project_root)
        .map(|f| f.len())
        .unwrap_or(0);
    if source_file_count == 0 {
        return json!({
            "project": project_name,
            "path": project_root,
            "relativePath": project.relative_path,
            "language": language,
            "manifestFile": project.manifest_file,
            "manifestBacked": project.is_manifest_backed,
            "status": "completed",
            "sourceFileCount": 0,
            "symbolCount": 0,
            "callEdgeCount": 0,
            "nodeCount": 0,
            "edgeCount": 0,
            "durationMs": 0,
            "executorMode": "project-once",
            "facadeCacheReady": false,
        });
    }

    let start = std::time::Instant::now();
    let project_run = match crate::engine_bridge::run_project_analysis_once(
        Path::new(&project_root),
        &language,
    ) {
        Ok(run) => run,
        Err(e) => {
            return json!({
                "project": project_name,
                "path": project_root,
                "relativePath": project.relative_path,
                "language": language,
                "manifestFile": project.manifest_file,
                "manifestBacked": project.is_manifest_backed,
                "status": "failed",
                "error": e,
                "sourceFileCount": source_file_count,
                "executorMode": "project-once"
            });
        }
    };
    let duration_ms = start.elapsed().as_millis() as u64;

    let artifact = AnalysisArtifact {
        schema_version: "1.0".into(),
        task_id: format!("workspace-project:{}:{}", language, project.relative_path),
        stage: AnalysisStage::Merge,
        language: language.clone(),
        unit_id: "__project__".into(),
        cache_key: None,
        data: json!({
            "analyzeValue": project_run.analyze_value,
            "nodeCount": project_run.node_count,
            "edgeCount": project_run.edge_count,
            "symbolCount": project_run.symbol_count,
            "callEdgeCount": project_run.call_edge_count,
            "sourceFileCount": source_file_count,
            "analysisTrace": project_run.analysis_trace,
        }),
        error: None,
        duration_ms,
        generated_from: ArtifactSemantics::default(),
    };
    let mut stage_times = HashMap::new();
    stage_times.insert("project".to_string(), duration_ms);
    let result = SerializableResult {
        total_tasks: 1,
        completed: 1,
        failed: 0,
        total_duration_ms: duration_ms,
        artifacts: vec![artifact],
        stage_times,
        executor_mode: "project-once".into(),
    };

    let key = CacheKey {
        path: format!("workspace:{}", project.relative_path),
        content_hash: format!("{:x}", duration_ms),
        language: language.clone(),
        adapter_version: "1.3".into(),
        parser_version: "project-once".into(),
        stage: AnalysisStage::Merge.name().to_string(),
        engine_version: "1.3".into(),
    };
    cache_store(key, result.artifacts[0].clone());

    let warm_result = try_warm_facade_cache_from_result(&project_root, &language, &result);
    let facade_symbol_count = warm_result.as_ref().map(|m| m.symbol_count).unwrap_or(0);
    let facade_cache_ready = warm_result.is_ok() && facade_symbol_count > 0;

    json!({
        "project": project_name,
        "path": project_root,
        "relativePath": project.relative_path,
        "language": language,
        "manifestFile": project.manifest_file,
        "manifestBacked": project.is_manifest_backed,
        "status": "completed",
        "sourceFileCount": source_file_count,
        "symbolCount": project_run.symbol_count,
        "callEdgeCount": project_run.call_edge_count,
        "nodeCount": project_run.node_count,
        "edgeCount": project_run.edge_count,
        "durationMs": duration_ms,
        "executorMode": "project-once",
        "totalTasks": 1,
        "facadeCacheReady": facade_cache_ready,
        "facadeCacheWarmStatus": if facade_cache_ready { "ready" } else { "warm_failed_or_empty_graph" },
        "facadeSymbolCount": facade_symbol_count,
        "staticAnalysisOnly": true,
        "targetCodeExecuted": false,
    })
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

    #[test]
    fn wait_for_job_returns_immediately_on_terminal() {
        let registry = McpJobRegistry::new();
        let job = registry.submit("/tmp/wait-terminal", "rust", "serial");
        let job_id = job.job_id;
        // 直接设置为 Succeeded
        registry.update_status(&job_id, JobStatus::Succeeded);

        let (status, timed_out) = registry.wait_for_job(&job_id, 5000);
        assert!(!timed_out, "should not time out on terminal job");
        assert!(status.is_some(), "should return status");
    }

    #[test]
    fn wait_for_job_returns_none_for_unknown_job() {
        let registry = McpJobRegistry::new();
        let (status, timed_out) = registry.wait_for_job("nonexistent_job", 100);
        assert!(!timed_out);
        assert!(status.is_none(), "unknown job should return None");
    }

    #[test]
    fn wait_for_job_timeout_on_running_job() {
        let registry = McpJobRegistry::new();
        let job = registry.submit("/tmp/wait-running", "rust", "serial");
        let job_id = job.job_id;
        // 保持 running 状态

        let (status, timed_out) = registry.wait_for_job(&job_id, 200);
        assert!(timed_out, "should time out on running job");
        assert!(status.is_some(), "should return partial status");
    }
}
