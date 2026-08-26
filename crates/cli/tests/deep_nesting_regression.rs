//! 深层嵌套回归测试（analyze 栈溢出执行卡）
//!
//! fixture：700 层嵌套表达式，walk_node CST 递归深度 > open-nwe/backend
//! 实测的 562 层。标定（2026-08-24，诊断过程不入库）：
//! - 旧实现（主线程 8MB + rayon 池 8MB）下本 fixture exit 134（栈溢出）；
//!   <8 文件走 output.rs 串行分支，崩在主线程 walk_node。
//! - 修复后（CLI 主路径 16MB 线程 + item.rs rayon 池 16MB）exit 0。
//! assert_cmd 跑真实二进制：旧实现下天然变红，新实现必须绿。

use assert_cmd::Command;

fn cli() -> Command {
    Command::cargo_bin("codelattice").unwrap()
}

fn fixture_root() -> String {
    let base = std::env::current_dir().unwrap();
    let root = if base.join("fixtures").exists() {
        base
    } else if let Some(p) = base.parent().and_then(|p| p.parent()) {
        p.to_path_buf()
    } else {
        base
    };
    root.join("fixtures")
        .join("rust")
        .join("deep-nesting-smoke")
        .to_string_lossy()
        .to_string()
}

#[test]
fn deep_nesting_project_analyzes_without_stack_overflow_json() {
    let output = cli()
        .args([
            "analyze",
            "--root",
            &fixture_root(),
            "--language",
            "rust",
            "--format",
            "json",
        ])
        .output()
        .expect("run codelattice analyze");
    assert!(
        output.status.success(),
        "深嵌套项目 analyze 不允许栈溢出: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let d: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(d["schemaVersion"], "0.3.0");
    // 深嵌套 fn 与普通 helper 都要被提取到
    assert!(d["summary"]["symbolCount"].as_u64().unwrap() >= 2);
}

#[test]
fn deep_nesting_project_analyzes_without_stack_overflow_webui_snapshot() {
    // 覆盖序列化段：整条 analyze（含 webui-snapshot 转换）都跑在大栈线程里
    let output = cli()
        .args([
            "analyze",
            "--root",
            &fixture_root(),
            "--language",
            "rust",
            "--format",
            "webui-snapshot",
        ])
        .output()
        .expect("run codelattice analyze webui-snapshot");
    assert!(
        output.status.success(),
        "深嵌套项目 webui-snapshot 不允许栈溢出: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let d: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(d["schemaVersion"], "webui.snapshot.v1");
}
