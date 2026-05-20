use std::fs;
use std::time::{Duration, SystemTime};

use gitnexus_analysis_scheduler::{
    build_schedule, fingerprint_root, AnalysisRequest, AnalysisScope, CacheIntent,
};

#[test]
fn schedule_has_stable_phase_order_for_graph_analysis() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(
        temp.path().join("Cargo.toml"),
        "[package]\nname='demo'\nversion='0.1.0'\n",
    )
    .expect("write manifest");
    fs::create_dir(temp.path().join("src")).expect("src");
    fs::write(temp.path().join("src/lib.rs"), "pub fn live() {}\n").expect("write source");

    let request = AnalysisRequest::new(temp.path(), "rust")
        .with_scope(AnalysisScope::Graph)
        .with_strict(false)
        .with_cache_intent(CacheIntent::ReusePreferred);
    let schedule = build_schedule(&request).expect("schedule");

    let phase_names: Vec<_> = schedule
        .phases
        .iter()
        .map(|phase| phase.name.as_str())
        .collect();
    assert_eq!(
        phase_names,
        [
            "discover",
            "fingerprint",
            "parse",
            "symbols",
            "imports",
            "calls",
            "diagnostics",
            "graph",
        ]
    );
    assert_eq!(schedule.decision.cache_intent, CacheIntent::ReusePreferred);
    assert_eq!(schedule.request.language, "rust");
    assert!(schedule.fingerprint.tracked_file_count >= 2);
}

#[test]
fn fingerprint_changes_when_source_metadata_changes() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(temp.path().join("src")).expect("src");
    let source = temp.path().join("src/lib.rs");
    fs::write(&source, "pub fn first() {}\n").expect("write source");

    let before = fingerprint_root(temp.path()).expect("fingerprint before");
    std::thread::sleep(Duration::from_millis(20));
    fs::write(&source, "pub fn first() {}\npub fn second() {}\n").expect("update source");
    let after = fingerprint_root(temp.path()).expect("fingerprint after");

    assert_ne!(before.fingerprint, after.fingerprint);
    assert_eq!(after.tracked_file_count, 1);
    assert!(after.total_bytes > before.total_bytes);
}

#[test]
fn fingerprint_ignores_hidden_and_target_directories() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(temp.path().join("src")).expect("src");
    fs::write(temp.path().join("src/lib.rs"), "pub fn live() {}\n").expect("write source");

    fs::create_dir(temp.path().join(".git")).expect(".git");
    fs::write(temp.path().join(".git/index"), "private").expect("write hidden");
    fs::create_dir(temp.path().join("target")).expect("target");
    fs::write(temp.path().join("target/build.log"), "generated").expect("write target");

    let fingerprint = fingerprint_root(temp.path()).expect("fingerprint");

    assert_eq!(fingerprint.tracked_file_count, 1);
    assert_eq!(fingerprint.tracked_extensions, vec!["rs"]);
}

#[test]
fn schedule_records_stale_reason_when_previous_fingerprint_differs() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(temp.path().join("src")).expect("src");
    fs::write(temp.path().join("src/lib.rs"), "pub fn live() {}\n").expect("write source");
    let old = fingerprint_root(temp.path()).expect("old fingerprint");

    std::thread::sleep(Duration::from_millis(20));
    fs::write(
        temp.path().join("src/lib.rs"),
        "pub fn live() {}\npub fn newer() {}\n",
    )
    .expect("update source");

    let request = AnalysisRequest::new(temp.path(), "rust")
        .with_previous_fingerprint(old.fingerprint)
        .with_cache_intent(CacheIntent::ReusePreferred);
    let schedule = build_schedule(&request).expect("schedule");

    assert_eq!(schedule.decision.action, "fresh");
    assert_eq!(
        schedule.decision.reason.as_deref(),
        Some("fingerprint-changed")
    );
}

#[test]
fn dirty_file_plan_reports_modified_source_file() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::create_dir(temp.path().join("src")).expect("src");
    let source = temp.path().join("src/lib.rs");
    fs::write(&source, "pub fn live() {}\n").expect("write source");

    let before_request = AnalysisRequest::new(temp.path(), "rust")
        .with_scope(AnalysisScope::Graph)
        .with_cache_intent(CacheIntent::ReusePreferred);
    let before = build_schedule(&before_request).expect("initial schedule");

    std::thread::sleep(Duration::from_millis(20));
    fs::write(&source, "pub fn live() {}\npub fn changed() {}\n").expect("update source");

    let after_request = AnalysisRequest::new(temp.path(), "rust")
        .with_scope(AnalysisScope::Graph)
        .with_previous_fingerprint(before.fingerprint.fingerprint.clone())
        .with_previous_files(before.file_snapshot.clone())
        .with_cache_intent(CacheIntent::ReusePreferred);
    let after = build_schedule(&after_request).expect("dirty schedule");

    assert_eq!(after.decision.action, "fresh");
    assert!(after.incremental_plan.available);
    assert!(after.incremental_plan.plan_only);
    assert_eq!(after.incremental_plan.dirty_file_count, 1);
    assert_eq!(after.incremental_plan.summary.modified, 1);
    assert_eq!(after.incremental_plan.strategy, "fileScopedCandidate");
    assert_eq!(after.incremental_plan.dirty_files[0].path, "src/lib.rs");
    assert_eq!(after.incremental_plan.dirty_files[0].status, "modified");
    assert!(
        after
            .incremental_plan
            .affected_phases
            .iter()
            .any(|phase| phase == "graph"),
        "source changes should affect graph phase"
    );
}

#[test]
fn request_normalizes_root_without_requiring_stringly_paths() {
    let temp = tempfile::tempdir().expect("tempdir");
    fs::write(temp.path().join("README.md"), "demo").expect("write readme");

    let request =
        AnalysisRequest::new(temp.path(), "auto").with_generated_at(SystemTime::UNIX_EPOCH);
    let schedule = build_schedule(&request).expect("schedule");

    assert_eq!(schedule.request.language, "auto");
    let leaf = temp
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .expect("tempdir leaf");
    assert!(schedule.request.root.ends_with(leaf));
    assert_eq!(schedule.generated_at_ms, 0);
}
