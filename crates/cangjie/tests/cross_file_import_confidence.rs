//! Integration tests for cross-file import confidence and reason assertions.
//!
//! Verifies the confidence matrix for cross-file references:
//! - ExplicitImport → confidence=0.85, reason contains "cross-file via explicit import"
//! - PackageAlias → confidence=0.80, reason contains "cross-file via package alias"
//! - WildcardImport → confidence=0.70, reason contains "cross-file via wildcard import"
//! - Ambiguous (multiple matches) → no edge produced
//!
//! Uses `extract_cangjie_references` with `ImportBindingTable` to test the full
//! push_reference pipeline including confidence/reason assignment.
//!
//! Requires the `tree-sitter-cangjie` feature.

#![cfg(feature = "tree-sitter-cangjie")]

use gitnexus_cangjie::extractors::references::{
    extract_cangjie_references, ImportBinding, ImportBindingTable, ImportKind,
};
use gitnexus_cangjie::extractors::{
    extract_cangjie_symbols_from_tree, parse_cangjie_source, CangjieReference,
};
use std::collections::HashMap;
use std::path::PathBuf;

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

/// Source with a type annotation referencing "Calculator" — NOT defined in this file.
/// The same-file symbol index won't contain it, forcing cross-file resolution.
const SOURCE_WITH_CALCULATOR_REF: &str = r#"
package demo

main(): Int64 {
    let calc: Calculator = Calculator()
    calc.compute(42)
}
"#;

/// Fixture path pointing at imports-basic.
fn fixture_dir() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates/cangjie
    path.pop(); // crates
    path.push("fixtures");
    path.push("cangjie");
    path.push("imports-basic");
    path
}

fn fixture_source() -> String {
    let mut path = fixture_dir();
    path.push("src");
    path.push("main.cj");
    std::fs::read_to_string(&path).expect("failed to read fixture source")
}

/// Build an ImportBindingTable with explicit import bindings only.
fn build_explicit_import_table() -> ImportBindingTable {
    let mut bindings: HashMap<(String, String), Vec<ImportBinding>> = HashMap::new();

    bindings
        .entry(("main.cj".to_string(), "Calculator".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: "/path/to/demo/math/add.cj".to_string(),
            target_name: "Calculator".to_string(),
            package_prefix: None,
            import_kind: ImportKind::ExplicitImport,
        });

    ImportBindingTable::new(bindings)
}

/// Build an ImportBindingTable with package alias bindings.
fn build_package_alias_table() -> ImportBindingTable {
    let mut bindings: HashMap<(String, String), Vec<ImportBinding>> = HashMap::new();

    // Simulate: `import demo.math as math` with symbol "Calculator" discovered through the alias.
    // The binding table maps ("main.cj", "Calculator") → the target resolved via package alias.
    bindings
        .entry(("main.cj".to_string(), "Calculator".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: "/path/to/demo/math/add.cj".to_string(),
            target_name: "Calculator".to_string(),
            package_prefix: Some("demo.math".to_string()),
            import_kind: ImportKind::PackageAlias,
        });

    ImportBindingTable::new(bindings)
}

/// Build an ImportBindingTable with wildcard import binding.
fn build_wildcard_import_table() -> ImportBindingTable {
    let mut bindings: HashMap<(String, String), Vec<ImportBinding>> = HashMap::new();

    bindings
        .entry(("main.cj".to_string(), "Calculator".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: "/path/to/demo/math/add.cj".to_string(),
            target_name: "Calculator".to_string(),
            package_prefix: Some("demo.math".to_string()),
            import_kind: ImportKind::WildcardImport,
        });

    ImportBindingTable::new(bindings)
}

/// Build an ImportBindingTable with ambiguous bindings (two WildcardImports).
fn build_ambiguous_table() -> ImportBindingTable {
    let mut bindings: HashMap<(String, String), Vec<ImportBinding>> = HashMap::new();

    // First wildcard import expansion (from pkg1.*)
    bindings
        .entry(("main.cj".to_string(), "Calculator".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: "/path/to/pkg1/calculator.cj".to_string(),
            target_name: "Calculator".to_string(),
            package_prefix: Some("pkg1".to_string()),
            import_kind: ImportKind::WildcardImport,
        });

    // Second wildcard import expansion (from pkg2.*)
    bindings
        .entry(("main.cj".to_string(), "Calculator".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: "/path/to/pkg2/calculator.cj".to_string(),
            target_name: "Calculator".to_string(),
            package_prefix: Some("pkg2".to_string()),
            import_kind: ImportKind::WildcardImport,
        });

    ImportBindingTable::new(bindings)
}

/// Extract references from the inline source with the given binding table.
fn extract_refs_with_bindings(source: &str, table: &ImportBindingTable) -> Vec<CangjieReference> {
    let tree = parse_cangjie_source(source).expect("parse should succeed");
    let symbols =
        extract_cangjie_symbols_from_tree(source, &tree).expect("symbol extraction should succeed");
    let file_path = PathBuf::from("main.cj");

    extract_cangjie_references(source, &file_path, &symbols, &tree, Some(table))
        .expect("reference extraction should succeed")
}

