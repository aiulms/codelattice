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
// 本文件只声明子模块；main.rs 使用实际模块路径注册命令。

pub mod analyzer;
pub mod assistant;
pub mod common;
pub mod evidence;
pub mod models;
pub mod secrets;
pub mod selftest;
pub mod sessions;
