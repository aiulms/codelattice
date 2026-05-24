//! Analysis executors — serial baseline and bounded parallel worker pool.
//! Simplified version for Analysis Engine 1.3.

use crate::adapter::LanguageAdapter;
use crate::dag::{AnalysisArtifact, AnalysisPlan, AnalysisStage, AnalysisTask, ArtifactSemantics};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineConfig {
    pub max_workers: usize,
    pub per_task_timeout_ms: u64,
    pub enable_parallel: bool,
    pub enable_progress_events: bool,
    pub cache_enabled: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_workers: 4,
            per_task_timeout_ms: 120_000,
            enable_parallel: true,
            enable_progress_events: false,
            cache_enabled: false,
        }
    }
}

impl EngineConfig {
    pub fn serial_only() -> Self {
        Self {
            enable_parallel: false,
            ..Default::default()
        }
    }
    pub fn parallel(workers: usize) -> Self {
        Self {
            enable_parallel: true,
            max_workers: workers,
            ..Default::default()
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressEvent {
    pub stage: String,
    pub completed_units: usize,
    pub total_units: usize,
    pub failed_units: usize,
    pub elapsed_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SerializableResult {
    pub total_tasks: usize,
    pub completed: usize,
    pub failed: usize,
    pub total_duration_ms: u64,
    pub artifacts: Vec<AnalysisArtifact>,
    pub stage_times: HashMap<String, u64>,
    pub executor_mode: String,
}

fn run_single_task(
    task: &AnalysisTask,
    adapter: &dyn LanguageAdapter,
) -> Result<AnalysisArtifact, String> {
    let start = Instant::now();
    let unit = adapter
        .discover_files(&task.root)?
        .into_iter()
        .find(|f| f.id == task.unit_id)
        .ok_or_else(|| format!("Unit {} not found", task.unit_id))?;

    let data = match task.stage {
        AnalysisStage::Parse => {
            serde_json::to_value(adapter.parse_file(&unit)?).map_err(|e| e.to_string())?
        }
        AnalysisStage::Symbol => {
            serde_json::to_value(adapter.extract_symbols(&unit)?).map_err(|e| e.to_string())?
        }
        AnalysisStage::Import => {
            serde_json::to_value(adapter.extract_imports(&unit)?).map_err(|e| e.to_string())?
        }
        AnalysisStage::Reference => {
            serde_json::to_value(adapter.extract_references(&unit)?).map_err(|e| e.to_string())?
        }
        _ => serde_json::json!({}),
    };

    Ok(AnalysisArtifact {
        schema_version: "codelattice.artifact.v1".into(),
        task_id: task.id.clone(),
        stage: task.stage,
        language: task.language.clone(),
        unit_id: task.unit_id.clone(),
        cache_key: task.cache_key.clone(),
        data,
        error: None,
        duration_ms: start.elapsed().as_millis() as u64,
        generated_from: ArtifactSemantics::default(),
    })
}

fn make_error_artifact(task: &AnalysisTask, error: String) -> AnalysisArtifact {
    AnalysisArtifact {
        schema_version: "codelattice.artifact.v1".into(),
        task_id: task.id.clone(),
        stage: task.stage,
        language: task.language.clone(),
        unit_id: task.unit_id.clone(),
        cache_key: None,
        data: serde_json::json!({}),
        error: Some(error),
        duration_ms: 0,
        generated_from: ArtifactSemantics::default(),
    }
}

// ═══════════════════════════════════════════════════════════════
// Serial Executor
// ═══════════════════════════════════════════════════════════════

pub struct SerialExecutor;

impl SerialExecutor {
    pub fn execute(
        &self,
        plan: &AnalysisPlan,
        adapter: &dyn LanguageAdapter,
    ) -> SerializableResult {
        let start = Instant::now();
        let mut artifacts = Vec::new();
        let mut stage_times = HashMap::new();
        let mut completed = 0usize;
        let mut failed = 0usize;

        for task in &plan.tasks {
            match run_single_task(task, adapter) {
                Ok(art) => {
                    *stage_times
                        .entry(task.stage.name().to_string())
                        .or_insert(0) += art.duration_ms;
                    artifacts.push(art);
                    completed += 1;
                }
                Err(e) => {
                    artifacts.push(make_error_artifact(task, e));
                    failed += 1;
                }
            }
        }

        SerializableResult {
            total_tasks: plan.total_tasks,
            completed,
            failed,
            total_duration_ms: start.elapsed().as_millis() as u64,
            artifacts,
            stage_times,
            executor_mode: "serial".into(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════
// Parallel Executor
// ═══════════════════════════════════════════════════════════════

use std::thread;

pub struct ParallelExecutor {
    max_workers: usize,
}

impl ParallelExecutor {
    pub fn new(workers: usize) -> Self {
        Self {
            max_workers: workers,
        }
    }

    pub fn execute(
        &self,
        plan: &AnalysisPlan,
        adapter: Arc<dyn LanguageAdapter + 'static>,
    ) -> SerializableResult {
        let start = Instant::now();

        // Separate parallel and serial tasks
        let (parallel_tasks, serial_tasks): (Vec<_>, Vec<_>) =
            plan.tasks.iter().cloned().partition(|t| t.parallelizable);

        // Shared queue
        let queue: Arc<Mutex<Vec<AnalysisTask>>> = Arc::new(Mutex::new(parallel_tasks));
        let results: Arc<Mutex<Vec<AnalysisArtifact>>> = Arc::new(Mutex::new(Vec::new()));
        let completed = Arc::new(AtomicUsize::new(0));
        let failed = Arc::new(AtomicUsize::new(0));
        let stage_times: Arc<Mutex<HashMap<String, u64>>> = Arc::new(Mutex::new(HashMap::new()));

        let worker_count = self.max_workers.min(plan.parallelizable_tasks.max(1));
        let mut workers = Vec::new();

        for _ in 0..worker_count {
            let q = queue.clone();
            let r = results.clone();
            let c = completed.clone();
            let f = failed.clone();
            let st = stage_times.clone();
            let a = adapter.clone();

            workers.push(thread::spawn(move || loop {
                let task = { q.lock().unwrap().pop() };
                match task {
                    None => break,
                    Some(task) => {
                        let t_start = Instant::now();
                        match run_single_task(&task, a.as_ref()) {
                            Ok(art) => {
                                c.fetch_add(1, Ordering::Relaxed);
                                if let Ok(mut tm) = st.lock() {
                                    *tm.entry(task.stage.name().to_string()).or_insert(0) +=
                                        art.duration_ms;
                                }
                                r.lock().unwrap().push(art);
                            }
                            Err(e) => {
                                f.fetch_add(1, Ordering::Relaxed);
                                r.lock().unwrap().push(make_error_artifact(&task, e));
                            }
                        }
                    }
                }
            }));
        }

        for w in workers {
            let _ = w.join();
        }

        // Serial tasks
        for task in &serial_tasks {
            match run_single_task(task, adapter.as_ref()) {
                Ok(art) => {
                    results.lock().unwrap().push(art);
                    completed.fetch_add(1, Ordering::Relaxed);
                }
                Err(e) => {
                    results.lock().unwrap().push(make_error_artifact(task, e));
                    failed.fetch_add(1, Ordering::Relaxed);
                }
            }
        }

        let comp = completed.load(Ordering::Relaxed);
        let fail = failed.load(Ordering::Relaxed);

        let artifacts_out: Vec<AnalysisArtifact> = results.lock().unwrap().clone();
        let st_out: HashMap<String, u64> = stage_times.lock().unwrap().clone();

        SerializableResult {
            total_tasks: plan.total_tasks,
            completed: comp,
            failed: fail,
            total_duration_ms: start.elapsed().as_millis() as u64,
            artifacts: artifacts_out,
            stage_times: st_out,
            executor_mode: format!("parallel-{}", worker_count),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adapter::{
        AdapterCapabilities, FileUnit, ImportEntry, ImportOutput, ParseOutput, ReferenceOutput,
        SymbolOutput,
    };

    struct MockAdapter;
    impl LanguageAdapter for MockAdapter {
        fn capabilities(&self) -> AdapterCapabilities {
            AdapterCapabilities {
                language: "mock".into(),
                adapter_version: "1.0".into(),
                parser_version: "1.0".into(),
                supports_parse: true,
                supports_symbols: true,
                supports_imports: true,
                supports_references: true,
                supports_calls: false,
                file_granularity: true,
                max_preferred_concurrency: Some(4),
                notes: vec![],
            }
        }
        fn discover_files(&self, _root: &str) -> Result<Vec<FileUnit>, String> {
            Ok((0..3)
                .map(|i| FileUnit {
                    id: format!("file_{}", i),
                    path: format!("file_{}.mock", i),
                    language: "mock".into(),
                    content_hash: Some(format!("hash_{}", i)),
                    size_bytes: 100,
                })
                .collect())
        }
        fn parse_file(&self, _unit: &FileUnit) -> Result<ParseOutput, String> {
            Ok(ParseOutput {
                unit_id: "x".into(),
                filename: "x".into(),
                ast_node_count: 10,
                tree_sitter_success: true,
                diagnostics: vec![],
                parse_duration_ms: 1,
            })
        }
        fn extract_symbols(&self, _unit: &FileUnit) -> Result<SymbolOutput, String> {
            Ok(SymbolOutput {
                unit_id: "x".into(),
                symbol_count: 2,
                symbols: vec![],
                duration_ms: 1,
            })
        }
        fn extract_imports(&self, _unit: &FileUnit) -> Result<ImportOutput, String> {
            Ok(ImportOutput {
                unit_id: "x".into(),
                import_count: 1,
                imports: vec![ImportEntry {
                    source: "a".into(),
                    target: "b".into(),
                    kind: "import".into(),
                    resolved: true,
                }],
                duration_ms: 1,
            })
        }
        fn extract_references(&self, _unit: &FileUnit) -> Result<ReferenceOutput, String> {
            Ok(ReferenceOutput {
                unit_id: "x".into(),
                call_count: 1,
                reference_count: 1,
                calls: vec![],
                duration_ms: 1,
            })
        }
    }

    fn make_plan() -> AnalysisPlan {
        AnalysisPlan {
            schema_version: "1.0".into(),
            root: "mock".into(),
            language: "mock".into(),
            total_tasks: 6,
            stages: vec![AnalysisStage::Parse, AnalysisStage::Symbol],
            parallelizable_tasks: 6,
            tasks: (0..3)
                .flat_map(|i| {
                    let id = format!("file_{}", i);
                    vec![
                        AnalysisTask {
                            id: format!("parse_{}", id),
                            stage: AnalysisStage::Parse,
                            root: "mock".into(),
                            language: "mock".into(),
                            unit_id: id.clone(),
                            depends_on: vec![],
                            cache_key: None,
                            parallelizable: true,
                        },
                        AnalysisTask {
                            id: format!("sym_{}", id),
                            stage: AnalysisStage::Symbol,
                            root: "mock".into(),
                            language: "mock".into(),
                            unit_id: id,
                            depends_on: vec![],
                            cache_key: None,
                            parallelizable: true,
                        },
                    ]
                })
                .collect(),
            estimated_stages: [("parse".into(), 3), ("symbol".into(), 3)].into(),
        }
    }

    #[test]
    fn serial_parallel_parity() {
        let plan = make_plan();
        let adapter = Arc::new(MockAdapter);

        let serial = SerialExecutor.execute(&plan, adapter.as_ref());
        let parallel = ParallelExecutor::new(4).execute(&plan, adapter.clone());

        // Collect results into owned Vecs
        let s_artifacts: Vec<_> = serial.artifacts.iter().map(|a| a.task_id.clone()).collect();
        let p_artifacts: Vec<_> = parallel
            .artifacts
            .iter()
            .map(|a| a.task_id.clone())
            .collect();

        assert_eq!(
            serial.completed, parallel.completed,
            "serial and parallel must complete same count"
        );
        assert_eq!(serial.failed, parallel.failed, "failure counts must match");
        assert_eq!(
            s_artifacts.len(),
            p_artifacts.len(),
            "artifact counts must match"
        );
    }
}