/// Extract references from the inline source without binding table.
fn extract_refs_without_bindings(source: &str) -> Vec<CangjieReference> {
    let tree = parse_cangjie_source(source).expect("parse should succeed");
    let symbols =
        extract_cangjie_symbols_from_tree(source, &tree).expect("symbol extraction should succeed");
    let file_path = PathBuf::from("main.cj");

    extract_cangjie_references(source, &file_path, &symbols, &tree, None)
        .expect("reference extraction should succeed")
}

// -----------------------------------------------------------------------
// Tests
// -----------------------------------------------------------------------

#[test]
fn test_explicit_import_confidence_and_reason() {
    let table = build_explicit_import_table();
    let refs = extract_refs_with_bindings(SOURCE_WITH_CALCULATOR_REF, &table);

    // Find cross-file Calculator references (should have target_file set)
    let calculator_refs: Vec<&CangjieReference> = refs
        .iter()
        .filter(|r| r.target_name == "Calculator" && r.target_file.is_some())
        .collect();

    assert!(
        !calculator_refs.is_empty(),
        "should have at least one cross-file Calculator reference, got {} total refs: {:?}",
        refs.len(),
        refs.iter()
            .map(|r| format!("{} (target_file={:?})", r.target_name, r.target_file))
            .collect::<Vec<_>>()
    );

    for r in &calculator_refs {
        assert!(
            (r.confidence - 0.85f64).abs() < 0.01,
            "ExplicitImport confidence should be 0.85, got {}",
            r.confidence
        );
        assert!(
            r.reason.contains("cross-file via explicit import"),
            "ExplicitImport reason should contain 'cross-file via explicit import', got: {}",
            r.reason
        );
    }
}

#[test]
fn test_package_alias_confidence_and_reason() {
    let table = build_package_alias_table();
    let refs = extract_refs_with_bindings(SOURCE_WITH_CALCULATOR_REF, &table);

    // Find cross-file Calculator references
    let calculator_refs: Vec<&CangjieReference> = refs
        .iter()
        .filter(|r| r.target_name == "Calculator" && r.target_file.is_some())
        .collect();

    assert!(
        !calculator_refs.is_empty(),
        "should have at least one cross-file Calculator reference via package alias"
    );

    for r in &calculator_refs {
        assert!(
            (r.confidence - 0.80f64).abs() < 0.01,
            "PackageAlias confidence should be 0.80, got {}",
            r.confidence
        );
        assert!(
            r.reason.contains("cross-file via package alias"),
            "PackageAlias reason should contain 'cross-file via package alias', got: {}",
            r.reason
        );
    }
}

#[test]
fn test_wildcard_import_confidence_and_reason() {
    let table = build_wildcard_import_table();
    let refs = extract_refs_with_bindings(SOURCE_WITH_CALCULATOR_REF, &table);

    // Find cross-file Calculator references
    let calculator_refs: Vec<&CangjieReference> = refs
        .iter()
        .filter(|r| r.target_name == "Calculator" && r.target_file.is_some())
        .collect();

    assert!(
        !calculator_refs.is_empty(),
        "should have at least one cross-file Calculator reference via wildcard import"
    );

    for r in &calculator_refs {
        assert!(
            (r.confidence - 0.70f64).abs() < 0.01,
            "WildcardImport confidence should be 0.70, got {}",
            r.confidence
        );
        assert!(
            r.reason.contains("cross-file via wildcard import"),
            "WildcardImport reason should contain 'cross-file via wildcard import', got: {}",
            r.reason
        );
    }
}

#[test]
fn test_ambiguous_import_produces_no_edge() {
    let table = build_ambiguous_table();
    let refs_with_ambiguous = extract_refs_with_bindings(SOURCE_WITH_CALCULATOR_REF, &table);
    let refs_without_bindings = extract_refs_without_bindings(SOURCE_WITH_CALCULATOR_REF);

    // With ambiguous bindings, Calculator should NOT resolve cross-file.
    // The push_reference function silently drops ambiguous references.
    let ambiguous_calc_refs: Vec<&CangjieReference> = refs_with_ambiguous
        .iter()
        .filter(|r| r.target_name == "Calculator" && r.target_file.is_some())
        .collect();

    assert!(
        ambiguous_calc_refs.is_empty(),
        "ambiguous import should produce NO cross-file edge for Calculator, but found {} edges: {:?}",
        ambiguous_calc_refs.len(),
        ambiguous_calc_refs
            .iter()
            .map(|r| format!("conf={} reason={}", r.confidence, r.reason))
            .collect::<Vec<_>>()
    );

    // Total refs with ambiguous bindings should equal refs without bindings
    // (ambiguous bindings don't add any edges)
    assert_eq!(
        refs_with_ambiguous.len(),
        refs_without_bindings.len(),
        "ambiguous bindings should not change reference count"
    );
}

