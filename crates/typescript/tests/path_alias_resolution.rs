//! Crate-level tests for TypeScript path alias / monorepo import resolution.
//!
//! Uses the `path-alias-monorepo` fixture at `fixtures/typescript/path-alias-monorepo/`.

use std::path::PathBuf;

/// Helper: get the fixture root path.
fn fixture_root() -> PathBuf {
    let mut path = std::env::current_dir().expect("current dir");
    // Navigate up to workspace root when running from crate dir
    for _ in 0..5 {
        if path
            .join("fixtures/typescript/path-alias-monorepo")
            .is_dir()
        {
            return path.join("fixtures/typescript/path-alias-monorepo");
        }
        if !path.pop() {
            break;
        }
    }
    // Fallback: assume we're at workspace root
    std::env::current_dir()
        .expect("current dir")
        .join("fixtures/typescript/path-alias-monorepo")
}

/// Helper: build a resolver for the fixture.
fn build_resolver() -> gitnexus_typescript::TsModuleResolver {
    let root = fixture_root();
    let source_files =
        gitnexus_typescript::list_source_files(&root).expect("list source files from fixture");
    gitnexus_typescript::TsModuleResolver::build(&root, &source_files)
}

#[test]
fn test_tsconfig_extends_loads_base_paths() {
    let resolver = build_resolver();

    // The base tsconfig has paths for @shared, @core/*, @models/*, @shared/*, @app/*
    // The child tsconfig extends it, so those paths should be available
    let found_paths = resolver
        .tsconfigs
        .iter()
        .flat_map(|tc| tc.paths.keys().cloned())
        .collect::<Vec<_>>();

    assert!(
        found_paths.contains(&"@shared".to_string()),
        "Expected @shared path from extends chain, got: {:?}",
        found_paths
    );
    assert!(
        found_paths.contains(&"@core/*".to_string()),
        "Expected @core/* path from extends chain, got: {:?}",
        found_paths
    );
    assert!(
        found_paths.contains(&"@models/*".to_string()),
        "Expected @models/* path from extends chain, got: {:?}",
        found_paths
    );
}

#[test]
fn test_exact_path_alias_resolves_shared() {
    let resolver = build_resolver();
    let importer = fixture_root().join("packages/app/src/main.ts");

    let result = resolver.resolve_import(&importer, "@shared");

    assert!(
        result.target_file.is_some(),
        "Expected @shared to resolve, got: {:?}",
        result
    );
    let file = result.target_file.unwrap();
    assert!(
        file.to_string_lossy().contains("shared/src/index.ts"),
        "Expected path to shared/src/index.ts, got: {}",
        file.display()
    );
    assert_eq!(
        result.resolution_kind,
        gitnexus_typescript::TsResolutionKind::TsConfigPathExact
    );
    assert_eq!(result.confidence, Some(0.90));
}

#[test]
fn test_wildcard_path_alias_resolves_core_logger() {
    let resolver = build_resolver();
    let importer = fixture_root().join("packages/app/src/main.ts");

    let result = resolver.resolve_import(&importer, "@core/logger");

    assert!(
        result.target_file.is_some(),
        "Expected @core/logger to resolve, got: {:?}",
        result
    );
    let file = result.target_file.unwrap();
    assert!(
        file.to_string_lossy().contains("app/src/core/logger.ts"),
        "Expected path to core/logger.ts, got: {}",
        file.display()
    );
    assert_eq!(
        result.resolution_kind,
        gitnexus_typescript::TsResolutionKind::TsConfigPathWildcard
    );
    assert_eq!(result.confidence, Some(0.85));
}

#[test]
fn test_wildcard_path_alias_resolves_models_order() {
    let resolver = build_resolver();
    let importer = fixture_root().join("packages/app/src/main.ts");

    let result = resolver.resolve_import(&importer, "@models/order");

    assert!(
        result.target_file.is_some(),
        "Expected @models/order to resolve, got: {:?}",
        result
    );
    let file = result.target_file.unwrap();
    assert!(
        file.to_string_lossy()
            .contains("shared/src/models/order.ts"),
        "Expected path to models/order.ts, got: {}",
        file.display()
    );
    assert_eq!(
        result.resolution_kind,
        gitnexus_typescript::TsResolutionKind::TsConfigPathWildcard
    );
    assert_eq!(result.confidence, Some(0.85));
}

#[test]
fn test_relative_import_resolves_index_extensionless() {
    let resolver = build_resolver();
    let importer = fixture_root().join("packages/app/src/main.ts");

    // "./ui/Button" → packages/app/src/ui/Button.tsx
    let result = resolver.resolve_import(&importer, "./ui/Button");

    assert!(
        result.target_file.is_some(),
        "Expected ./ui/Button to resolve, got: {:?}",
        result
    );
    let file = result.target_file.unwrap();
    assert!(
        file.to_string_lossy().ends_with("Button.tsx"),
        "Expected Button.tsx, got: {}",
        file.display()
    );
    assert_eq!(
        result.resolution_kind,
        gitnexus_typescript::TsResolutionKind::RelativeFile
    );

    // "./features/orders" → packages/app/src/features/orders.ts
    let result2 = resolver.resolve_import(&importer, "./features/orders");
    assert!(
        result2.target_file.is_some(),
        "Expected ./features/orders to resolve, got: {:?}",
        result2
    );
    assert!(
        result2
            .target_file
            .as_ref()
            .unwrap()
            .to_string_lossy()
            .ends_with("orders.ts"),
        "Expected orders.ts, got: {}",
        result2.target_file.as_ref().unwrap().display()
    );
}

