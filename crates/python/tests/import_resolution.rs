//! Tests for Python module resolution.

use std::path::PathBuf;

fn import_resolution_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/python/import-resolution")
}

fn collect_py_files(root: &std::path::Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    collect_py_files_recursive(root, &mut files);
    files.sort();
    files
}

fn collect_py_files_recursive(dir: &std::path::Path, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with('.') && name != "__pycache__" && name != "node_modules" {
                    collect_py_files_recursive(&path, files);
                }
            } else if path.extension().and_then(|e| e.to_str()) == Some("py") {
                files.push(path);
            }
        }
    }
}

#[test]
fn module_index_maps_src_layout() {
    let root = import_resolution_root();
    let files = collect_py_files(&root);
    assert!(!files.is_empty(), "should find Python files in fixture");
    let idx = gitnexus_python::PythonModuleIndex::build(&root, &files);

    // src-layout: package root should be src/
    assert!(
        idx.package_roots.iter().any(|r| r.ends_with("src")),
        "package roots should include src/: {:?}",
        idx.package_roots
    );

    // Module mappings
    assert!(
        idx.module_to_file.contains_key("shop"),
        "should contain 'shop' module: {:?}",
        idx.module_to_file.keys().collect::<Vec<_>>()
    );
    assert!(idx.module_to_file.contains_key("shop.api"));
    assert!(idx.module_to_file.contains_key("shop.services"));
    assert!(idx.module_to_file.contains_key("shop.models"));
    assert!(idx.module_to_file.contains_key("shop.config"));
    assert!(idx.module_to_file.contains_key("shop.utils"));
    assert!(idx.module_to_file.contains_key("shop.utils.formatters"));
}

#[test]
fn relative_import_resolves_sibling() {
    let root = import_resolution_root();
    let files = collect_py_files(&root);
    let idx = gitnexus_python::PythonModuleIndex::build(&root, &files);

    let api_file = root.join("src/shop/api.py");
    let result = idx.resolve_import("services", Some("OrderService"), None, 1, &api_file);

    assert!(result.is_ok(), "should resolve: {:?}", result.err());
    let resolved = result.unwrap();
    assert_eq!(resolved.target_module, "shop.services");
    assert_eq!(resolved.target_symbol, Some("OrderService".to_string()));
    assert!(resolved.confidence >= 0.85);
}

#[test]
fn relative_import_resolves_parent_sibling() {
    let root = import_resolution_root();
    let files = collect_py_files(&root);
    let idx = gitnexus_python::PythonModuleIndex::build(&root, &files);

    // `from . import config as sibling_config` in shop/api.py
    // level=1, module_path="config" -> "shop.config"
    let api_file = root.join("src/shop/api.py");
    let result = idx.resolve_import("config", None, None, 1, &api_file);

    assert!(result.is_ok(), "should resolve: {:?}", result.err());
    let resolved = result.unwrap();
    assert_eq!(resolved.target_module, "shop.config");
}

#[test]
fn parent_relative_import_resolves() {
    let root = import_resolution_root();
    let files = collect_py_files(&root);
    let idx = gitnexus_python::PythonModuleIndex::build(&root, &files);

    let formatters_file = root.join("src/shop/utils/formatters.py");
    let result = idx.resolve_import(
        "config",
        Some("DEFAULT_CURRENCY"),
        None,
        2,
        &formatters_file,
    );

    assert!(result.is_ok(), "should resolve: {:?}", result.err());
    let resolved = result.unwrap();
    assert_eq!(resolved.target_module, "shop.config");
    assert_eq!(resolved.target_symbol, Some("DEFAULT_CURRENCY".to_string()));
}

#[test]
fn init_reexport_resolves() {
    let root = import_resolution_root();
    let files = collect_py_files(&root);
    let idx = gitnexus_python::PythonModuleIndex::build(&root, &files);

    let test_file = root.join("tests/test_api.py");
    // `from shop import create_order` should resolve through __init__.py re-export
    let result = idx.resolve_import("shop", Some("create_order"), None, 0, &test_file);

    assert!(result.is_ok(), "should resolve: {:?}", result.err());
    let resolved = result.unwrap();
    // Should resolve to shop.api.create_order via re-export chain
    assert!(
        resolved.confidence >= 0.75,
        "re-export confidence should be >= 0.75, got {}",
        resolved.confidence
    );
    assert_eq!(resolved.target_module, "shop.api");
    assert_eq!(resolved.target_symbol, Some("create_order".to_string()));
}

