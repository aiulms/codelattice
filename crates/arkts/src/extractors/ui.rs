//! ArkTS declarative UI call extraction.
//!
//! ArkTS uses declarative UI in `build()` methods with chained calls
//! like `Text(this.message).fontSize(30).fontColor(Color.Blue)`.

/// A UI component call extracted from an ArkTS build() method.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArkTsUiCall {
    /// The component name (e.g., "Column", "Text", "Button").
    pub component: String,
    /// 1-based line number.
    pub line: usize,
    /// Chained attribute calls (e.g., ["fontSize", "fontColor"]).
    pub attributes: Vec<String>,
}
