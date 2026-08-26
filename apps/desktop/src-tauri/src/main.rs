// Tauri Core —— 只做窗口生命周期、commands/channels、worker 编排与 SecretStore
// 桥接（P0 §5.2）。Gateway 业务在 understanding-gateway crate；本文件保持薄。

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod analyzer;
mod commands;
mod models;
mod query_store;
mod snapshots;

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use understanding_gateway::secret::SecretStore;
use understanding_gateway::secret_keychain::KeychainSecretStore;
use understanding_gateway::service::UnderstandingService;

pub struct AppState {
    pub gateway: Mutex<UnderstandingService>,
    pub supervisor: analyzer::AnalyzerSupervisor,
    /// 有界 LRU QueryStore（返工第二轮 D-fix）。
    pub query_store: Mutex<query_store::QueryStore>,
    /// 进行中的流式请求取消标志。
    pub active_requests: Mutex<HashMap<String, Arc<AtomicBool>>>,
    /// 被 agent/UI pin 的 snapshot id。
    pub pinned_snapshots: Mutex<Vec<String>>,
}

/// 生产 SecretStore：macOS Keychain（§7.2）；测试通过 env CODELATTICE_TEST_SECRET=1
/// 退回 MemorySecretStore 以避免污染用户钥匙串。
fn create_secret_store() -> Box<dyn SecretStore> {
    if std::env::var("CODELATTICE_TEST_SECRET")
        .map(|v| v == "1")
        .unwrap_or(false)
    {
        return Box::new(understanding_gateway::secret::MemorySecretStore::new());
    }
    Box::new(KeychainSecretStore::new())
}

impl AppState {
    pub fn new() -> Self {
        Self {
            gateway: Mutex::new(UnderstandingService::new(create_secret_store())),
            supervisor: analyzer::AnalyzerSupervisor::default(),
            query_store: Mutex::new(query_store::QueryStore::default()),
            active_requests: Mutex::new(HashMap::new()),
            pinned_snapshots: Mutex::new(Vec::new()),
        }
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::new())
        .setup(|_| {
            commands::selftest::trace("rust:setup");
            Ok(())
        })
        .on_page_load(|webview, payload| {
            commands::selftest::trace(&format!(
                "webview:{:?}:{}",
                payload.event(),
                payload.url()
            ));
            if std::env::var("CODELATTICE_SELFTEST").as_deref() == Ok("1")
                && matches!(payload.event(), tauri::webview::PageLoadEvent::Finished)
            {
                let _ = webview.eval(
                    r#"
                    (() => {
                      const invoke = window.__TAURI_INTERNALS__?.invoke;
                      if (!invoke) return;
                      const report = (kind, value) => invoke("workbench_selftest_probe", {
                        details: {
                          kind,
                          value: String(value ?? ""),
                          href: location.href,
                          readyState: document.readyState,
                          scripts: Array.from(document.scripts).map((script) => script.src || "inline"),
                          body: (document.body?.innerText || "").slice(0, 300)
                        }
                      }).catch(() => {});
                      report("page-finished", "ok");
                      window.addEventListener("error", (event) => report("error", event.message));
                      window.addEventListener("unhandledrejection", (event) => report("rejection", event.reason));
                    })();
                    "#,
                );
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::evidence::workbench_list_snapshots,
            commands::evidence::workbench_load_snapshot,
            commands::evidence::workbench_node_context,
            commands::evidence::workbench_edge_evidence,
            commands::evidence::workbench_call_chain,
            commands::models::workbench_models_list,
            commands::models::workbench_models_add,
            commands::models::workbench_models_update,
            commands::models::workbench_models_remove,
            commands::models::workbench_models_set_default,
            commands::models::workbench_models_test,
            commands::secrets::workbench_secret_set,
            commands::secrets::workbench_secret_delete,
            commands::assistant::workbench_explain,
            commands::assistant::workbench_chat,
            commands::assistant::workbench_cancel,
            commands::selftest::workbench_selftest_enabled,
            commands::selftest::workbench_selftest_probe,
            commands::selftest::workbench_smoke_report,
            commands::analyzer::workbench_analyze,
            commands::analyzer::workbench_inspect,
            commands::analyzer::workbench_analyze_cancel,
            commands::analyzer::workbench_analyze_status,
            commands::analyzer::workbench_pin_snapshot,
            commands::analyzer::workbench_unpin_snapshot,
            commands::sessions::workbench_session_create,
            commands::sessions::workbench_session_pin,
            commands::sessions::workbench_session_close,
            commands::evidence::workbench_query_store_metrics,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
