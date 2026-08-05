// analyzer —— Desktop Analyzer supervisor（F1 #5 / P0-C）。
// 只由 Tauri Core 启停；独立 jobId、单任务、取消、退出清理与临时目录。
// 绝不复用/控制 Agent MCP 的 job registry（§8）。

use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use understanding_gateway::worker::{SupervisorConfig, WorkerCommand, WorkerEvent, WorkerState, WorkerSupervisor};

pub struct RunningAnalyzer {
    pub job_id: String,
    pub child: Mutex<Option<Child>>,
    pub cancel_flag: AtomicBool,
    pub temp_output: PathBuf,
}

pub struct AnalyzerSupervisor {
    machine: WorkerSupervisor,
    running: Mutex<Option<RunningAnalyzer>>,
}

impl Default for AnalyzerSupervisor {
    fn default() -> Self {
        Self {
            machine: WorkerSupervisor::new(SupervisorConfig {
                temp_prefix: "codelattice-desktop-analyzer-".to_string(),
                default_nice: 10,
            }),
            running: Mutex::new(None),
        }
    }
}

impl AnalyzerSupervisor {
    pub fn state(&self) -> WorkerState {
        self.machine.state()
    }

    /// 后台线程完成等待+发布（进度/取消事件由调用方转发）。
    pub fn run_to_completion(&mut self, publish_dir: PathBuf) -> Result<WorkerEvent, String> {
        self.wait_and_publish(publish_dir)
    }

    /// 启动分析（单任务 gate 由 WorkerSupervisor 强制）。
    /// 输出：stdout JSON 写入 temp 文件，完成时 atomic rename 发布到 snapshot dir。
    pub fn start(
        &mut self,
        project_root: PathBuf,
        language: String,
        codelattice_bin: PathBuf,
        _publish_dir: PathBuf,
    ) -> Result<WorkerEvent, String> {
        let job_id = format!("job:{}", std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).map_err(|e| e.to_string())?.as_secs());
        let temp_output = std::env::temp_dir().join(format!("{job_id}.tmp.json"));

        let cmd = WorkerCommand {
            job_id: job_id.clone(),
            project_root: project_root.clone(),
            language: language.clone(),
            temp_output: temp_output.clone(),
            nice_level: 0,
            max_threads: None,
        };
        // _publish_dir：发布目录由 wait_and_publish 在完成时刻决定（§8 temp +
        // atomic publish），start 只负责 spawn 与临时路径。
        let event = self.machine.start(cmd).map_err(|e| e.to_string())?;

        // OS nice 封装在 supervisor 之后（§8.7）：darwin 用 `nice -n`
        let nice = self.machine.active_job().map(|c| c.nice_level).unwrap_or(10);
        let mut command = Command::new("nice");
        command
            .arg("-n")
            .arg(nice.to_string())
            .arg(&codelattice_bin)
            .args(["analyze", "--root"])
            .arg(&project_root)
            .args(["--language", &language, "--format", "json"])
            .stdout(Stdio::piped())
            .stderr(Stdio::null());

        let child = command.spawn().map_err(|e| {
            self.machine.fail(&format!("spawn failed: {e}"));
            format!("spawn failed: {e}")
        })?;

        let running = RunningAnalyzer {
            job_id: job_id.clone(),
            child: Mutex::new(Some(child)),
            cancel_flag: AtomicBool::new(false),
            temp_output: temp_output.clone(),
        };
        *self.running.lock().unwrap() = Some(running);
        Ok(event)
    }

    pub fn request_cancel(&mut self) {
        self.machine.request_cancel();
        if let Some(r) = self.running.lock().unwrap().as_ref() {
            r.cancel_flag.store(true, Ordering::SeqCst);
            if let Some(mut child) = r.child.lock().unwrap().take() {
                let _ = child.kill();
            }
        }
    }

    /// 等待子进程结束并收集 stdout → temp 文件 → 发布。
    pub fn wait_and_publish(&mut self, publish_dir: PathBuf) -> Result<WorkerEvent, String> {
        let job = self
            .running
            .lock()
            .unwrap()
            .take()
            .ok_or("no running analyzer")?;

        if job.cancel_flag.load(Ordering::SeqCst) {
            let _ = std::fs::remove_file(&job.temp_output);
            return Ok(WorkerEvent::Cancelled { job_id: job.job_id });
        }

        let child = job.child.lock().unwrap().take().ok_or("child already taken")?;
        let output = child.wait_with_output().map_err(|e| e.to_string())?;
        if !output.status.success() {
            let msg = format!("analyze failed with {}", output.status);
            let _ = std::fs::remove_file(&job.temp_output);
            return Ok(WorkerEvent::Failed { job_id: job.job_id, message: msg });
        }

        std::fs::write(&job.temp_output, &output.stdout).map_err(|e| e.to_string())?;
        let final_path = publish_dir.join(format!("{}.json", job.job_id));
        self.machine.publish(final_path)
    }

    pub fn active_job_id(&self) -> Option<String> {
        self.machine.active_job().map(|c| c.job_id.clone())
    }
}
