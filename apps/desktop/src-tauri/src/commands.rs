// commands —— 薄命令层（返工 G-fix 拆分）。
//
// 原始 commands.rs 已达 991 行；拆分为独立子模块：
//   common    — 共享辅助函数（index_for、resolve_api_key 等）
//   evidence  — snapshot 查询命令
//   models    — 模型池管理
//   secrets   — SecretStore
//   sessions  — 会话生命周期
//   assistant — explain/chat 流式
//   analyzer  — Desktop Analyzer
//   selftest  — selftest/smoke
//
// 本文件只做 pub use 重导出，不再持有业务逻辑。

pub mod common;
pub mod evidence;
pub mod models;
pub mod secrets;
pub mod sessions;
pub mod assistant;
pub mod analyzer;
pub mod selftest;

// 重导出所有 Tauri 命令函数，供 main.rs invoke_handler 使用
pub use evidence::{
    workbench_list_snapshots,
    workbench_load_snapshot,
    workbench_node_context,
    workbench_edge_evidence,
    workbench_call_chain,
};
pub use models::{
    workbench_models_list,
    workbench_models_add,
    workbench_models_remove,
    workbench_models_set_default,
    workbench_models_test,
};
pub use secrets::{
    workbench_secret_set,
    workbench_secret_delete,
};
pub use sessions::{
    workbench_session_create,
    workbench_session_pin,
    workbench_session_close,
    workbench_select_directory,
};
pub use assistant::{
    workbench_explain,
    workbench_chat,
    workbench_cancel,
};
pub use analyzer::{
    workbench_analyze,
    workbench_analyze_cancel,
    workbench_analyze_status,
    workbench_pin_snapshot,
    workbench_unpin_snapshot,
};
pub use selftest::{
    workbench_selftest_enabled,
    workbench_smoke_report,
};
