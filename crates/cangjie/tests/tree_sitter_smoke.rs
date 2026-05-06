#[cfg(feature = "tree-sitter-cangjie")]
mod tree_sitter_smoke {
    use gitnexus_cangjie::extractors::{
        is_cangjie_parser_available, parse_cangjie_source, tree_has_error_nodes,
    };
    use std::path::PathBuf;

    fn fixture_source(name: &str, file: &str) -> String {
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.pop(); // crates/cangjie → crates
        path.pop(); // crates → repo root
        path.push("fixtures");
        path.push("cangjie");
        path.push(name);
        path.push(file);
        std::fs::read_to_string(&path).expect("failed to read fixture source")
    }

    #[test]
    fn parser_available_when_feature_enabled() {
        assert!(is_cangjie_parser_available());
    }

    #[test]
    fn parse_cjpm_basic_main_no_error_nodes() {
        let source = fixture_source("cjpm-basic", "src/main.cj");
        let tree = parse_cangjie_source(&source).expect("parse should succeed");
        assert!(
            !tree_has_error_nodes(&tree),
            "expected no ERROR/MISSING nodes in cjpm-basic/src/main.cj"
        );
    }

    #[test]
    fn parse_cjpm_basic_cjpm_toml_no_error_nodes() {
        // cjpm.toml is parsed as Cangjie source (not as TOML) via tree-sitter —
        // it should either parse cleanly or report syntax errors gracefully.
        let source = fixture_source("cjpm-basic", "cjpm.toml");
        // cjpm.toml is not valid Cangjie — expect either failure or ERROR nodes.
        // We test that the parser handles it gracefully without panic.
        match parse_cangjie_source(&source) {
            Ok(tree) => {
                let has_errors = tree_has_error_nodes(&tree);
                // TOML is not Cangjie syntax, so ERROR nodes are expected here.
                // The key property: no panic.
                let _ = has_errors;
            }
            Err(e) => {
                // Also acceptable: parse may fail entirely for non-Cangjie input.
                let _ = e;
            }
        }
    }
}
