//! GitNexus Rust-core CLI
//!
//! 提供 project-model inspect / validate / fixtures list 命令。
//! MVP 只实现 inspect，输出 contract-compliant JSON 到 stdout。
//! Human-readable logs 输出到 stderr，确保 stdout 只包含 JSON，可管道到 jq / 文件。

use clap::{Parser, Subcommand};
use std::path::Path;

#[derive(Parser)]
#[command(
    name = "gitnexus-rust-core",
    version,
    about = "GitNexus Rust-core 复刻 CLI"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// ProjectModel 子命令域
    ProjectModel {
        #[command(subcommand)]
        sub: ProjectModelCommands,
    },
    /// Cangjie 子命令域
    Cangjie {
        #[command(subcommand)]
        sub: CangjieCommands,
    },
}

#[derive(Subcommand)]
enum ProjectModelCommands {
    /// 输出 ProjectModel JSON
    Inspect {
        /// repo 根目录路径
        #[arg(long)]
        root: String,
        /// 输出格式（MVP 仅支持 json）
        #[arg(long, default_value = "json")]
        format: String,
        /// 额外包含的数据（可多次指定）
        /// symbols: 提取 item/symbol 列表
        #[arg(long, value_name = "INCLUDE")]
        include: Vec<String>,
    },
}

#[derive(Subcommand)]
enum CangjieCommands {
    /// 输出 Cangjie 项目 JSON
    Inspect {
        /// 项目根目录路径
        #[arg(long)]
        root: String,
    },
    /// 输出 Cangjie 图 JSON
    Graph {
        /// 项目根目录路径
        #[arg(long)]
        root: String,
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::ProjectModel { sub } => match sub {
            ProjectModelCommands::Inspect {
                root,
                format,
                include,
            } => {
                if format != "json" {
                    eprintln!("错误：当前仅支持 --format json");
                    std::process::exit(1);
                }

                // 解析 --include symbols / graph / imports flag
                let include_symbols = include.iter().any(|s| s == "symbols");
                let include_graph = include.iter().any(|s| s == "graph");
                let include_imports = include.iter().any(|s| s == "imports");
                let include_calls = include.iter().any(|s| s == "calls");

                let root_path = Path::new(&root);
                if !root_path.exists() {
                    eprintln!("错误：root 路径不存在: {root}");
                    std::process::exit(1);
                }

                // 调用真实 manifest scanner
                let pm_output = gitnexus_project_model::output::inspect_project_model_with_options(
                    root_path,
                    include_symbols,
                    include_graph,
                    include_imports,
                    include_calls,
                );

                // 输出：--include graph 时输出 GraphOutput，否则输出 ProjectModelOutput
                if include_graph {
                    let graph_output =
                        gitnexus_project_model::output::emit_graph_output(&pm_output);
                    let json = serde_json::to_string_pretty(&graph_output).unwrap_or_else(|e| {
                        eprintln!("错误：Graph JSON 序列化失败: {e}");
                        std::process::exit(1);
                    });
                    println!("{json}");
                } else {
                    let json = serde_json::to_string_pretty(&pm_output).unwrap_or_else(|e| {
                        eprintln!("错误：JSON 序列化失败: {e}");
                        std::process::exit(1);
                    });
                    println!("{json}");
                }
            }
        },
        Commands::Cangjie { sub } => match sub {
            CangjieCommands::Inspect { root } | CangjieCommands::Graph { root } => {
                // Feature gate check
                #[cfg(not(feature = "tree-sitter-cangjie"))]
                {
                    let _root = root; // Suppress unused variable warning
                    eprintln!("错误：Cangjie support is disabled.");
                    eprintln!("请使用 --features tree-sitter-cangjie 重新编译：");
                    eprintln!("  cargo run --features tree-sitter-cangjie -p gitnexus-rust-core-cli -- cangjie inspect --root <path>");
                    std::process::exit(1);
                }

                #[cfg(feature = "tree-sitter-cangjie")]
                {
                    let root_path = Path::new(&root);
                    if !root_path.exists() {
                        eprintln!("错误：root 路径不存在: {root}");
                        std::process::exit(1);
                    }

                    match gitnexus_cangjie::graph::inspect_cangjie_project(root_path) {
                        Ok(graph_output) => {
                            let json =
                                serde_json::to_string_pretty(&graph_output).unwrap_or_else(|e| {
                                    eprintln!("错误：Cangjie JSON 序列化失败: {e}");
                                    std::process::exit(1);
                                });
                            println!("{json}");
                        }
                        Err(e) => {
                            eprintln!("错误：Cangjie 项目分析失败: {e}");
                            std::process::exit(1);
                        }
                    }
                }
            }
        },
    }
}
