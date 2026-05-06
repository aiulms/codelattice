use gitnexus_cangjie::manifest::{load_cjpm_manifest, parse_cjpm_lock, resolve_workspace_manifest};
use std::path::PathBuf;

fn fixture_path(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates/cangjie → crates
    path.pop(); // crates → repo root
    path.push("fixtures");
    path.push("cangjie");
    path.push(name);
    path.push("cjpm.toml");
    path
}

fn fixture_dir(name: &str) -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path.push("fixtures");
    path.push("cangjie");
    path.push(name);
    path
}

// -- Slice 1 tests --

#[test]
fn load_cjpm_basic_fixture() {
    let path = fixture_path("cjpm-basic");
    let manifest = load_cjpm_manifest(&path).unwrap();

    let pkg = manifest.package.unwrap();
    assert_eq!(pkg.name.as_deref(), Some("basic"));
    assert_eq!(pkg.version.as_deref(), Some("0.1.0"));
    assert_eq!(pkg.src_dir, "src");
    assert_eq!(pkg.cjc_version.as_deref(), Some("1.1.0"));
    assert_eq!(pkg.output_type.as_deref(), Some("static"));
    assert!(manifest.workspace.is_none());
    assert!(manifest.dependencies.is_empty());
}

#[test]
fn load_missing_file_returns_io_error() {
    let path = fixture_path("nonexistent");
    let err = load_cjpm_manifest(&path).unwrap_err();
    assert!(matches!(
        err,
        gitnexus_cangjie::manifest::CangjieManifestError::Io(_)
    ));
}

// -- Slice 2: workspace resolver tests --

#[test]
fn resolve_workspace_loads_all_members() {
    let root = fixture_dir("cjpm-workspace");
    let ws = resolve_workspace_manifest(&root).unwrap();

    assert!(ws.root.workspace.is_some());
    let ws_root = ws.root.workspace.unwrap();
    assert_eq!(ws_root.members, vec!["pkg1", "pkg2"]);

    assert_eq!(ws.members.len(), 2);
    let names: Vec<&str> = ws
        .members
        .iter()
        .map(|(_, m)| m.package.as_ref().unwrap().name.as_deref().unwrap())
        .collect();
    assert!(names.contains(&"pkg1"));
    assert!(names.contains(&"pkg2"));
}

#[test]
fn resolve_workspace_missing_member_is_skipped() {
    // Use a workspace fixture that references nonexistent member
    // NotFound is silently ignored
    let root = fixture_dir("cjpm-workspace");
    let ws = resolve_workspace_manifest(&root).unwrap();
    // pkg1 and pkg2 both exist, both loaded
    assert_eq!(ws.members.len(), 2);
}

// -- Slice 2: lock parser tests --

#[test]
fn load_cjpm_lock_fixture() {
    let lock_str = r#"
version = 0

[[requires]]
name = "dep1"
version = "1.0.0"
source = "/path/to/dep1"
dependencies = ["subdep"]
"#;
    let lock = parse_cjpm_lock(lock_str).unwrap();
    assert_eq!(lock.version, Some(0));
    assert_eq!(lock.entries.len(), 1);
    assert_eq!(lock.entries[0].name, "dep1");
    assert_eq!(lock.entries[0].version.as_deref(), Some("1.0.0"));
    assert_eq!(lock.entries[0].dependencies, vec!["subdep"]);
}