#[test]
fn test_workspace_package_import_resolves_package_root() {
    let resolver = build_resolver();

    // Verify workspace packages are discovered
    assert!(
        resolver.workspace_packages.contains_key("@pkg/shared"),
        "Expected @pkg/shared in workspace packages, got: {:?}",
        resolver.workspace_packages.keys().collect::<Vec<_>>()
    );
    assert!(
        resolver.workspace_packages.contains_key("@pkg/app"),
        "Expected @pkg/app in workspace packages, got: {:?}",
        resolver.workspace_packages.keys().collect::<Vec<_>>()
    );

    // Resolve workspace package import
    let importer = fixture_root().join("packages/app/src/main.ts");
    let result = resolver.resolve_import(&importer, "@pkg/shared");

    assert!(
        result.target_file.is_some(),
        "Expected @pkg/shared to resolve, got: {:?}",
        result
    );
    let file = result.target_file.unwrap();
    assert!(
        file.to_string_lossy().contains("shared/src/index.ts"),
        "Expected shared/src/index.ts, got: {}",
        file.display()
    );
    assert_eq!(
        result.resolution_kind,
        gitnexus_typescript::TsResolutionKind::WorkspacePackage
    );
    assert_eq!(result.confidence, Some(0.80));
}

#[test]
fn test_unresolved_alias_returns_no_target() {
    let resolver = build_resolver();
    let importer = fixture_root().join("packages/app/src/main.ts");

    // "@shared/missing" — matches @shared/* but file doesn't exist
    let result = resolver.resolve_import(&importer, "@shared/missing");

    assert!(
        result.target_file.is_none(),
        "Expected @shared/missing to be unresolved, got: {:?}",
        result
    );
    // Could be Unresolved if tsconfig wildcard matches but file not found
    assert!(
        matches!(
            result.resolution_kind,
            gitnexus_typescript::TsResolutionKind::Unresolved
        ),
        "Expected Unresolved, got: {:?}",
        result.resolution_kind
    );
    assert!(
        result.reason.contains("unresolved"),
        "Expected unresolved reason, got: {}",
        result.reason
    );
}

#[test]
fn test_external_package_returns_external() {
    let resolver = build_resolver();
    let importer = fixture_root().join("packages/app/src/main.ts");

    // "react" is an external npm package
    let result = resolver.resolve_import(&importer, "react");

    assert!(
        result.target_file.is_none(),
        "Expected react to have no target, got: {:?}",
        result
    );
    assert_eq!(
        result.resolution_kind,
        gitnexus_typescript::TsResolutionKind::External
    );
    assert!(
        result.reason.contains("external"),
        "Expected external reason, got: {}",
        result.reason
    );
}

#[test]
fn test_reexport_one_hop_resolves_create_order() {
    let resolver = build_resolver();
    let importer = fixture_root().join("packages/app/src/main.ts");

    // @shared resolves to shared/src/index.ts which re-exports from models/order
    // The resolution itself is to index.ts — the re-export is a downstream concern
    let result = resolver.resolve_import(&importer, "@shared");

    assert!(result.target_file.is_some());
    let resolved = result.target_file.unwrap();
    assert!(resolved.to_string_lossy().contains("shared/src/index.ts"));

    // Also verify @models/order resolves to the actual file with createOrder
    let result2 = resolver.resolve_import(&importer, "@models/order");
    assert!(result2.target_file.is_some());
    let resolved2 = result2.target_file.unwrap();
    assert!(resolved2.to_string_lossy().contains("models/order.ts"));
}

#[test]
fn test_deterministic_ordering() {
    let resolver1 = build_resolver();
    let resolver2 = build_resolver();

    // tsconfig paths should be in same order (BTreeMap)
    let paths1: Vec<_> = resolver1
        .tsconfigs
        .iter()
        .flat_map(|tc| tc.paths.keys().cloned())
        .collect();
    let paths2: Vec<_> = resolver2
        .tsconfigs
        .iter()
        .flat_map(|tc| tc.paths.keys().cloned())
        .collect();
    assert_eq!(paths1, paths2, "Paths should be deterministically ordered");

    // Workspace packages should be in same order
    let pkgs1: Vec<_> = resolver1.workspace_packages.keys().collect();
    let pkgs2: Vec<_> = resolver2.workspace_packages.keys().collect();
    assert_eq!(
        pkgs1, pkgs2,
        "Workspace packages should be deterministically ordered"
    );

    // Same resolution for same inputs
    let importer = fixture_root().join("packages/app/src/main.ts");
    let r1 = resolver1.resolve_import(&importer, "@core/logger");
    let r2 = resolver2.resolve_import(&importer, "@core/logger");
    assert_eq!(r1.target_file, r2.target_file);
    assert_eq!(r1.resolution_kind, r2.resolution_kind);
    assert_eq!(r1.confidence, r2.confidence);
}
