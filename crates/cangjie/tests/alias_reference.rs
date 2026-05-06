//! Integration tests for alias reference resolution.
//!
//! Tests grouped import alias and package alias support.

use gitnexus_cangjie::extractors::imports::{parse_named_import_candidates, ImportCandidate};
use gitnexus_cangjie::extractors::references::{ImportBinding, ImportBindingTable};
use std::collections::HashMap;

// -----------------------------------------------------------------------
// Grouped import alias tests
// -----------------------------------------------------------------------

#[test]
fn test_grouped_import_with_alias() {
    // `demo.math.{add, sub as subtract}`
    let candidates = parse_named_import_candidates("demo.math.{add, sub as subtract}");
    assert_eq!(candidates.len(), 2);

    let add = candidates.iter().find(|c| c.local_name == "add").unwrap();
    assert_eq!(add.package_name, "demo.math");
    assert_eq!(add.exported_name, "add");
    assert_eq!(add.local_name, "add");

    let subtract = candidates
        .iter()
        .find(|c| c.local_name == "subtract")
        .unwrap();
    assert_eq!(subtract.package_name, "demo.math");
    assert_eq!(subtract.exported_name, "sub");
    assert_eq!(subtract.local_name, "subtract");
}

#[test]
fn test_grouped_import_multiple_aliases() {
    // `pkg.{a as b, c as d, e}`
    let candidates = parse_named_import_candidates("pkg.{a as b, c as d, e}");
    assert_eq!(candidates.len(), 3);

    assert_eq!(candidates[0].exported_name, "a");
    assert_eq!(candidates[0].local_name, "b");

    assert_eq!(candidates[1].exported_name, "c");
    assert_eq!(candidates[1].local_name, "d");

    assert_eq!(candidates[2].exported_name, "e");
    assert_eq!(candidates[2].local_name, "e");
}

#[test]
fn test_grouped_import_no_alias() {
    // `pkg.{a, b, c}` - no aliases, should still work
    let candidates = parse_named_import_candidates("pkg.{a, b, c}");
    assert_eq!(candidates.len(), 3);

    for candidate in &candidates {
        assert_eq!(candidate.package_name, "pkg");
        assert_eq!(candidate.exported_name, candidate.local_name);
    }
}

#[test]
fn test_simple_alias_still_works() {
    // `demo.math.add as plus`
    let candidates = parse_named_import_candidates("demo.math.add as plus");
    assert_eq!(candidates.len(), 1);

    let candidate = &candidates[0];
    assert_eq!(candidate.package_name, "demo.math");
    assert_eq!(candidate.exported_name, "add");
    assert_eq!(candidate.local_name, "plus");
}

// -----------------------------------------------------------------------
// Package alias tests (reference resolution)
// -----------------------------------------------------------------------

#[test]
fn test_import_binding_with_package_prefix() {
    // Create a binding table with package prefix support
    let mut bindings: HashMap<(String, String), Vec<ImportBinding>> = HashMap::new();

    // Import binding: `import pkg as p` → package_prefix = "pkg"
    bindings
        .entry(("main.cj".to_string(), "p".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: String::new(),
            target_name: String::new(),
            package_prefix: Some("pkg".to_string()),
        });

    // Regular import binding: `import pkg.Func` → direct binding
    bindings
        .entry(("main.cj".to_string(), "pkg.Func".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: "/path/to/pkg/func.cj".to_string(),
            target_name: "Func".to_string(),
            package_prefix: None,
        });

    let table = ImportBindingTable::new(bindings);

    // Test that "p.Func" resolves through package alias
    let resolved = table.resolve("main.cj", "p.Func");
    assert!(
        resolved.is_some(),
        "p.Func should resolve through package alias"
    );
    let binding = resolved.unwrap();
    assert_eq!(binding.target_name, "Func");
    assert_eq!(binding.target_file, "/path/to/pkg/func.cj");
}

