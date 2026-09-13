//! bin+lib 同包跨 target 调用回归测试（2026-08-26 调用边修复）。
//!
//! 复现结构：`src/main.rs`（bin）`use <pkg>::discover_existing` 调用
//! lib 侧 `src/instance.rs` 的函数，经 `lib.rs` 的 `pub use` re-export。
//! 修复前：main.rs 的 use 被判为 external crate 跳过、instance.rs 的符号
//! 因多 target AmbiguousTarget 无 package 归属 → crate-wide 搜索失联 →
//! CALLS 边为 0。

fn write_fixture(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join("src")).unwrap();
    std::fs::write(
        dir.join("Cargo.toml"),
        "[package]\nname = \"binlib\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/lib.rs"),
        "pub mod instance;\npub use instance::discover_existing;\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/instance.rs"),
        "pub fn discover_existing(x: u32) -> u32 { x + 1 }\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/main.rs"),
        "use binlib::discover_existing;\nfn main() { let _ = discover_existing(1); }\n",
    )
    .unwrap();
}

#[test]
fn bin_to_lib_reexport_call_edge_exists() {
    let tmp = std::env::temp_dir().join(format!("binlib-test-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    write_fixture(&tmp);

    let output = gitnexus_project_model::output::inspect_project_model_with_options(
        &tmp, true, true, false, true,
    );

    let target_id = "binlib::crate::instance::discover_existing";

    let calls: Vec<_> = output
        .calls
        .iter()
        .filter(|c| c.callee_name == "discover_existing")
        .collect();
    assert!(
        !calls.is_empty(),
        "main.rs 中 discover_existing 调用点未被提取"
    );
    let resolved: Vec<_> = calls
        .iter()
        .filter(|c| c.resolved_symbol_id.as_deref() == Some(target_id))
        .collect();
    assert!(
        !resolved.is_empty(),
        "调用点未解析到 lib 侧符号 {target_id}；实际解析: {:?}",
        calls
            .iter()
            .map(|c| (&c.reason, &c.resolved_symbol_id))
            .collect::<Vec<_>>()
    );

    // 边在 graph emitter 生成（calls resolved → CALLS edge）；
    // resolved_symbol_id 已断言，graph 层由 CLI 端到端测试覆盖。

    let _ = std::fs::remove_dir_all(&tmp);
}

#[test]
fn lib_module_symbols_have_package_for_crate_wide_search() {
    let tmp = std::env::temp_dir().join(format!("binlib-own-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    write_fixture(&tmp);

    let output = gitnexus_project_model::output::inspect_project_model_with_options(
        &tmp, true, false, false, false,
    );

    // instance.rs（非 target-root 文件）修复前 package=None → 失联
    let inst = output
        .source_ownership
        .iter()
        .find(|s| s.source_path == "src/instance.rs")
        .expect("instance.rs 应在 ownership 列表");
    assert!(
        inst.package.is_some(),
        "bin+lib 包内非 target-root 文件不应无 package 归属（修复前 AmbiguousTarget 置 None）"
    );

    let _ = std::fs::remove_dir_all(&tmp);
}