#[test]
fn test_confidence_ranking_explicit_beats_wildcard() {
    // When both explicit and wildcard bindings exist, explicit should win
    // with confidence 0.85
    let mut bindings: HashMap<(String, String), Vec<ImportBinding>> = HashMap::new();

    // Wildcard binding
    bindings
        .entry(("main.cj".to_string(), "Calculator".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: "/path/to/pkg1/calculator.cj".to_string(),
            target_name: "Calculator".to_string(),
            package_prefix: Some("pkg1".to_string()),
            import_kind: ImportKind::WildcardImport,
        });

    // Explicit binding (same key, should win via disambiguation)
    bindings
        .entry(("main.cj".to_string(), "Calculator".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: "/path/to/pkg2/calculator.cj".to_string(),
            target_name: "Calculator".to_string(),
            package_prefix: None,
            import_kind: ImportKind::ExplicitImport,
        });

    let table = ImportBindingTable::new(bindings);
    let refs = extract_refs_with_bindings(SOURCE_WITH_CALCULATOR_REF, &table);

    let calculator_refs: Vec<&CangjieReference> = refs
        .iter()
        .filter(|r| r.target_name == "Calculator" && r.target_file.is_some())
        .collect();

    assert!(
        !calculator_refs.is_empty(),
        "should resolve Calculator when explicit beats wildcard"
    );

    for r in &calculator_refs {
        assert!(
            (r.confidence - 0.85f64).abs() < 0.01,
            "disambiguated ExplicitImport should have confidence 0.85, got {}",
            r.confidence
        );
        assert!(
            r.reason.contains("cross-file via explicit import"),
            "disambiguated ExplicitImport reason should contain 'cross-file via explicit import', got: {}",
            r.reason
        );
    }
}

#[test]
fn test_ambiguous_mixed_kinds_produces_no_edge() {
    // When two PackageAlias bindings match the same symbol, it's ambiguous
    let mut bindings: HashMap<(String, String), Vec<ImportBinding>> = HashMap::new();

    bindings
        .entry(("main.cj".to_string(), "Calculator".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: "/path/to/a/calculator.cj".to_string(),
            target_name: "Calculator".to_string(),
            package_prefix: Some("pkga".to_string()),
            import_kind: ImportKind::PackageAlias,
        });

    bindings
        .entry(("main.cj".to_string(), "Calculator".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: "/path/to/b/calculator.cj".to_string(),
            target_name: "Calculator".to_string(),
            package_prefix: Some("pkgb".to_string()),
            import_kind: ImportKind::PackageAlias,
        });

    let table = ImportBindingTable::new(bindings);
    let refs = extract_refs_with_bindings(SOURCE_WITH_CALCULATOR_REF, &table);
    let refs_without = extract_refs_without_bindings(SOURCE_WITH_CALCULATOR_REF);

    let calc_refs: Vec<&CangjieReference> = refs
        .iter()
        .filter(|r| r.target_name == "Calculator" && r.target_file.is_some())
        .collect();

    assert!(
        calc_refs.is_empty(),
        "ambiguous PackageAlias pair should produce NO edge, found {}",
        calc_refs.len()
    );
    assert_eq!(
        refs.len(),
        refs_without.len(),
        "ambiguous bindings should not add edges"
    );
}

// -----------------------------------------------------------------------
// Fixture-based test: imports-basic with real ImportBindingTable
// -----------------------------------------------------------------------

#[test]
fn test_fixture_imports_basic_with_import_bindings() {
    let root = fixture_dir();
    let source = fixture_source();
    let tree = parse_cangjie_source(&source).expect("fixture should parse");
    let symbols = extract_cangjie_symbols_from_tree(&source, &tree)
        .expect("symbol extraction should succeed");
    let file_path = root.join("src").join("main.cj");

    // Build a manual binding table with an explicit import for "add"
    let mut bindings: HashMap<(String, String), Vec<ImportBinding>> = HashMap::new();
    bindings
        .entry((file_path.to_string_lossy().to_string(), "add".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: root
                .join("src")
                .join("demo")
                .join("math")
                .join("add.cj")
                .to_string_lossy()
                .to_string(),
            target_name: "add".to_string(),
            package_prefix: None,
            import_kind: ImportKind::ExplicitImport,
        });

    let table = ImportBindingTable::new(bindings);

    let refs = extract_cangjie_references(&source, &file_path, &symbols, &tree, Some(&table))
        .expect("reference extraction should succeed");

    // The fixture calls add(1, 2) — if "add" is not in same-file symbols,
    // it should resolve via the explicit import binding.
    let add_refs: Vec<&CangjieReference> = refs
        .iter()
        .filter(|r| r.target_name == "add" && r.target_file.is_some())
        .collect();

    // Note: "add" may or may not be in same-file symbols (it's imported, not defined).
    // If it resolves cross-file, verify confidence and reason.
    for r in &add_refs {
        assert!(
            (r.confidence - 0.85f64).abs() < 0.01,
            "explicit import for 'add' should have confidence 0.85, got {}",
            r.confidence
        );
        assert!(
            r.reason.contains("cross-file via explicit import"),
            "explicit import for 'add' reason should contain 'cross-file via explicit import', got: {}",
            r.reason
        );
    }
}
