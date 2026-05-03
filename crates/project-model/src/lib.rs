//! GitNexus Rust-core ProjectModel 库
//!
//! 提供 ProjectModel 输出类型和 CLI/output contract 冻结的序列化格式。
//! 第一刀实现 manifest scanner：读取 Cargo.toml，发现 package/workspace/target。
//! 第二刀实现 source ownership scanner：判定 .rs 文件归属的 package/target。
//! 第三刀实现 root resolution scanner：解析 crate:: 路径到 module source file。

pub mod diagnostic;
pub mod graph;
pub mod imports;
pub mod item;
pub mod manifest;
pub mod model;
pub mod output;
pub mod root_resolution;
pub mod source;
