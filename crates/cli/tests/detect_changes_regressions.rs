//! detect-changes 回归测试（2026-08-26 复核收尾）。
//!
//! 覆盖两个外部复核点名缺自动化回归的修复：
//! 1. `--base-ref`：三点拼接/HEAD 组合修复后，diff 结果与原生
//!    `git diff --merge-base <base>` 一致（不再 usage 报错/恒空 diff）。
//! 2. workspace auto 探测：根下散脚本 + 嵌套项目 marker 时报
//!    Ambiguous（引导显式 language），不再误选 Python 后
//!    not-compiled 崩溃。
//!
//! 这两个路径依赖 git 子进程与语言探测，走 CLI 端到端而非 lib 单测。

use std::path::{Path, PathBuf};
use std::process::Command;

fn bin_path() -> PathBuf {
    // 集成测试由 cargo 提供 env；找不到时退回 target/debug
    let env = std::env::var_os("CARGO_BIN_EXE_codelattice")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/debug/codelattice"));
    if env.exists() {
        return env;
    }
    PathBuf::from("../target/debug/codelattice")
}

fn run_detect_changes(root: &Path, args: &[&str]) -> (bool, String) {
    let out = Command::new(bin_path())
        .arg("detect-changes")
        .arg("--root")
        .arg(root)
        .args(args)
        .output()
        .expect("spawn codelattice");
    (
        out.status.success(),
        String::from_utf8_lossy(&out.stdout).to_string(),
    )
}

/// 构造 git 仓库：两个 commit，HEAD~1 与工作树 staged 各有差异。
fn make_repo(dir: &Path) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"dcreg\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(dir.join("src/lib.rs"), "pub fn one() -> u32 { 1 }\n").unwrap();
    let git = |args: &[&str]| {
        Command::new("git")
            .args(args)
            .current_dir(dir)
            .output()
            .expect("git");
    };
    git(&["init", "-q"]);
    git(&["add", "-A"]);
    git(&[
        "-c",
        "user.email=t@t",
        "-c",
        "user.name=t",
        "commit",
        "-qm",
        "one",
    ]);
    std::fs::write(
        dir.join("src/lib.rs"),
        "pub fn one() -> u32 { 1 }\npub fn two() -> u32 { 2 }\n",
    )
    .unwrap();
    git(&["add", "-A"]);
}

#[test]
fn base_ref_diff_matches_git_merge_base() {
    let tmp = std::env::temp_dir().join(format!("dcreg-base-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    make_repo(&tmp);

    // 提交 staged 作为第二 commit，HEAD~1 与 HEAD 有真实差异
    Command::new("git")
        .args([
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-qm",
            "two",
        ])
        .current_dir(&tmp)
        .output()
        .expect("git commit");

    let (ok, stdout) = run_detect_changes(&tmp, &["--base-ref", "HEAD~1", "--language", "rust"]);
    assert!(ok, "base-ref 调用失败: {stdout}");
    let parsed: serde_json::Value = serde_json::from_str(&stdout).expect("输出应为 JSON");
    let files = parsed["summary"]["changedFileCount"].as_u64().unwrap_or(0);
    assert!(
        files > 0,
        "base-ref HEAD~1 不应恒空 diff（修复前 HEAD 与 --merge-base 组合成共同祖先对比 HEAD）：{stdout}"
    );
    let paths: Vec<&str> = parsed["changedFiles"]
        .as_array()
        .map(|a| a.iter().filter_map(|f| f["path"].as_str()).collect())
        .unwrap_or_default();
    assert!(
        paths.contains(&"src/lib.rs"),
        "变更文件应含 src/lib.rs：{paths:?}"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn workspace_root_auto_detect_is_ambiguous_not_loose_python() {
    let tmp = std::env::temp_dir().join(format!("dcreg-auto-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    // 根：散工具脚本 + 嵌套 Rust/TS 项目 marker（open-nwe 形态）
    std::fs::create_dir_all(tmp.join("backend/src")).unwrap();
    std::fs::create_dir_all(tmp.join("frontend/src")).unwrap();
    std::fs::write(
        tmp.join("backend/Cargo.toml"),
        "[package]\nname = \"b\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(tmp.join("backend/src/lib.rs"), "pub fn b() {}\n").unwrap();
    std::fs::write(tmp.join("frontend/tsconfig.json"), "{}\n").unwrap();
    std::fs::write(tmp.join("update_index.py"), "print(1)\n").unwrap();

    let out = Command::new(bin_path())
        .arg("detect-changes")
        .arg("--root")
        .arg(&tmp)
        .output()
        .expect("spawn");
    let stderr = String::from_utf8_lossy(&out.stderr).to_string();
    let stdout = String::from_utf8_lossy(&out.stdout).to_string();
    // 修复前：误检 Python → "Python language support not compiled" 崩溃
    assert!(
        !stdout.contains("Python language support not compiled"),
        "不应再误选散脚本 Python 后 not-compiled 崩溃"
    );
    assert!(
        stderr.contains("多种清单") || stderr.contains("语言检测失败"),
        "workspace 根应报 Ambiguous 引导显式指定：{stderr}"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
