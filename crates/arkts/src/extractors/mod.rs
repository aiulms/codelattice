pub mod component;
pub mod state;
pub mod ui;

pub use component::ArkTsComponent;
pub use state::ArkTsStateProperty;
pub use ui::ArkTsUiCall;

/// Check whether the ArkTS parser is available at runtime.
pub fn is_arkts_parser_available() -> bool {
    cfg!(feature = "tree-sitter-arkts")
}
