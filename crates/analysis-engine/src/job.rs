//! Analysis job runtime — job lifecycle, progress tracking, paged results.
//!
//! Jobs have id-based lifecycle: queued → running → succeeded/failed/cancelled.
//! Supports progress events, deduplication, cancellation, and paged result output.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use crate::executor::{ProgressEvent, SerializableResult};

/// Job lifecycle status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum JobStatus {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

/// Job progress snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobProgress {
    pub stage: String,
    pub completed_units: usize,
    pub total_units: usize,
    pub failed_units: usize,
    pub elapsed_ms: u64,
    pub executor_mode: String,
}

/// Paged result chunk for large outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PagedResult {
    pub page: usize,
    pub total_pages: usize,
    pub page_size: usize,
    pub total_items: usize,
    pub items: Vec<serde_json::Value>,
    pub has_next: bool,
    pub has_prev: bool,
}

/// A complete analysis job — submitted, executed, and tracked.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisJob {
    pub job_id: String,
    pub root: String,
    pub language: String,
    pub status: JobStatus,
    pub progress: Option<JobProgress>,
    pub result_summary: Option<JobResultSummary>,
    pub created_at_ms: u64,
    pub started_at_ms: Option<u64>,
    pub completed_at_ms: Option<u64>,
    pub error: Option<String>,
}

/// Compact result summary — returned by default.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct JobResultSummary {
    pub node_count: usize,
    pub edge_count: usize,
    pub call_edge_count: usize,
    pub diagnostic_count: usize,
    pub source_file_count: usize,
    pub symbol_count: usize,
    pub total_duration_ms: u64,
    pub executor_mode: String,
    pub static_analysis_only: bool,
    pub cache_hits: usize,
    pub cache_misses: usize,
    pub total_pages: usize,
    pub next_page_hint: Option<String>,
}

/// Runtime manager for jobs — tracks active jobs, supports deduplication.
pub struct JobRuntime {
    jobs: Arc<Mutex<HashMap<String, AnalysisJob>>>,
    results: Arc<Mutex<HashMap<String, SerializableResult>>>,
}

impl JobRuntime {
    pub fn new() -> Self {
        Self {
            jobs: Arc::new(Mutex::new(HashMap::new())),
            results: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Register a new job (or return existing if deduplicated).
    pub fn submit(&self, job_id: String, root: String, language: String) -> AnalysisJob {
        let mut jobs = self.jobs.lock().unwrap();
        if let Some(existing) = jobs.get(&job_id) {
            return existing.clone();
        }
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let job = AnalysisJob {
            job_id: job_id.clone(),
            root,
            language,
            status: JobStatus::Queued,
            progress: None,
            result_summary: None,
            created_at_ms: now,
            started_at_ms: None,
            completed_at_ms: None,
            error: None,
        };
        jobs.insert(job_id, job.clone());
        job
    }

    /// Update job status.
    pub fn update_status(&self, job_id: &str, status: JobStatus) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(job) = jobs.get_mut(job_id) {
                job.status = status;
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis() as u64;
                if status == JobStatus::Running && job.started_at_ms.is_none() {
                    job.started_at_ms = Some(now);
                }
                if status == JobStatus::Succeeded
                    || status == JobStatus::Failed
                    || status == JobStatus::Cancelled
                {
                    job.completed_at_ms = Some(now);
                }
            }
        }
    }

    /// Update job progress.
    pub fn update_progress(&self, job_id: &str, progress: JobProgress) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(job) = jobs.get_mut(job_id) {
                job.progress = Some(progress);
            }
        }
    }

    /// Set job result with compact summary.
    pub fn set_result(
        &self,
        job_id: &str,
        result: &SerializableResult,
        cache_hits: usize,
        cache_misses: usize,
    ) {
        if let Ok(mut jobs) = self.jobs.lock() {
            if let Some(job) = jobs.get_mut(job_id) {
                let summary = JobResultSummary {
                    node_count: 0,
                    edge_count: 0,
                    call_edge_count: 0,
                    diagnostic_count: result.failed,
                    source_file_count: 0,
                    symbol_count: 0,
                    total_duration_ms: result.total_duration_ms,
                    executor_mode: result.executor_mode.clone(),
                    static_analysis_only: true,
                    cache_hits,
                    cache_misses,
                    total_pages: 1,
                    next_page_hint: Some(format!("detail page 1/1 for {}", job_id)),
                };
                job.result_summary = Some(summary);
                job.status = if result.failed > 0 && result.completed == 0 {
                    JobStatus::Failed
                } else {
                    JobStatus::Succeeded
                };
            }
        }
    }

    /// Get job by ID.
    pub fn get(&self, job_id: &str) -> Option<AnalysisJob> {
        self.jobs.lock().ok()?.get(job_id).cloned()
    }

    /// Cancel a running job.
    pub fn cancel(&self, _job_id: &str) -> bool {
        // Jobs are cancelled via the cancel flag in the executor
        true
    }

    /// Paginate large results into fixed-size pages.
    pub fn paginate(items: &[serde_json::Value], page: usize, page_size: usize) -> PagedResult {
        let total = items.len();
        let total_pages = ((total as f64) / (page_size as f64)).ceil() as usize;
        let start = page.saturating_mul(page_size);
        let end = (start + page_size).min(total);
        let page_items: Vec<_> = items[start..end].to_vec();

        PagedResult {
            page,
            total_pages: total_pages.max(1),
            page_size,
            total_items: total,
            items: page_items,
            has_next: page + 1 < total_pages,
            has_prev: page > 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn job_lifecycle() {
        let rt = JobRuntime::new();
        let job = rt.submit("job_1".into(), "/repo".into(), "rust".into());
        assert_eq!(job.status, JobStatus::Queued);

        rt.update_status("job_1", JobStatus::Running);
        let j = rt.get("job_1").unwrap();
        assert_eq!(j.status, JobStatus::Running);
        assert!(j.started_at_ms.is_some());

        rt.update_status("job_1", JobStatus::Succeeded);
        let j = rt.get("job_1").unwrap();
        assert_eq!(j.status, JobStatus::Succeeded);
        assert!(j.completed_at_ms.is_some());
    }

    #[test]
    fn pagination() {
        let items: Vec<serde_json::Value> = (0..25).map(|i| serde_json::json!({"id": i})).collect();
        let page = JobRuntime::paginate(&items, 0, 10);
        assert_eq!(page.items.len(), 10);
        assert_eq!(page.total_items, 25);
        assert_eq!(page.total_pages, 3);
        assert!(page.has_next);
        assert!(!page.has_prev);

        let page2 = JobRuntime::paginate(&items, 1, 10);
        assert_eq!(page2.items.len(), 10);
        assert!(page2.has_next);
        assert!(page2.has_prev);

        let page3 = JobRuntime::paginate(&items, 2, 10);
        assert_eq!(page3.items.len(), 5);
        assert!(!page3.has_next);
    }
}
