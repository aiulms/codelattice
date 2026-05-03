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

                // 解析 --include symbols / graph flag
                let include_symbols = include.iter().any(|s| s == "symbols");
                let include_graph = include.iter().any(|s| s == "graph");

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
    }
}
