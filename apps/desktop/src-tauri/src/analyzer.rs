// analyzer —— Desktop Analyzer supervisor（返工第二轮 C-fix）。
//
// 关键修复：
// - 不在 wait_with_output 期间持有 supervisor mutex（锁只用于短操作）
// - cancel 可在运行中 kill child（不阻塞）
// - status 立即返回 Running（不需要等任务结束）
// - 完成后返回 publishedSnapshotId

use std::path::PathBuf;
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use serde_json::json;
use understanding_gateway::worker::{SupervisorConfig, WorkerEvent, WorkerState, WorkerSupervisor};

/// 运行中的 analyzer 进程信息（child 独立 mutex，不阻塞 supervisor）。
struct RunningJob {
    job_id: String,
    child: Mutex<Option<Child>>,
    cancel_flag: AtomicBool,
    temp_output: PathBuf,
    publish_dir: PathBuf,
}

pub struct AnalyzerSupervisor {
    machine: WorkerSupervisor,
    /// 独立 mutex 保护 RunningJob 引用；wait 在锁外执行。
    running: Mutex<Option<RunningJob>>,
    /// 最终结果（完成后可查）。
    last_result: Mutex<Option<AnalyzerResult>>,
}

/// 分析器完成结果。
#[derive(Debug, Clone)]
pub struct AnalyzerResult {
    pub job_id: String,
    pub state: AnalyzerState,
    pub published_snapshot_id: Option<String>,
    pub published_path: Option<String>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AnalyzerState {
    Idle,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl Default for AnalyzerSupervisor {
    fn default() -> Self {
        Self {
            machine: WorkerSupervisor::new(SupervisorConfig {
                temp_prefix: "codelattice-desktop-analyzer-".to_string(),
                default_nice: 10,
            }),
            running: Mutex::new(None),
            last_result: Mutex::new(None),
        }
    }
}

impl AnalyzerSupervisor {
    pub fn state(&self) -> WorkerState {
        self.machine.state()
    }

    /// 返回当前状态（不阻塞 wait）。
    pub fn analyzer_state(&self) -> AnalyzerState {
        if self.running.lock().unwrap().is_some() {
            return AnalyzerState::Running;
        }
        match self.last_result.lock().unwrap().as_ref() {
            Some(r) => r.state.clone(),
            None => AnalyzerState::Idle,
        }
    }

    /// 返回当前 job_id（不阻塞）。
    pub fn active_job_id(&self) -> Option<String> {
        self.running.lock().unwrap().as_ref().map(|r| r.job_id.clone())
    }

    /// 返回最近一次结果（不阻塞）。
    pub fn last_result(&self) -> Option<AnalyzerResult> {
        self.last_result.lock().unwrap().clone()
    }

    /// 启动分析。
    pub fn start(
        &self,
        project_root: PathBuf,
        language: String,
        codelattice_bin: PathBuf,
        publish_dir: PathBuf,
    ) -> Result<String, String> {
        // 单任务 gate
        if self.running.lock().unwrap().is_some() {
            return Err("analyzer already running".to_string());
        }

        let job_id = format!("job-{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_secs());
        let temp_output = std::env::temp_dir().join(format!("{job_id}.tmp.json"));

        let mut command = std::process::Command::new("nice");
        command
            .arg("-n")
            .arg("10")
            .arg(&codelattice_bin)
            .args(["analyze", "--root"])
            .arg(&project_root)
            .args(["--language", &language, "--format", "json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let child = command.spawn().map_err(|e| format!("spawn failed: {e}"))?;

        let job = RunningJob {
            job_id: job_id.clone(),
            child: Mutex::new(Some(child)),
            cancel_flag: AtomicBool::new(false),
            temp_output,
            publish_dir,
        };
        *self.running.lock().unwrap() = Some(job);
        *self.last_result.lock().unwrap() = None;
        Ok(job_id)
    }

    /// 取消正在运行的 child（不阻塞 wait）。
    pub fn request_cancel(&self) {
        if let Some(job) = self.running.lock().unwrap().as_ref() {
            job.cancel_flag.store(true, Ordering::SeqCst);
            // kill child — 短操作
            if let Some(mut child) = job.child.lock().unwrap().take() {
                let _ = child.kill();
            }
        }
    }

    /// 等待完成并发布（在独立线程调用；锁外执行 wait）。
    pub fn wait_and_publish(&self) -> AnalyzerResult {
        // 从 mutex 中取出 job（短操作），然后在锁外 wait
        let job = {
            let mut guard = self.running.lock().unwrap();
            match guard.take() {
                Some(j) => j,
                None => {
                    return AnalyzerResult {
                        job_id: String::new(),
                        state: AnalyzerState::Idle,
                        published_snapshot_id: None,
                        published_path: None,
                        error: Some("no running analyzer".to_string()),
                    };
                }
            }
        };

        if job.cancel_flag.load(Ordering::SeqCst) {
            let _ = std::fs::remove_file(&job.temp_output);
            let result = AnalyzerResult {
                job_id: job.job_id.clone(),
                state: AnalyzerState::Cancelled,
                published_snapshot_id: None,
                published_path: None,
                error: None,
            };
            *self.last_result.lock().unwrap() = Some(result.clone());
            return result;
        }

        // 锁外执行 wait_with_output
        let child = match job.child.lock().unwrap().take() {
            Some(c) => c,
            None => {
                let result = AnalyzerResult {
                    job_id: job.job_id.clone(),
                    state: AnalyzerState::Failed,
                    published_snapshot_id: None,
                    published_path: None,
                    error: Some("child already taken".to_string()),
                };
                *self.last_result.lock().unwrap() = Some(result.clone());
                return result;
            }
        };

        let output = match child.wait_with_output() {
            Ok(o) => o,
            Err(e) => {
                let result = AnalyzerResult {
                    job_id: job.job_id.clone(),
                    state: AnalyzerState::Failed,
                    published_snapshot_id: None,
                    published_path: None,
                    error: Some(format!("wait failed: {e}")),
                };
                *self.last_result.lock().unwrap() = Some(result.clone());
                return result;
            }
        };

        if job.cancel_flag.load(Ordering::SeqCst) {
            let _ = std::fs::remove_file(&job.temp_output);
            let result = AnalyzerResult {
                job_id: job.job_id.clone(),
                state: AnalyzerState::Cancelled,
                published_snapshot_id: None,
                published_path: None,
                error: None,
            };
            *self.last_result.lock().unwrap() = Some(result.clone());
            return result;
        }

        if !output.status.success() {
            let _ = std::fs::remove_file(&job.temp_output);
            let result = AnalyzerResult {
                job_id: job.job_id.clone(),
                state: AnalyzerState::Failed,
                published_snapshot_id: None,
                published_path: None,
                error: Some(format!("analyze failed with {}", output.status)),
            };
            *self.last_result.lock().unwrap() = Some(result.clone());
            return result;
        }

        // 原子发布
        let _ = std::fs::write(&job.temp_output, &output.stdout);
        let final_path = job.publish_dir.join(format!("{}.json", job.job_id));
        let tmp_path = job.publish_dir.join(format!(".{}.tmp", job.job_id));
        let _ = std::fs::write(&tmp_path, &output.stdout);
        let publish_result = std::fs::rename(&tmp_path, &final_path);
        let _ = std::fs::remove_file(&job.temp_output);

        match publish_result {
            Ok(_) => {
                let result = AnalyzerResult {
                    job_id: job.job_id.clone(),
                    state: AnalyzerState::Completed,
                    published_snapshot_id: Some(job.job_id.clone()),
                    published_path: Some(final_path.to_string_lossy().to_string()),
                    error: None,
                };
                *self.last_result.lock().unwrap() = Some(result.clone());
                result
            }
            Err(e) => {
                let result = AnalyzerResult {
                    job_id: job.job_id.clone(),
                    state: AnalyzerState::Failed,
                    published_snapshot_id: None,
                    published_path: None,
                    error: Some(format!("publish failed: {e}")),
                };
                *self.last_result.lock().unwrap() = Some(result.clone());
                result
            }
        }
    }
}
