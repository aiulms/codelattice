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
    },
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::ProjectModel { sub } => match sub {
            ProjectModelCommands::Inspect { root, format } => {
                if format != "json" {
                    eprintln!("错误：当前仅支持 --format json");
                    std::process::exit(1);
                }

                let root_path = Path::new(&root);
                if !root_path.exists() {
                    eprintln!("错误：root 路径不存在: {root}");
                    std::process::exit(1);
                }

                // 调用真实 manifest scanner
                let output = gitnexus_project_model::output::inspect_project_model(root_path);
                let json = serde_json::to_string_pretty(&output).unwrap_or_else(|e| {
                    eprintln!("错误：JSON 序列化失败: {e}");
                    std::process::exit(1);
                });

                // stdout 只输出 JSON，human logs 去 stderr
                println!("{json}");
            }
        },
    }
}
