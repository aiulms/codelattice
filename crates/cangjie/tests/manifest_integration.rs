use gitnexus_cangjie::manifest::load_cjpm_manifest;
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
