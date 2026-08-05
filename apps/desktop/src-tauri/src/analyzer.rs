// analyzer —— Desktop Analyzer supervisor（返工第二轮 C-fix）。
//
// 关键修复：
// - 不在 wait_with_output 期间持有 supervisor mutex（锁只用于短操作）
// - cancel 可在运行中 kill child（不阻塞）
// - status 立即返回 Running（不需要等任务结束）
// - 完成后返回 publishedSnapshotId

use std::fs::File;
use std::path::PathBuf;
use std::process::{Child, ExitStatus, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// 运行中的 analyzer 进程信息（child 独立 mutex，不阻塞 supervisor）。
struct RunningJob {
    job_id: String,
    child: Mutex<Option<Child>>,
    cancel_flag: AtomicBool,
    temp_output: PathBuf,
    publish_dir: PathBuf,
}

pub struct AnalyzerSupervisor {
    /// 共享 job 保留到 wait 结束，使 status/cancel 始终能定位 child。
    running: Mutex<Option<Arc<RunningJob>>>,
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
            running: Mutex::new(None),
            last_result: Mutex::new(None),
        }
    }
}

impl AnalyzerSupervisor {
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
        self.running
            .lock()
            .unwrap()
            .as_ref()
            .map(|r| r.job_id.clone())
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

        let job_id = format!(
            "job-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_err(|e| e.to_string())?
                .as_nanos()
        );
        let temp_output = std::env::temp_dir().join(format!("{job_id}.tmp.json"));
        let stdout = File::create(&temp_output)
            .map_err(|e| format!("create analyzer output failed: {e}"))?;

        let mut command = std::process::Command::new("nice");
        command
            .arg("-n")
            .arg("10")
            .arg(&codelattice_bin)
            .args(["analyze", "--root"])
            .arg(&project_root)
            .args(["--language", &language, "--format", "json"])
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::null());

        let child = command.spawn().map_err(|e| {
            let _ = std::fs::remove_file(&temp_output);
            format!("spawn failed: {e}")
        })?;

        let job = Arc::new(RunningJob {
            job_id: job_id.clone(),
            child: Mutex::new(Some(child)),
            cancel_flag: AtomicBool::new(false),
            temp_output,
            publish_dir,
        });
        *self.running.lock().unwrap() = Some(job);
        *self.last_result.lock().unwrap() = None;
        Ok(job_id)
    }

    /// 取消正在运行的 child（不阻塞 wait）。
    pub fn request_cancel(&self) {
        let job = self.running.lock().unwrap().clone();
        if let Some(job) = job {
            job.cancel_flag.store(true, Ordering::SeqCst);
            // child 保留在共享 job 中；kill 后由 wait loop 回收状态。
            if let Some(child) = job.child.lock().unwrap().as_mut() {
                let _ = child.kill();
            }
        }
    }

    fn finish(&self, result: AnalyzerResult) -> AnalyzerResult {
        let mut running = self.running.lock().unwrap();
        if running
            .as_ref()
            .is_some_and(|job| job.job_id == result.job_id)
        {
            *running = None;
        }
        drop(running);
        *self.last_result.lock().unwrap() = Some(result.clone());
        result
    }

    /// 等待完成并发布（独立线程调用；只短暂锁 child 做 try_wait）。
    pub fn wait_and_publish(&self) -> AnalyzerResult {
        let job = {
            let guard = self.running.lock().unwrap();
            match guard.as_ref() {
                Some(j) => Arc::clone(j),
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

        let status: ExitStatus = loop {
            let polled = {
                let mut child = job.child.lock().unwrap();
                match child.as_mut() {
                    Some(child) => child.try_wait(),
                    None => {
                        return self.finish(AnalyzerResult {
                            job_id: job.job_id.clone(),
                            state: AnalyzerState::Failed,
                            published_snapshot_id: None,
                            published_path: None,
                            error: Some("analyzer child missing".to_string()),
                        });
                    }
                }
            };
            match polled {
                Ok(Some(status)) => break status,
                Ok(None) => std::thread::sleep(Duration::from_millis(20)),
                Err(e) => {
                    return self.finish(AnalyzerResult {
                        job_id: job.job_id.clone(),
                        state: AnalyzerState::Failed,
                        published_snapshot_id: None,
                        published_path: None,
                        error: Some(format!("wait failed: {e}")),
                    });
                }
            }
        };

        if job.cancel_flag.load(Ordering::SeqCst) {
            let _ = std::fs::remove_file(&job.temp_output);
            return self.finish(AnalyzerResult {
                job_id: job.job_id.clone(),
                state: AnalyzerState::Cancelled,
                published_snapshot_id: None,
                published_path: None,
                error: None,
            });
        }

        if !status.success() {
            let _ = std::fs::remove_file(&job.temp_output);
            return self.finish(AnalyzerResult {
                job_id: job.job_id.clone(),
                state: AnalyzerState::Failed,
                published_snapshot_id: None,
                published_path: None,
                error: Some(format!("analyze failed with {status}")),
            });
        }

        // 原子发布
        let final_path = job.publish_dir.join(format!("{}.json", job.job_id));
        let tmp_path = job.publish_dir.join(format!(".{}.tmp", job.job_id));
        if let Err(e) = std::fs::copy(&job.temp_output, &tmp_path) {
            let _ = std::fs::remove_file(&job.temp_output);
            return self.finish(AnalyzerResult {
                job_id: job.job_id.clone(),
                state: AnalyzerState::Failed,
                published_snapshot_id: None,
                published_path: None,
                error: Some(format!("stage publish failed: {e}")),
            });
        }
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
                self.finish(result)
            }
            Err(e) => {
                let result = AnalyzerResult {
                    job_id: job.job_id.clone(),
                    state: AnalyzerState::Failed,
                    published_snapshot_id: None,
                    published_path: None,
                    error: Some(format!("publish failed: {e}")),
                };
                self.finish(result)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::Arc;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn test_dir(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!(
            "codelattice-analyzer-{label}-{}-{nonce}",
            std::process::id()
        ));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn running_job_remains_observable_and_cancellable_while_waiting() {
        let dir = test_dir("cancel");
        let script = dir.join("slow-analyzer.sh");
        fs::write(&script, "#!/bin/sh\nsleep 2\nprintf '{\"ok\":true}'\n").unwrap();
        let mut permissions = fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script, permissions).unwrap();

        let supervisor = Arc::new(AnalyzerSupervisor::default());
        supervisor
            .start(dir.clone(), "rust".into(), script, dir.clone())
            .unwrap();

        let waiter = Arc::clone(&supervisor);
        let join = std::thread::spawn(move || waiter.wait_and_publish());
        std::thread::sleep(Duration::from_millis(100));

        let observed_while_waiting = supervisor.analyzer_state();
        let cancel_started = std::time::Instant::now();
        supervisor.request_cancel();
        let cancel_latency = cancel_started.elapsed();
        let result = join.join().unwrap();

        let _ = fs::remove_dir_all(&dir);
        assert_eq!(observed_while_waiting, AnalyzerState::Running);
        assert!(cancel_latency < Duration::from_millis(100));
        assert_eq!(result.state, AnalyzerState::Cancelled);
        assert!(result.published_snapshot_id.is_none());
    }
}