#[test]
fn module_index_handles_star_import_fixture() {
    let root = import_resolution_root();
    let files = collect_py_files(&root);
    let idx = gitnexus_python::PythonModuleIndex::build(&root, &files);

    // Module index should still work — star imports are handled at graph level
    assert!(idx.module_to_file.contains_key("shop.models"));
}

#[test]
fn absolute_import_resolves() {
    let root = import_resolution_root();
    let files = collect_py_files(&root);
    let idx = gitnexus_python::PythonModuleIndex::build(&root, &files);

    let test_file = root.join("tests/test_api.py");
    // `from shop.models import Order as PublicOrder`
    let result = idx.resolve_import(
        "shop.models",
        Some("Order"),
        Some("PublicOrder"),
        0,
        &test_file,
    );

    assert!(result.is_ok(), "should resolve: {:?}", result.err());
    let resolved = result.unwrap();
    assert_eq!(resolved.target_module, "shop.models");
    assert_eq!(resolved.target_symbol, Some("Order".to_string()));
    assert_eq!(resolved.alias, Some("PublicOrder".to_string()));
}

#[test]
fn re_exports_extracted_from_init() {
    let root = import_resolution_root();
    let files = collect_py_files(&root);
    let idx = gitnexus_python::PythonModuleIndex::build(&root, &files);

    let shop_exports = idx.re_exports.get("shop");
    assert!(
        shop_exports.is_some(),
        "shop should have re-exports: {:?}",
        idx.re_exports.keys().collect::<Vec<_>>()
    );
    let exports = shop_exports.unwrap();

    assert!(
        exports
            .iter()
            .any(|e| e.name == "create_order" && e.source_module == "shop.api"),
        "should re-export create_order from shop.api: {:?}",
        exports
    );
    assert!(
        exports
            .iter()
            .any(|e| e.name == "Order" && e.source_module == "shop.models"),
        "should re-export Order from shop.models: {:?}",
        exports
    );
}

#[test]
fn absolute_import_config() {
    let root = import_resolution_root();
    let files = collect_py_files(&root);
    let idx = gitnexus_python::PythonModuleIndex::build(&root, &files);

    let test_file = root.join("tests/test_api.py");
    // `from shop.config import load_settings`
    let result = idx.resolve_import("shop.config", Some("load_settings"), None, 0, &test_file);

    assert!(result.is_ok(), "should resolve: {:?}", result.err());
    let resolved = result.unwrap();
    assert_eq!(resolved.target_module, "shop.config");
}

#[test]
fn relative_import_with_alias() {
    let root = import_resolution_root();
    let files = collect_py_files(&root);
    let idx = gitnexus_python::PythonModuleIndex::build(&root, &files);

    let api_file = root.join("src/shop/api.py");
    // `from .config import DEFAULT_CURRENCY as CURRENCY`
    let result = idx.resolve_import(
        "config",
        Some("DEFAULT_CURRENCY"),
        Some("CURRENCY"),
        1,
        &api_file,
    );

    assert!(result.is_ok(), "should resolve: {:?}", result.err());
    let resolved = result.unwrap();
    assert_eq!(resolved.target_module, "shop.config");
    assert_eq!(resolved.target_symbol, Some("DEFAULT_CURRENCY".to_string()));
    assert_eq!(resolved.alias, Some("CURRENCY".to_string()));
}

#[test]
fn utils_init_reexport() {
    let root = import_resolution_root();
    let files = collect_py_files(&root);
    let idx = gitnexus_python::PythonModuleIndex::build(&root, &files);

    let utils_exports = idx.re_exports.get("shop.utils");
    assert!(utils_exports.is_some(), "shop.utils should have re-exports");
    let exports = utils_exports.unwrap();
    assert!(
        exports
            .iter()
            .any(|e| e.name == "format_price" && e.source_module == "shop.utils.formatters"),
        "should re-export format_price from shop.utils.formatters: {:?}",
        exports
    );
}
