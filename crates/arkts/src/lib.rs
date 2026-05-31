//! GitNexus ArkTS language adapter.
//!
//! This crate provides ArkTS-specific semantic analysis on top of the
//! `gitnexus-typescript` shared base. ArkTS is a TypeScript subset used in
//! HarmonyOS development, featuring decorators (`@Component`, `@State`, etc.),
//! `struct`-based component definitions, and declarative UI via `build()`.
//!
//! The tree-sitter grammar reuses `tree-sitter-typescript` since ArkTS is a
//! strict subset with added decorators. The ArkTS semantic layer interprets
//! ERROR nodes produced by `struct`/`decorator` patterns and maps them to
//! component/state/UI concepts.
//!
//! ## Layout
//!
//! - `extractors/` — ArkTS-specific extractors (component, state, UI)
//! - `graph.rs` — ArkTS-aware graph construction (extends TypeScript graph)

pub mod extractors;
pub mod graph;
pub mod project;

// Re-export base types from the TypeScript crate (always available)
pub use gitnexus_typescript::{
    graph::build_ts_graph, is_ts_parser_available, list_source_files, load_ts_manifest, TsImport,
    TsManifest, TsManifestError, TsPackageInfo, TsParseError, TsProject, TsProjectKind,
    TsReference, TsSymbol, TsSymbolKind,
};

// Re-export tree-sitter-dependent items when the feature is enabled
#[cfg(feature = "tree-sitter-arkts")]
pub use gitnexus_typescript::extractors::{
    extract_ts_file_from_root, extract_ts_imports, extract_ts_references, extract_ts_symbols,
    try_init_ts_parser, TsExtraction, TsLanguage,
};

// Re-export ArkTS-specific types
pub use extractors::component::ArkTsComponent;

#[cfg(feature = "tree-sitter-arkts")]
pub use extractors::component::{extract_arkts_components, extract_arkts_components_from_root};

/// Parse-once extraction result for one ArkTS source file.
#[cfg(feature = "tree-sitter-arkts")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArkTsExtraction {
    pub base: TsExtraction,
    pub components: Vec<ArkTsComponent>,
}

/// Extract TypeScript-base data and ArkTS components from a single parse tree.
#[cfg(feature = "tree-sitter-arkts")]
pub fn extract_arkts_file(source: &str) -> ArkTsExtraction {
    let mut parser = match try_init_ts_parser(TsLanguage::TypeScript) {
        Some(p) => p,
        None => {
            return ArkTsExtraction {
                base: TsExtraction {
                    symbols: vec![],
                    imports: vec![],
                    references: vec![],
                },
                components: vec![],
            };
        }
    };
    let tree = match parser.parse(source, None) {
        Some(t) => t,
        None => {
            return ArkTsExtraction {
                base: TsExtraction {
                    symbols: vec![],
                    imports: vec![],
                    references: vec![],
                },
                components: vec![],
            };
        }
    };
    let root = tree.root_node();
    ArkTsExtraction {
        base: extract_ts_file_from_root(&root, source),
        components: extract_arkts_components_from_root(&root, source),
    }
}

#[cfg(all(test, feature = "tree-sitter-arkts"))]
mod tests {
    use super::*;

    #[test]
    fn combined_arkts_extraction_matches_separate_extractors() {
        let source = r#"import { Logger } from "./Logger";

@Entry
@Component
struct Index {
  build() {
    Column() {
      Text(Logger.title())
    }
  }
}
"#;

        let combined = extract_arkts_file(source);
        let lang = TsLanguage::TypeScript;

        assert_eq!(combined.base.symbols, extract_ts_symbols(source, lang));
        assert_eq!(combined.base.imports, extract_ts_imports(source, lang));
        assert_eq!(
            combined.base.references,
            extract_ts_references(source, lang)
        );
        assert_eq!(combined.components, extract_arkts_components(source));
    }
}