#[test]
#[ignore]
fn test_import_binding_exact_match_priority() {
    // Exact match should have priority over package prefix matching
    let mut bindings: HashMap<(String, String), Vec<ImportBinding>> = HashMap::new();

    // Simulate a scenario where we have:
    // 1. A wildcard import that could match "Func" (from package alias expansion)
    // 2. An explicit import that matches "Func" exactly

    // Package alias binding (simulating wildcard expansion result)
    bindings
        .entry(("main.cj".to_string(), "Func".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: "/path/to/pkg/func.cj".to_string(),
            target_name: "Func".to_string(),
            package_prefix: Some("pkg".to_string()), // This came from wildcard import
        });

    // Exact match for direct import
    bindings
        .entry(("main.cj".to_string(), "Func".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: "/path/to/other/func.cj".to_string(),
            target_name: "Func".to_string(),
            package_prefix: None, // Explicit import
        });

    let table = ImportBindingTable::new(bindings);

    // Exact match should take priority over wildcard-expanded alias
    let resolved = table.resolve("main.cj", "Func");
    assert!(
        resolved.is_some(),
        "Func should resolve via exact match priority"
    );
    let binding = resolved.unwrap();
    assert_eq!(binding.target_name, "Func");
    assert_eq!(binding.package_prefix, None); // Should prefer the explicit import
}

#[test]
#[ignore]
fn test_import_binding_no_ambiguous_resolution() {
    // Multiple exact matches should return None (no fake edge)
    let mut bindings: HashMap<(String, String), Vec<ImportBinding>> = HashMap::new();

    // Simulate multiple wildcard imports that could resolve to the same symbol
    // Both could match "Func", making it truly ambiguous

    // First wildcard import expansion (from pkg1.*)
    bindings
        .entry(("main.cj".to_string(), "Func".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: "/path/to/pkg1/func.cj".to_string(),
            target_name: "Func".to_string(),
            package_prefix: Some("pkg1".to_string()),
        });

    // Second wildcard import expansion (from pkg2.*)
    bindings
        .entry(("main.cj".to_string(), "Func".to_string()))
        .or_default()
        .push(ImportBinding {
            target_file: "/path/to/pkg2/func.cj".to_string(),
            target_name: "Func".to_string(),
            package_prefix: Some("pkg2".to_string()),
        });

    let table = ImportBindingTable::new(bindings);

    let resolved = table.resolve("main.cj", "Func");
    assert!(
        resolved.is_none(),
        "Ambiguous resolution should return None"
    );
}

// -----------------------------------------------------------------------
// Combined scenarios
// -----------------------------------------------------------------------

#[test]
fn test_grouped_and_simple_alias_together() {
    // Test that grouped imports with aliases work alongside simple aliases
    let candidates = parse_named_import_candidates("pkg.{a, b as c}");
    assert_eq!(candidates.len(), 2);

    let a = candidates.iter().find(|c| c.local_name == "a").unwrap();
    assert_eq!(a.exported_name, "a");
    assert_eq!(a.local_name, "a");

    let c = candidates.iter().find(|c| c.local_name == "c").unwrap();
    assert_eq!(c.exported_name, "b");
    assert_eq!(c.local_name, "c");
}

#[test]
fn test_nested_grouped_import_with_alias() {
    // Test that complex grouped imports work
    let candidates =
        parse_named_import_candidates("demo.math.{add, sub as subtract, mul as multiply}");
    assert_eq!(candidates.len(), 3);

    let names: Vec<_> = candidates.iter().map(|c| c.local_name.as_str()).collect();
    assert!(names.contains(&"add"));
    assert!(names.contains(&"subtract"));
    assert!(names.contains(&"multiply"));

    // Verify the exported names are correct
    let subtract = candidates
        .iter()
        .find(|c| c.local_name == "subtract")
        .unwrap();
    assert_eq!(subtract.exported_name, "sub");

    let multiply = candidates
        .iter()
        .find(|c| c.local_name == "multiply")
        .unwrap();
    assert_eq!(multiply.exported_name, "mul");
}
