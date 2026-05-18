//! Shell graph extraction.

use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::path::{Path, PathBuf};

use crate::project::ShellProject;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ShellNodeKind {
    Repository,
    SourceFile,
    Symbol,
    Command,
    EnvironmentVariable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ShellEdgeKind {
    OwnsSource,
    Defines,
    Calls,
    Sources,
    ReadsEnv,
    WritesEnv,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShellSymbolKind {
    ScriptEntry,
    Function,
}

impl fmt::Display for ShellSymbolKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ShellSymbolKind::ScriptEntry => write!(f, "scriptEntry"),
            ShellSymbolKind::Function => write!(f, "function"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellSymbol {
    pub id: String,
    pub name: String,
    pub kind: ShellSymbolKind,
    pub source_path: String,
    pub line_start: usize,
    pub line_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellCommand {
    pub name: String,
    pub line: usize,
    pub line_text: String,
    pub owner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellSourceRef {
    pub path: String,
    pub line: usize,
    pub owner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellEnvAccess {
    pub name: String,
    pub line: usize,
    pub write: bool,
    pub owner: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    pub source_path: String,
    pub line: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellFileAnalysis {
    pub source_path: String,
    pub line_count: usize,
    pub has_shebang: bool,
    pub symbols: Vec<ShellSymbol>,
    pub commands: Vec<ShellCommand>,
    pub sources: Vec<ShellSourceRef>,
    pub env: Vec<ShellEnvAccess>,
    pub diagnostics: Vec<ShellDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellGraphNode {
    pub id: String,
    pub kind: ShellNodeKind,
    pub label: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub properties: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellGraphEdge {
    #[serde(rename = "type")]
    pub kind: ShellEdgeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellGraphOutput {
    #[serde(rename = "schemaVersion")]
    pub schema_version: String,
    pub nodes: Vec<ShellGraphNode>,
    pub edges: Vec<ShellGraphEdge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<serde_json::Value>,
}

pub fn extract_shell_file(source: &str, rel_path: &str) -> ShellFileAnalysis {
    let lines: Vec<&str> = source.lines().collect();
    let has_shebang = lines.first().map(|l| l.starts_with("#!")).unwrap_or(false);
    let script_entry_id = shell_id(&format!("shell:symbol:{rel_path}:script-entry"));

    let mut symbols = vec![ShellSymbol {
        id: script_entry_id.clone(),
        name: rel_path.to_string(),
        kind: ShellSymbolKind::ScriptEntry,
        source_path: rel_path.to_string(),
        line_start: 1,
        line_end: lines.len().max(1),
    }];

    let mut function_ranges: Vec<(String, usize, usize)> = Vec::new();
    let mut current_fn: Option<(String, usize, i32)> = None;

    for (idx, raw) in lines.iter().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim().to_string();
        if line.is_empty() {
            continue;
        }
        if current_fn.is_none() {
            if let Some(name) = parse_function_name(&line) {
                let balance = brace_delta(&line);
                current_fn = Some((name, line_no, balance.max(1)));
                continue;
            }
        } else if let Some((name, start, depth)) = current_fn.as_mut() {
            *depth += brace_delta(&line);
            if *depth <= 0 || line == "}" {
                let id = shell_id(&format!("shell:symbol:{rel_path}:{name}"));
                symbols.push(ShellSymbol {
                    id,
                    name: name.clone(),
                    kind: ShellSymbolKind::Function,
                    source_path: rel_path.to_string(),
                    line_start: *start,
                    line_end: line_no,
                });
                function_ranges.push((name.clone(), *start, line_no));
                current_fn = None;
            }
        }
    }
    if let Some((name, start, _)) = current_fn.take() {
        let id = shell_id(&format!("shell:symbol:{rel_path}:{name}"));
        symbols.push(ShellSymbol {
            id,
            name: name.clone(),
            kind: ShellSymbolKind::Function,
            source_path: rel_path.to_string(),
            line_start: start,
            line_end: lines.len().max(start),
        });
        function_ranges.push((name, start, lines.len().max(start)));
    }

    let mut commands = Vec::new();
    let mut sources = Vec::new();
    let mut env = Vec::new();
    let mut diagnostics = Vec::new();

    for (idx, raw) in lines.iter().enumerate() {
        let line_no = idx + 1;
        let line = strip_comment(raw).trim().to_string();
        if line.is_empty() || parse_function_name(&line).is_some() || line == "}" {
            continue;
        }
        let owner = owner_for_line(rel_path, line_no, &function_ranges, &script_entry_id);

        for access in extract_env_accesses(&line, line_no, &owner) {
            env.push(access);
        }
        if let Some(path) = parse_source_ref(&line) {
            sources.push(ShellSourceRef {
                path,
                line: line_no,
                owner: owner.clone(),
            });
        }
        for name in command_names(&line) {
            commands.push(ShellCommand {
                name,
                line: line_no,
                line_text: line.clone(),
                owner: owner.clone(),
            });
        }
        diagnostics.extend(risk_diagnostics(&line, rel_path, line_no));
    }

    ShellFileAnalysis {
        source_path: rel_path.to_string(),
        line_count: lines.len(),
        has_shebang,
        symbols,
        commands,
        sources,
        env,
        diagnostics,
    }
}

pub fn build_shell_graph(
    project: &ShellProject,
    analyses_by_file: &BTreeMap<PathBuf, ShellFileAnalysis>,
) -> ShellGraphOutput {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut diagnostics = Vec::new();
    let mut node_ids = BTreeSet::new();

    let repo_id = shell_id(&format!("shell:repo:{}", project.root.to_string_lossy()));
    push_node(
        &mut nodes,
        &mut node_ids,
        ShellGraphNode {
            id: repo_id.clone(),
            kind: ShellNodeKind::Repository,
            label: project
                .root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("shell-project")
                .to_string(),
            properties: json!({"projectKind": format!("{:?}", project.kind)}),
        },
    );

    let mut file_id_map = BTreeMap::new();
    for file in &project.source_files {
        let rel = rel_path(&project.root, file);
        let file_id = shell_id(&format!("shell:file:{rel}"));
        file_id_map.insert(file.clone(), file_id.clone());
        push_node(
            &mut nodes,
            &mut node_ids,
            ShellGraphNode {
                id: file_id.clone(),
                kind: ShellNodeKind::SourceFile,
                label: rel.clone(),
                properties: json!({
                    "sourcePath": rel,
                    "extension": file.extension().and_then(|e| e.to_str()).unwrap_or(""),
                }),
            },
        );
        edges.push(edge(
            ShellEdgeKind::OwnsSource,
            Some(repo_id.clone()),
            file_id,
            None,
        ));
    }

    let mut function_by_name: BTreeMap<String, String> = BTreeMap::new();
    for analysis in analyses_by_file.values() {
        let file_id = shell_id(&format!("shell:file:{}", analysis.source_path));
        for sym in &analysis.symbols {
            push_node(
                &mut nodes,
                &mut node_ids,
                ShellGraphNode {
                    id: sym.id.clone(),
                    kind: ShellNodeKind::Symbol,
                    label: sym.name.clone(),
                    properties: json!({
                        "symbolKind": sym.kind.to_string(),
                        "name": sym.name,
                        "sourcePath": sym.source_path,
                        "fileId": file_id,
                        "lineStart": sym.line_start,
                        "lineEnd": sym.line_end,
                        "visibility": "script",
                    }),
                },
            );
            edges.push(edge(
                ShellEdgeKind::Defines,
                Some(file_id.clone()),
                sym.id.clone(),
                None,
            ));
            if sym.kind == ShellSymbolKind::Function {
                function_by_name
                    .entry(sym.name.clone())
                    .or_insert_with(|| sym.id.clone());
            }
        }
    }

    let mut command_nodes = BTreeSet::new();
    let mut env_nodes = BTreeSet::new();
    for analysis in analyses_by_file.values() {
        for command in &analysis.commands {
            if let Some(target) = function_by_name.get(&command.name) {
                edges.push(edge(
                    ShellEdgeKind::Calls,
                    Some(command.owner.clone()),
                    target.clone(),
                    Some(json!({
                        "confidence": 0.8,
                        "reason": "shell-function-name-match",
                        "line": command.line,
                    })),
                ));
            } else {
                let cmd_id = shell_id(&format!("shell:command:{}", command.name));
                if command_nodes.insert(cmd_id.clone()) {
                    push_node(
                        &mut nodes,
                        &mut node_ids,
                        ShellGraphNode {
                            id: cmd_id.clone(),
                            kind: ShellNodeKind::Command,
                            label: command.name.clone(),
                            properties: json!({"command": command.name}),
                        },
                    );
                }
                edges.push(edge(
                    ShellEdgeKind::Calls,
                    Some(command.owner.clone()),
                    cmd_id,
                    Some(json!({
                        "confidence": 0.55,
                        "reason": "external-command-invocation",
                        "line": command.line,
                    })),
                ));
            }
        }

        for source_ref in &analysis.sources {
            if let Some(target_file) =
                resolve_source_path(&project.root, &analysis.source_path, &source_ref.path)
            {
                if let Some(target_id) = file_id_map.get(&target_file) {
                    edges.push(edge(
                        ShellEdgeKind::Sources,
                        Some(source_ref.owner.clone()),
                        target_id.clone(),
                        Some(json!({
                            "confidence": 0.85,
                            "reason": "literal-source-path",
                            "line": source_ref.line,
                            "path": source_ref.path,
                        })),
                    ));
                    continue;
                }
            }
            diagnostics.push(json!({
                "code": "unresolved_source",
                "severity": "medium",
                "message": format!("Could not resolve sourced script {}", source_ref.path),
                "sourcePath": analysis.source_path,
                "line": source_ref.line,
            }));
        }

        for access in &analysis.env {
            let env_id = shell_id(&format!("shell:env:{}", access.name));
            if env_nodes.insert(env_id.clone()) {
                push_node(
                    &mut nodes,
                    &mut node_ids,
                    ShellGraphNode {
                        id: env_id.clone(),
                        kind: ShellNodeKind::EnvironmentVariable,
                        label: access.name.clone(),
                        properties: json!({"name": access.name}),
                    },
                );
            }
            edges.push(edge(
                if access.write {
                    ShellEdgeKind::WritesEnv
                } else {
                    ShellEdgeKind::ReadsEnv
                },
                Some(access.owner.clone()),
                env_id,
                Some(json!({"line": access.line})),
            ));
        }
        for d in &analysis.diagnostics {
            diagnostics.push(serde_json::to_value(d).unwrap_or_else(|_| json!({})));
        }
    }

    ShellGraphOutput {
        schema_version: "shell-v0.1".to_string(),
        nodes,
        edges,
        diagnostics,
    }
}

fn push_node(nodes: &mut Vec<ShellGraphNode>, ids: &mut BTreeSet<String>, node: ShellGraphNode) {
    if ids.insert(node.id.clone()) {
        nodes.push(node);
    }
}

fn edge(
    kind: ShellEdgeKind,
    source: Option<String>,
    target: String,
    properties: Option<serde_json::Value>,
) -> ShellGraphEdge {
    ShellGraphEdge {
        kind,
        source,
        target,
        properties,
    }
}

fn rel_path(root: &Path, file: &Path) -> String {
    file.strip_prefix(root)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/")
}

fn shell_id(raw: &str) -> String {
    raw.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, ':' | '_' | '-' | '.' | '/') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn strip_comment(line: &str) -> String {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;
    let mut out = String::new();
    for ch in line.chars() {
        if escaped {
            out.push(ch);
            escaped = false;
            continue;
        }
        if ch == '\\' {
            out.push(ch);
            escaped = true;
            continue;
        }
        if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if ch == '"' && !in_single {
            in_double = !in_double;
        } else if ch == '#' && !in_single && !in_double {
            break;
        }
        out.push(ch);
    }
    out
}

fn parse_function_name(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix("function ") {
        let name = rest
            .split(|c: char| c.is_whitespace() || c == '{' || c == '(')
            .next()
            .unwrap_or("");
        return valid_name(name).then(|| name.to_string());
    }
    if let Some(pos) = trimmed.find("()") {
        let name = trimmed[..pos].trim();
        return valid_name(name).then(|| name.to_string());
    }
    None
}

fn valid_name(name: &str) -> bool {
    let mut chars = name.chars();
    match chars.next() {
        Some(ch) if ch.is_ascii_alphabetic() || ch == '_' => {}
        _ => return false,
    }
    chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_')
}

fn brace_delta(line: &str) -> i32 {
    line.chars().filter(|&c| c == '{').count() as i32
        - line.chars().filter(|&c| c == '}').count() as i32
}

fn owner_for_line(
    rel_path: &str,
    line: usize,
    ranges: &[(String, usize, usize)],
    script_entry_id: &str,
) -> String {
    for (name, start, end) in ranges {
        if line >= *start && line <= *end {
            return shell_id(&format!("shell:symbol:{rel_path}:{name}"));
        }
    }
    script_entry_id.to_string()
}

fn extract_env_accesses(line: &str, line_no: usize, owner: &str) -> Vec<ShellEnvAccess> {
    let mut out = Vec::new();
    let mut words = line.split_whitespace().peekable();
    if words.peek() == Some(&"export") {
        words.next();
    }
    for word in words.clone() {
        if let Some((name, _)) = word.split_once('=') {
            if valid_env_name(name) {
                out.push(ShellEnvAccess {
                    name: name.to_string(),
                    line: line_no,
                    write: true,
                    owner: owner.to_string(),
                });
            }
        } else {
            break;
        }
    }
    let bytes = line.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'$' {
            let (name, next) = if i + 1 < bytes.len() && bytes[i + 1] == b'{' {
                let mut j = i + 2;
                while j < bytes.len() && bytes[j] != b'}' {
                    j += 1;
                }
                (line[i + 2..j.min(bytes.len())].to_string(), j + 1)
            } else {
                let mut j = i + 1;
                while j < bytes.len()
                    && ((bytes[j] as char).is_ascii_alphanumeric() || bytes[j] == b'_')
                {
                    j += 1;
                }
                (line[i + 1..j].to_string(), j)
            };
            if valid_env_name(&name) {
                out.push(ShellEnvAccess {
                    name,
                    line: line_no,
                    write: false,
                    owner: owner.to_string(),
                });
            }
            i = next;
        } else {
            i += 1;
        }
    }
    out
}

fn valid_env_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .chars()
            .all(|ch| ch.is_ascii_uppercase() || ch.is_ascii_digit() || ch == '_')
}

fn parse_source_ref(line: &str) -> Option<String> {
    let mut parts = line.split_whitespace();
    let first = parts.next()?;
    if first != "source" && first != "." {
        return None;
    }
    parts.next().map(unquote)
}

fn command_names(line: &str) -> Vec<String> {
    let mut out = Vec::new();
    for segment in line
        .split('|')
        .flat_map(|part| part.split("&&"))
        .flat_map(|part| part.split("||"))
        .flat_map(|part| part.split(';'))
    {
        let segment = segment.trim();
        if segment.is_empty() {
            continue;
        }
        let mut words = segment.split_whitespace().peekable();
        while let Some(word) = words.peek().copied() {
            if word == "export" {
                words.next();
                continue;
            }
            if word.contains('=') && !word.starts_with('$') {
                words.next();
                continue;
            }
            break;
        }
        let Some(first) = words.next() else {
            continue;
        };
        let name = unquote(first);
        if is_keyword(&name) || name.starts_with('$') || name == "source" || name == "." {
            continue;
        }
        out.push(name);
    }
    out
}

fn is_keyword(name: &str) -> bool {
    matches!(
        name,
        "if" | "then"
            | "else"
            | "elif"
            | "fi"
            | "for"
            | "do"
            | "done"
            | "case"
            | "esac"
            | "while"
            | "until"
            | "select"
            | "function"
            | "in"
            | "{"
            | "}"
            | "["
            | "[["
            | "set"
    )
}

fn unquote(s: &str) -> String {
    s.trim_matches('"')
        .trim_matches('\'')
        .trim_end_matches(',')
        .to_string()
}

fn risk_diagnostics(line: &str, rel_path: &str, line_no: usize) -> Vec<ShellDiagnostic> {
    let lower = line.to_ascii_lowercase();
    let mut out = Vec::new();
    if lower.contains("curl")
        && lower.contains('|')
        && (lower.contains(" sh") || lower.contains(" bash"))
    {
        out.push(diag(
            "curl_pipe_shell",
            "high",
            "curl output is piped into a shell; verify source and checksum before running",
            rel_path,
            line_no,
        ));
    }
    if lower.contains("rm -rf") {
        out.push(diag(
            "rm_rf",
            "medium",
            "recursive deletion detected; verify variable guards and target path",
            rel_path,
            line_no,
        ));
    }
    if lower.contains("chmod 777") {
        out.push(diag(
            "chmod_777",
            "medium",
            "world-writable permissions detected",
            rel_path,
            line_no,
        ));
    }
    if lower.split_whitespace().any(|w| w == "sudo") {
        out.push(diag(
            "sudo_command",
            "medium",
            "sudo command detected; static analysis cannot verify privilege boundary",
            rel_path,
            line_no,
        ));
    }
    out
}

fn diag(
    code: &str,
    severity: &str,
    message: &str,
    source_path: &str,
    line: usize,
) -> ShellDiagnostic {
    ShellDiagnostic {
        code: code.to_string(),
        severity: severity.to_string(),
        message: message.to_string(),
        source_path: source_path.to_string(),
        line,
    }
}

fn resolve_source_path(root: &Path, source_rel: &str, sourced: &str) -> Option<PathBuf> {
    let cleaned = sourced
        .replace("${ROOT_DIR}", ".")
        .replace("$ROOT_DIR", ".")
        .replace("${SCRIPT_DIR}", ".")
        .replace("$SCRIPT_DIR", ".");
    let base = root.join(source_rel).parent()?.to_path_buf();
    let candidate = if cleaned.starts_with('/') {
        PathBuf::from(cleaned)
    } else {
        base.join(cleaned)
    };
    candidate.canonicalize().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::project::{find_shell_project_root, list_shell_source_files};

    #[test]
    fn extracts_functions_commands_sources_and_risks() {
        let source = r#"#!/usr/bin/env bash
ROOT_DIR="$(pwd)"
source "$ROOT_DIR/scripts/common.sh"
build_project() {
  log_info "build"
  cargo build
  rm -rf "$DIST_DIR/tmp"
}
"#;
        let analysis = extract_shell_file(source, "build.sh");
        assert!(analysis.has_shebang);
        assert!(analysis.symbols.iter().any(|s| s.name == "build_project"));
        assert!(analysis.commands.iter().any(|c| c.name == "cargo"));
        assert!(analysis
            .sources
            .iter()
            .any(|s| s.path.contains("common.sh")));
        assert!(analysis.diagnostics.iter().any(|d| d.code == "rm_rf"));
    }

    #[test]
    fn builds_portable_smoke_graph_without_dangling_edges() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures")
            .join("shell")
            .join("portable-smoke");
        let project = find_shell_project_root(&root).expect("shell project");
        let files = list_shell_source_files(&project).expect("files");
        let mut analyses = BTreeMap::new();
        for file in files {
            let rel = file
                .strip_prefix(&project.root)
                .unwrap()
                .to_string_lossy()
                .to_string();
            let source = std::fs::read_to_string(&file).unwrap();
            analyses.insert(file, extract_shell_file(&source, &rel));
        }
        let graph = build_shell_graph(&project, &analyses);
        assert!(graph.nodes.len() >= 10);
        assert!(graph.edges.len() >= 10);
        let node_ids: BTreeSet<_> = graph.nodes.iter().map(|n| n.id.as_str()).collect();
        for edge in graph.edges {
            if let Some(source) = edge.source.as_deref() {
                assert!(node_ids.contains(source), "dangling source {source}");
            }
            assert!(
                node_ids.contains(edge.target.as_str()),
                "dangling target {}",
                edge.target
            );
        }
    }
}
