//! understanding-gateway — CodeLattice Understanding Gateway（P0 §5.3）。
//!
//! 分层管道：Transport Adapter（Tauri/Web，本 crate 外）→ Session Manager →
//! Tool Dispatcher → Model Adapter → Output Validator → Cache Manager。
//! 纯业务逻辑，不依赖任何 Tauri/Web 类型；`worker` 模块提供 Desktop Analyzer
//! supervisor 的纯状态机（进程编排由 Tauri Core 层实现）。

pub mod cache;
pub mod dispatcher;
pub mod dto;
#[cfg(test)]
mod e2e_tests;
pub mod graph_store;
pub mod provider;
#[cfg(feature = "http")]
pub mod provider_http;
pub mod secret;
#[cfg(target_os = "macos")]
pub mod secret_keychain;
pub mod service;
pub mod session;
pub mod validator;
pub mod worker;

pub use dto::*;
