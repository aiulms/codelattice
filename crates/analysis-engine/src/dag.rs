//! Analysis DAG contract — unified types for analysis planning and execution.
//!
//! Analysis runs in phases (stages): discover → parse → symbol → import → reference → calls → merge.
//! Each phase produces named artifacts. Tasks express dependencies and cacheability.
//! Plans are serializable for debug/diagnosis.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Logical phase of analysis (ordinal ordering).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnalysisStage {
    Discover = 0,
    Fingerprint = 1,
    Parse = 2,
    Symbol = 3,
    Import = 4,
    Reference = 5,
    Calls = 6,
    Merge = 7,
}

impl AnalysisStage {
    pub fn name(&self) -> &'static str {
        match self {
            AnalysisStage::Discover => "discover",
            AnalysisStage::Fingerprint => "fingerprint",
            AnalysisStage::Parse => "parse",
            AnalysisStage::Symbol => "symbol",
            AnalysisStage::Import => "import",
            AnalysisStage::Reference => "reference",
            AnalysisStage::Calls => "calls",
            AnalysisStage::Merge => "merge",
        }
    }

    pub fn is_file_parallelizable(&self) -> bool {
        matches!(self, AnalysisStage::Parse | AnalysisStage::Symbol | AnalysisStage::Import | AnalysisStage::Reference)
    }

    pub fn depends_on(&self) -> Vec<AnalysisStage> {
        match self {
            AnalysisStage::Discover => vec![],
            AnalysisStage::Fingerprint => vec![AnalysisStage::Discover],
            AnalysisStage::Parse => vec![AnalysisStage::Discover],
            AnalysisStage::Symbol => vec![AnalysisStage::Parse],
            AnalysisStage::Import => vec![AnalysisStage::Parse],
            AnalysisStage::Reference => vec![AnalysisStage::Parse, AnalysisStage::Import],
            AnalysisStage::Calls => vec![AnalysisStage::Reference, AnalysisStage::Import],
            AnalysisStage::Merge => vec![AnalysisStage::Calls],
        }
    }
}

/// A single unit of work in the analysis DAG.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisTask {
    pub id: String,
    pub stage: AnalysisStage,
    pub root: String,
    pub language: String,
    pub unit_id: String,
    pub depends_on: Vec<String>,
    pub cache_key: Option<String>,
    pub parallelizable: bool,
}

/// Artifact produced by a completed task.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisArtifact {
    pub schema_version: String,
    pub task_id: String,
    pub stage: AnalysisStage,
    pub language: String,
    pub unit_id: String,
    pub cache_key: Option<String>,
    pub data: serde_json::Value,
    pub error: Option<String>,
    pub duration_ms: u64,
    pub generated_from: ArtifactSemantics,
}

/// Semantic metadata — static analysis only, no target code executed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtifactSemantics {
    pub static_analysis: bool,
    pub runtime_verified: bool,
    pub target_code_executed: bool,
    pub coverage_verified: bool,
    pub scripts_executed: bool,
}

impl Default for ArtifactSemantics {
    fn default() -> Self {
        Self {
            static_analysis: true,
            runtime_verified: false,
            target_code_executed: false,
            coverage_verified: false,
            scripts_executed: false,
        }
    }
}

/// Complete plan for one analysis root.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalysisPlan {
    pub schema_version: String,
    pub root: String,
    pub language: String,
    pub total_tasks: usize,
    pub stages: Vec<AnalysisStage>,
    pub tasks: Vec<AnalysisTask>,
    pub parallelizable_tasks: usize,
    pub estimated_stages: BTreeMap<String, usize>, // stage → task count
}

impl AnalysisPlan {
    pub fn tasks_for_stage(&self, stage: AnalysisStage) -> Vec<&AnalysisTask> {
        self.tasks.iter().filter(|t| t.stage == stage).collect()
    }

    pub fn debug_json(&self) -> serde_json::Value {
        serde_json::json!({
            "schemaVersion": self.schema_version,
            "root": self.root,
            "language": self.language,
            "totalTasks": self.total_tasks,
            "parallelizableTasks": self.parallelizable_tasks,
            "estimatedStages": self.estimated_stages,
            "tasks": self.tasks.iter().map(|t| serde_json::json!({
                "id": t.id,
                "stage": t.stage.name(),
                "unitId": t.unit_id,
                "dependsOn": t.depends_on,
                "cacheKey": t.cache_key,
                "parallelizable": t.parallelizable,
            })).collect::<Vec<_>>(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stage_ordering_is_correct() {
        assert!(AnalysisStage::Parse > AnalysisStage::Discover);
        assert!(AnalysisStage::Symbol > AnalysisStage::Parse);
        assert!(AnalysisStage::Merge > AnalysisStage::Calls);
    }

    #[test]
    fn file_parallelizable_stages() {
        assert!(AnalysisStage::Parse.is_file_parallelizable());
        assert!(AnalysisStage::Symbol.is_file_parallelizable());
        assert!(!AnalysisStage::Merge.is_file_parallelizable());
        assert!(!AnalysisStage::Discover.is_file_parallelizable());
    }

    #[test]
    fn artifact_semantics_default_is_static_only() {
        let s = ArtifactSemantics::default();
        assert!(s.static_analysis);
        assert!(!s.target_code_executed);
        assert!(!s.runtime_verified);
    }
}
