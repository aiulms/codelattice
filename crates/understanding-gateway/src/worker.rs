//! Desktop Analyzer worker supervisor —— 纯状态机（P0 §8 / F1 #5）。
//!
//! 规则（§8）：
//! - 同一时刻最多一个重分析任务（单任务）
//! - 独立 jobId、取消令牌、临时目录；绝不复用/控制 Agent MCP 的 job registry
//! - 完整 snapshot 通过 temp + atomic rename 发布
//! - OS `nice`、线程数、取消策略封装在 scheduler interface 之后
//! 实际进程编排（spawn Command、kill）在 Tauri Core 层实现。

use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkerState {
    Idle,
    Running,
    Failed,
    Cancelled,
    Succeeded,
}

#[derive(Debug, Clone)]
pub struct WorkerCommand {
    pub job_id: String,
    pub project_root: PathBuf,
    pub language: String,
    /// 输出临时路径；发布时 atomic rename 到 store。
    pub temp_output: PathBuf,
    /// 资源预算（由 F1 基线冻结后写入，§8.5）。
    pub nice_level: i32,
    pub max_threads: Option<usize>,
}

impl WorkerEvent {
    pub fn into_job_id(self) -> String {
        match self {
            WorkerEvent::Started { job_id }
            | WorkerEvent::Progress { job_id, .. }
            | WorkerEvent::Finished { job_id, .. }
            | WorkerEvent::Failed { job_id, .. }
            | WorkerEvent::Cancelled { job_id } => job_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkerEvent {
    Started {
        job_id: String,
    },
    Progress {
        job_id: String,
        stage: String,
        pct: u8,
    },
    Finished {
        job_id: String,
        published_path: PathBuf,
    },
    Failed {
        job_id: String,
        message: String,
    },
    Cancelled {
        job_id: String,
    },
}

#[derive(Debug, Clone)]
pub struct SupervisorConfig {
    /// 任务目录前缀；临时文件必须带独立前缀便于清理（§8.2）。
    pub temp_prefix: String,
    pub default_nice: i32,
}

impl Default for SupervisorConfig {
    fn default() -> Self {
        Self {
            temp_prefix: "codelattice-desktop-analyzer-".to_string(),
            default_nice: 10, // 低优先级（agent 查询优先，§8.5）
        }
    }
}

#[derive(Debug, Clone)]
pub struct WorkerSupervisor {
    config: SupervisorConfig,
    state: WorkerState,
    active_job: Option<WorkerCommand>,
    cancel_requested: bool,
}

impl WorkerSupervisor {
    pub fn new(config: SupervisorConfig) -> Self {
        Self {
            config,
            state: WorkerState::Idle,
            active_job: None,
            cancel_requested: false,
        }
    }

    pub fn state(&self) -> WorkerState {
        self.state
    }

    pub fn active_job(&self) -> Option<&WorkerCommand> {
        self.active_job.as_ref()
    }

    /// 单任务 gate：Running 时拒绝新任务（§8.4）。
    pub fn start(&mut self, mut cmd: WorkerCommand) -> Result<WorkerEvent, &'static str> {
        if self.state == WorkerState::Running {
            return Err("a desktop analysis task is already running");
        }
        if cmd.nice_level == 0 {
            cmd.nice_level = self.config.default_nice;
        }
        self.cancel_requested = false;
        self.active_job = Some(cmd.clone());
        self.state = WorkerState::Running;
        Ok(WorkerEvent::Started { job_id: cmd.job_id })
    }

    pub fn progress(&mut self, stage: &str, pct: u8) -> Option<WorkerEvent> {
        if self.state != WorkerState::Running {
            return None;
        }
        let job_id = self.active_job.as_ref()?.job_id.clone();
        Some(WorkerEvent::Progress {
            job_id,
            stage: stage.to_string(),
            pct: pct.min(100),
        })
    }

    pub fn request_cancel(&mut self) {
        if self.state == WorkerState::Running {
            self.cancel_requested = true;
        }
    }

    pub fn is_cancel_requested(&self) -> bool {
        self.cancel_requested
    }

    /// 发布：temp + atomic rename（§8.2 / P0-C #1）。失败不允许留下半写文件。
    pub fn publish(&mut self, final_path: PathBuf) -> Result<WorkerEvent, String> {
        if self.state != WorkerState::Running {
            return Err("publish outside running state".to_string());
        }
        let cmd = self.active_job.clone().ok_or("no active job")?;
        if self.cancel_requested {
            self.state = WorkerState::Cancelled;
            return Ok(WorkerEvent::Cancelled { job_id: cmd.job_id });
        }
        std::fs::rename(&cmd.temp_output, &final_path)
            .map_err(|e| format!("atomic rename failed: {e}"))?;
        self.state = WorkerState::Succeeded;
        self.active_job = None;
        self.cancel_requested = false;
        Ok(WorkerEvent::Finished {
            job_id: cmd.job_id,
            published_path: final_path,
        })
    }

    pub fn fail(&mut self, message: &str) -> Option<WorkerEvent> {
        if self.state != WorkerState::Running {
            return None;
        }
        let job_id = self.active_job.as_ref()?.job_id.clone();
        // 失败清理：删除半写临时文件（§8.6 / 验收 24）
        if let Some(cmd) = &self.active_job {
            let _ = std::fs::remove_file(&cmd.temp_output);
        }
        self.state = WorkerState::Failed;
        self.active_job = None;
        self.cancel_requested = false;
        Some(WorkerEvent::Failed {
            job_id,
            message: message.to_string(),
        })
    }

    pub fn temp_dir_for(&self, job_id: &str) -> PathBuf {
        PathBuf::from(format!("{}{}", self.config.temp_prefix, job_id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    fn cmd(job: &str) -> WorkerCommand {
        // 唯一后缀避免并行测试互相踩临时文件
        let unique = format!(
            "{job}-{}",
            COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        );
        WorkerCommand {
            job_id: job.to_string(),
            project_root: PathBuf::from("/tmp/proj"),
            language: "rust".to_string(),
            temp_output: PathBuf::from(format!("/tmp/{unique}.tmp.json")),
            nice_level: 0,
            max_threads: None,
        }
    }

    #[test]
    fn single_task_gate_blocks_concurrent_start() {
        let mut s = WorkerSupervisor::new(SupervisorConfig::default());
        assert!(s.start(cmd("job1")).is_ok());
        assert_eq!(s.state(), WorkerState::Running);
        assert!(s.start(cmd("job2")).is_err(), "同时最多一个重分析任务");
    }

    #[test]
    fn cancel_before_publish_yields_cancelled_event_and_no_publish() {
        let mut s = WorkerSupervisor::new(SupervisorConfig::default());
        s.start(cmd("job1")).unwrap();
        s.request_cancel();
        let ev = s.publish(PathBuf::from("/tmp/final.json")).unwrap();
        assert_eq!(
            ev,
            WorkerEvent::Cancelled {
                job_id: "job1".to_string()
            }
        );
        assert_eq!(s.state(), WorkerState::Cancelled);
        assert!(
            !PathBuf::from("/tmp/final.json").exists(),
            "取消不得发布半写 snapshot"
        );
    }

    #[test]
    fn publish_uses_atomic_rename_and_succeeds() {
        let mut s = WorkerSupervisor::new(SupervisorConfig::default());
        let mut c = cmd("job1");
        std::fs::write(&c.temp_output, b"{}").unwrap();
        let final_path = c.temp_output.with_extension("final.json");
        let _ = std::fs::remove_file(&final_path);
        s.start(c.clone()).unwrap();
        c.nice_level = 10;
        let ev = s.publish(final_path.clone()).unwrap();
        assert!(
            matches!(ev, WorkerEvent::Finished { published_path, .. } if published_path == final_path)
        );
        assert!(final_path.exists());
        assert_eq!(s.state(), WorkerState::Succeeded);
        let _ = std::fs::remove_file(final_path);
    }

    #[test]
    fn fail_cleans_halfwritten_temp_file() {
        let mut s = WorkerSupervisor::new(SupervisorConfig::default());
        let c = cmd("job1");
        std::fs::write(&c.temp_output, b"half").unwrap();
        s.start(c.clone()).unwrap();
        let ev = s.fail("analyze crashed").unwrap();
        assert!(matches!(ev, WorkerEvent::Failed { message, .. } if message == "analyze crashed"));
        assert!(!c.temp_output.exists(), "失败必须清理临时文件");
        assert_eq!(s.state(), WorkerState::Failed);
        assert!(s.start(cmd("job2")).is_ok(), "失败后可再启动");
    }

    #[test]
    fn default_nice_is_low_priority() {
        let mut s = WorkerSupervisor::new(SupervisorConfig::default());
        s.start(cmd("job1")).unwrap();
        assert_eq!(
            s.active_job().unwrap().nice_level,
            10,
            "默认低优先级（agent 查询优先）"
        );
    }
}
