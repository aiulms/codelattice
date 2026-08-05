// Tauri Core —— 只做窗口生命周期、commands/channels、worker 编排与 SecretStore
// 桥接（P0 §5.2）。Gateway 业务在 understanding-gateway crate；本文件保持薄。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analyzer;
mod commands;
mod models;
mod snapshots;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use understanding_gateway::graph_store::SnapshotGraphIndex;
use understanding_gateway::secret::MemorySecretStore;
use understanding_gateway::service::UnderstandingService;

pub struct AppState {
    pub gateway: Mutex<UnderstandingService>,
    pub supervisor: Mutex<analyzer::AnalyzerSupervisor>,
    /// G3 选型：full immutable graph index 缓存（snapshotId -> 只读索引）。
    /// 懒加载 + 缓存；snapshot 原子发布后按 id 重载。与 Agent MCP 进程内
    /// cache 完全独立（§8.1）。
    pub query_store: Mutex<HashMap<String, Arc<SnapshotGraphIndex>>>,
    /// 进行中的流式请求取消标志（requestId -> flag；P0-B1 streaming/cancel）。
    pub active_requests: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// P0-C：被 agent/UI pin 的 snapshot id（cleanup 时保留）。
    pub pinned_snapshots: Mutex<Vec<String>>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            gateway: Mutex::new(UnderstandingService::new(Box::new(MemorySecretStore::new()))),
            supervisor: Mutex::new(analyzer::AnalyzerSupervisor::default()),
            query_store: Mutex::new(HashMap::new()),
            active_requests: Mutex::new(HashMap::new()),
            pinned_snapshots: Mutex::new(Vec::new()),
        }
    }
}

fn main() {
    tauri::Builder::default()
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::workbench_list_snapshots,
            commands::workbench_load_snapshot,
            commands::workbench_node_context,
            commands::workbench_edge_evidence,
            commands::workbench_call_chain,
            commands::workbench_models_list,
            commands::workbench_models_add,
            commands::workbench_models_remove,
            commands::workbench_models_set_default,
            commands::workbench_models_test,
            commands::workbench_secret_set,
            commands::workbench_secret_delete,
            commands::workbench_explain,
            commands::workbench_chat,
            commands::workbench_cancel,
            commands::workbench_selftest_enabled,
            commands::workbench_smoke_report,
            commands::workbench_analyze,
            commands::workbench_analyze_cancel,
            commands::workbench_analyze_status,
            commands::workbench_pin_snapshot,
            commands::workbench_unpin_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
