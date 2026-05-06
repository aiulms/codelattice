//! Cangjie diagnostic types — language-agnostic representation of compiler
//! and linter diagnostics.
//!
//! Aligns with GitNexus-RC TypeScript `NormalizedDiagnostic` interface.

use serde::Serialize;

/// Severity of a diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
    Note,
    Suggestion,
}

/// A normalized diagnostic from cjc compiler or cjlint linter.
///
/// Line and column numbers are 0-based (normalized from Cangjie SDK 1-based
/// output).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CangjieDiagnostic {
    /// File path relative to project root.
    pub file_path: String,
    /// 0-based start line.
    pub start_line: usize,
    /// 0-based start column.
    pub start_column: usize,
    /// 0-based end line.
    pub end_line: usize,
    /// 0-based end column.
    pub end_column: usize,
    /// Diagnostic severity.
    pub severity: DiagnosticSeverity,
    /// Human-readable diagnostic message.
    pub message: String,
    /// Source tool: "cjc" or "cjlint".
    pub source: String,
    /// Rule / diagnostic kind identifier (e.g. cjlint defect type or cjc DiagKind).
    pub rule: Option<String>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn diagnostic_serializes_to_json() {
        let d = CangjieDiagnostic {
            file_path: "src/main.cj".to_string(),
            start_line: 0,
            start_column: 5,
            end_line: 0,
            end_column: 10,
            severity: DiagnosticSeverity::Error,
            message: "type mismatch".to_string(),
            source: "cjc".to_string(),
            rule: Some("E001".to_string()),
        };
        let json = serde_json::to_string(&d).unwrap();
        assert!(json.contains("\"filePath\""));
        assert!(json.contains("\"startLine\":0"));
        assert!(json.contains("\"startColumn\":5"));
        assert!(json.contains("\"severity\":\"error\""));
        assert!(json.contains("\"source\":\"cjc\""));
        assert!(json.contains("\"rule\":\"E001\""));
    }

    #[test]
    fn severity_serializes_camel_case() {
        assert_eq!(
            serde_json::to_string(&DiagnosticSeverity::Error).unwrap(),
            "\"error\""
        );
        assert_eq!(
            serde_json::to_string(&DiagnosticSeverity::Warning).unwrap(),
            "\"warning\""
        );
        assert_eq!(
            serde_json::to_string(&DiagnosticSeverity::Note).unwrap(),
            "\"note\""
        );
        assert_eq!(
            serde_json::to_string(&DiagnosticSeverity::Suggestion).unwrap(),
            "\"suggestion\""
        );
    }
}
