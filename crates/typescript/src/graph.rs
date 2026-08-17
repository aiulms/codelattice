//! Graph output for TypeScript project model and symbol extraction.
//!
//! Produces a language-agnostic graph structure (nodes + edges) compatible
//! with the project-model `GraphOutput` JSON schema.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::extractors::imports::TsImport;
use crate::extractors::references::{TsReference, TsReferenceKind};
use crate::extractors::symbol::{TsSymbol, TsSymbolKind};
use crate::module_resolution::TsModuleResolver;
use crate::project::TsProject;

// ---------------------------------------------------------------------------
// Node types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TsNodeKind {
    Repository,
    Package,
    SourceFile,
    Symbol,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsGraphNode {
    pub id: String,
    pub kind: TsNodeKind,
    pub label: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub properties: serde_json::Value,
}

// ---------------------------------------------------------------------------
// Edge types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TsEdgeKind {
    ContainsPackage,
    OwnsSource,
    Defines,
    Imports,
    Calls,
    TypeUse,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsGraphEdge {
    #[serde(rename = "type")]
    pub kind: TsEdgeKind,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub properties: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Graph output
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TsGraphOutput {
    pub nodes: Vec<TsGraphNode>,
    pub edges: Vec<TsGraphEdge>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub diagnostics: Vec<serde_json::Value>,
}

/// Build a complete graph from a TsProject and extracted per-file data.
pub fn build_ts_graph(
    project: &TsProject,
    symbols: &BTreeMap<PathBuf, Vec<TsSymbol>>,
    imports: &BTreeMap<PathBuf, Vec<TsImport>>,
    references: &BTreeMap<PathBuf, Vec<TsReference>>,
    module_resolver: Option<&TsModuleResolver>,
) -> TsGraphOutput {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut diagnostics = Vec::new();

    // Repository node
    let repo_id = format!("repo:{}", project.root.display());
    nodes.push(TsGraphNode {
        id: repo_id.clone(),
        kind: TsNodeKind::Repository,
        label: "repository".to_string(),
        properties: serde_json::json!({
            "name": project
                .root
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("root")
                .to_string(),
            "language": format!("{:?}", project.kind),
        }),
    });

    // Package node (if manifest available)
    let pkg_id = if let Some(ref manifest) = project.manifest {
        let pkg_id = format!("pkg:{}", manifest.name);
        let manifest_path = match manifest.kind {
            crate::manifest::TsManifestKind::OhPackageJson5 => project
                .root
                .join("oh-package.json5")
                .to_string_lossy()
                .to_string(),
            crate::manifest::TsManifestKind::PackageJson => project
                .root
                .join("package.json")
                .to_string_lossy()
                .to_string(),
            crate::manifest::TsManifestKind::TsconfigJson => project
                .root
                .join("tsconfig.json")
                .to_string_lossy()
                .to_string(),
        };
        nodes.push(TsGraphNode {
            id: pkg_id.clone(),
            kind: TsNodeKind::Package,
            label: "package".to_string(),
            properties: serde_json::json!({
                "name": manifest.name.clone(),
                "manifestPath": manifest_path,
            }),
        });
        edges.push(TsGraphEdge {
            kind: TsEdgeKind::ContainsPackage,
            source: Some(repo_id.clone()),
            target: pkg_id.clone(),
            properties: None,
        });
        Some(pkg_id)
    } else {
        None
    };

    // Collect all file node IDs for dangling edge prevention
    let mut file_ids: std::collections::BTreeSet<String> = std::collections::BTreeSet::new();
    // Also build a map from canonical absolute path → file_id for resolver matching.
    // The resolver produces absolute paths, but file_ids may be relative (when project.root is relative).
    let mut canonical_to_file_id: std::collections::BTreeMap<PathBuf, String> =
        std::collections::BTreeMap::new();
    let mut canonical_to_source_file: std::collections::BTreeMap<PathBuf, PathBuf> =
        std::collections::BTreeMap::new();
    let mut symbol_ids_by_name: BTreeMap<String, Vec<TsSymbolCandidate>> = BTreeMap::new();
    let mut symbol_ids_by_file_name: BTreeMap<(PathBuf, String), Vec<TsSymbolCandidate>> =
        BTreeMap::new();
    let mut symbol_ranges_by_file: BTreeMap<PathBuf, Vec<(usize, usize, String)>> = BTreeMap::new();
    // CALLS 按 (caller, callee) 聚合后出边，调用点行号聚合进 lines/callCount。
    let mut resolved_calls: BTreeMap<(String, String), TsCallEdgeAgg> = BTreeMap::new();

    // Source file nodes
    for file in &project.source_files {
        let file_id = format!("file:{}", file.display());
        let rel = file.strip_prefix(&project.root).unwrap_or(file);
        file_ids.insert(file_id.clone());
        // Try to canonicalize for resolver matching
        if let Ok(canonical) = std::fs::canonicalize(file) {
            canonical_to_file_id.insert(canonical.clone(), file_id.clone());
            canonical_to_source_file.insert(canonical, file.clone());
        } else {
            // Fallback: store as-is (absolute path)
            canonical_to_file_id.insert(file.clone(), file_id.clone());
            canonical_to_source_file.insert(file.clone(), file.clone());
        }
        nodes.push(TsGraphNode {
            id: file_id.clone(),
            kind: TsNodeKind::SourceFile,
            label: "source-file".to_string(),
            properties: serde_json::json!({
                "sourcePath": rel.to_string_lossy().to_string(),
                "packageId": pkg_id,
            }),
        });
        edges.push(TsGraphEdge {
            kind: TsEdgeKind::OwnsSource,
            source: Some(repo_id.clone()),
            target: file_id.clone(),
            properties: None,
        });

        // Symbol nodes for this file
        if let Some(syms) = symbols.get(file) {
            for sym in syms {
                let sym_id = format!(
                    "sym:{}:{}:{}:{}",
                    rel.display(),
                    sym.kind,
                    sym.name,
                    sym.start_line
                );
                nodes.push(TsGraphNode {
                    id: sym_id.clone(),
                    kind: TsNodeKind::Symbol,
                    label: "symbol".to_string(),
                    properties: serde_json::json!({
                        "name": sym.name,
                        "symbolKind": sym.kind.to_string(),
                        "sourcePath": rel.display().to_string(),
                        "fileId": file_id,
                        "lineStart": sym.start_line,
                        "lineEnd": sym.end_line,
                        "ownerName": sym.owner_name,
                    }),
                });
                edges.push(TsGraphEdge {
                    kind: TsEdgeKind::Defines,
                    source: Some(file_id.clone()),
                    target: sym_id.clone(),
                    properties: None,
                });
                let candidate = TsSymbolCandidate {
                    id: sym_id.clone(),
                    kind: sym.kind,
                };
                symbol_ids_by_name
                    .entry(sym.name.clone())
                    .or_default()
                    .push(candidate.clone());
                symbol_ids_by_file_name
                    .entry((file.clone(), sym.name.clone()))
                    .or_default()
                    .push(candidate);
                symbol_ranges_by_file
                    .entry(file.clone())
                    .or_default()
                    .push((sym.start_line, sym.end_line, sym_id.clone()));
            }
        }
    }

    // Build import alias map (source file + local name -> resolved target file) for call resolution.
    let mut import_target_by_file_name: BTreeMap<(PathBuf, String), PathBuf> = BTreeMap::new();

    // IMPORTS 按 (file, target) 聚合后出边：同文件多条 import 语句指向同一模块时
    // 合并 names/lines，避免重复三元组触发 duplicate_edges 门。
    let mut import_edge_agg: BTreeMap<(String, String), TsImportAgg> = BTreeMap::new();

    // Import edges — use resolver if available
    for file in &project.source_files {
        let file_id = format!("file:{}", file.display());
        if let Some(imps) = imports.get(file) {
            for imp in imps {
                if let Some(resolver) = module_resolver {
                    let resolved = resolver.resolve_import(file, &imp.module_path);
                    match resolved.resolution_kind {
                        crate::module_resolution::TsResolutionKind::External => {
                            // No edge — diagnostic only
                            diagnostics.push(serde_json::json!({
                                "kind": "typescript-external-package-not-indexed",
                                "source": file_id,
                                "specifier": imp.module_path,
                                "line": imp.line,
                                "reason": resolved.reason,
                            }));
                        }
                        crate::module_resolution::TsResolutionKind::Unresolved => {
                            // No edge — diagnostic only
                            diagnostics.push(serde_json::json!({
                                "kind": "typescript-import-unresolved",
                                "source": file_id,
                                "specifier": imp.module_path,
                                "line": imp.line,
                                "reason": resolved.reason,
                            }));
                        }
                        _ => {
                            if let Some(ref target_file) = resolved.target_file {
                                // Resolve the target file to the correct file_id via canonical path
                                let canonical_target = std::fs::canonicalize(target_file).ok();
                                let target_id = {
                                    // Try canonical match first
                                    if let Some(canonical) = canonical_target.as_ref() {
                                        canonical_to_file_id.get(canonical).cloned()
                                    } else {
                                        None
                                    }
                                }
                                .unwrap_or_else(|| format!("file:{}", target_file.display()));
                                let target_source_file = canonical_target
                                    .as_ref()
                                    .and_then(|canonical| canonical_to_source_file.get(canonical))
                                    .cloned()
                                    .unwrap_or_else(|| target_file.clone());

                                // Only create edge if target is an existing node
                                if file_ids.contains(&target_id) {
                                    let agg = import_edge_agg
                                        .entry((file_id.clone(), target_id))
                                        .or_insert(TsImportAgg {
                                            names: std::collections::BTreeSet::new(),
                                            lines: vec![],
                                            confidence: resolved.confidence,
                                            reason: Some(resolved.reason.to_string()),
                                        });
                                    agg.names.extend(imp.imported_names.iter().cloned());
                                    agg.lines.push(imp.line);

                                    // Track aliases for call resolution
                                    for name in &imp.imported_names {
                                        import_target_by_file_name.insert(
                                            (file.clone(), name.clone()),
                                            target_source_file.clone(),
                                        );
                                    }
                                    if let Some(alias) = &imp.namespace_alias {
                                        import_target_by_file_name.insert(
                                            (file.clone(), alias.clone()),
                                            target_source_file.clone(),
                                        );
                                    }
                                } else {
                                    // Resolved but not a known source file — diagnostic
                                    diagnostics.push(serde_json::json!({
                                        "kind": "typescript-import-unresolved",
                                        "source": file_id,
                                        "specifier": imp.module_path,
                                        "line": imp.line,
                                        "reason": "resolved-target-not-in-graph",
                                    }));
                                }
                            }
                        }
                    }
                } else {
                    // 无 resolver 时（如 ArkTS CLI 路径）不再产出 module:<specifier>
                    // 合成目标 —— 该目标从无对应节点，必然形成 dangling edge。
                    // 与 resolver 路径的 External/Unresolved 处理一致：不出边，记诊断。
                    diagnostics.push(serde_json::json!({
                        "kind": "typescript-import-unresolved",
                        "severity": "info",
                        "source": file_id,
                        "specifier": imp.module_path,
                        "line": imp.line,
                        "reason": "no-module-resolver",
                    }));
                }
            }
        }
    }

    // 发出聚合后的 IMPORTS 边（file → file，端点均为真实节点）
    for ((source_id, target_id), agg) in import_edge_agg {
        let mut props = serde_json::json!({
            "names": agg.names.into_iter().collect::<Vec<_>>(),
            "line": agg.lines.first().copied().unwrap_or(0),
            "lines": agg.lines,
        });
        if let Some(confidence) = agg.confidence {
            props["confidence"] = serde_json::json!(confidence);
        }
        if let Some(reason) = agg.reason {
            props["reason"] = serde_json::json!(reason);
        }
        edges.push(TsGraphEdge {
            kind: TsEdgeKind::Imports,
            source: Some(source_id),
            target: target_id,
            properties: Some(props),
        });
    }

    // Reference edges
    // TYPE_USE 先聚合后出边：同一 (file, symbol) 只出一条边并计入 useCount；
    // 未解析的按 (file, name) 去重记诊断，避免大文件刷屏。
    let mut resolved_type_uses: BTreeMap<(String, String), TsTypeUseAgg> = BTreeMap::new();
    let mut unresolved_type_uses: BTreeMap<(PathBuf, String), (usize, usize)> = BTreeMap::new();
    for file in &project.source_files {
        let file_id = format!("file:{}", file.display());
        if let Some(refs) = references.get(file) {
            for rf in refs {
                match rf.kind {
                    TsReferenceKind::Call | TsReferenceKind::NewExpression => {
                        let Some(source_id) =
                            source_symbol_for_call(file, rf.line, &symbol_ranges_by_file)
                        else {
                            continue;
                        };
                        match resolve_call_target(
                            file,
                            rf,
                            &import_target_by_file_name,
                            &symbol_ids_by_file_name,
                            &symbol_ids_by_name,
                        ) {
                            TsCallTargetResolution::Resolved {
                                target_id,
                                confidence,
                                reason,
                            } => {
                                // 同一 (caller, callee) 的多个调用点聚合为一条边；
                                // 首次解析结果的 confidence/reason 代表该调用对。
                                let agg = resolved_calls.entry((source_id, target_id)).or_insert(
                                    TsCallEdgeAgg {
                                        first_line: rf.line,
                                        callee: rf.name.clone(),
                                        first_full_text: rf.full_text.clone(),
                                        confidence,
                                        reason,
                                        call_count: 0,
                                        lines: vec![],
                                    },
                                );
                                agg.call_count += 1;
                                agg.lines.push(rf.line);
                            }
                            TsCallTargetResolution::Ambiguous { candidates } => {
                                diagnostics.push(serde_json::json!({
                                    "kind": "typescript-call-ambiguous",
                                    "severity": "info",
                                    "source": file_id,
                                    "callee": rf.name,
                                    "line": rf.line,
                                    "candidateCount": candidates,
                                    "reason": "multiple-symbols-match-callee",
                                }));
                            }
                            TsCallTargetResolution::Unresolved => {}
                        }
                    }
                    TsReferenceKind::TypeUse => {
                        // 类型引用必须解析到真实符号节点；旧实现的 ref:TypeUse:<name>
                        // 合成 target 从无对应节点，是 dangling edge 的根因。
                        // 解析失败不出边，循环结束后统一记诊断。
                        match resolve_type_use_target(
                            file,
                            &rf.name,
                            &import_target_by_file_name,
                            &symbol_ids_by_file_name,
                        ) {
                            Some((target_id, confidence, reason)) => {
                                let key = (file_id.clone(), target_id);
                                let agg = resolved_type_uses.entry(key).or_insert(TsTypeUseAgg {
                                    first_line: rf.line,
                                    first_full_text: rf.full_text.clone(),
                                    confidence,
                                    reason,
                                    use_count: 0,
                                });
                                agg.use_count += 1;
                            }
                            None => {
                                let (_, count) = unresolved_type_uses
                                    .entry((file.clone(), rf.name.clone()))
                                    .or_insert((rf.line, 0usize));
                                *count += 1;
                            }
                        }
                    }
                    TsReferenceKind::MemberAccess => {}
                }
            }
        }
    }

    // 发出聚合后的 CALLS 边（caller → callee，三元组唯一，调用点聚合在 lines/callCount）
    for ((source_id, target_id), agg) in resolved_calls {
        edges.push(TsGraphEdge {
            kind: TsEdgeKind::Calls,
            source: Some(source_id),
            target: target_id,
            properties: Some(serde_json::json!({
                "line": agg.first_line,
                "lines": agg.lines,
                "callCount": agg.call_count,
                "callee": agg.callee,
                "fullText": agg.first_full_text,
                "confidence": agg.confidence,
                "reason": agg.reason,
            })),
        });
    }

    // 发出解析成功的 TYPE_USE 边（file → symbol，端点均有真实节点）
    for ((source_id, target_id), agg) in resolved_type_uses {
        edges.push(TsGraphEdge {
            kind: TsEdgeKind::TypeUse,
            source: Some(source_id),
            target: target_id,
            properties: Some(serde_json::json!({
                "line": agg.first_line,
                "fullText": agg.first_full_text,
                "useCount": agg.use_count,
                "confidence": agg.confidence,
                "reason": agg.reason,
            })),
        });
    }
    // 未解析的类型引用不出边（no-edge policy），仅记诊断
    for ((file, name), (line, count)) in unresolved_type_uses {
        diagnostics.push(serde_json::json!({
            "kind": "typescript-type-use-unresolved",
            "severity": "info",
            "source": format!("file:{}", file.display()),
            "name": name,
            "line": line,
            "useCount": count,
            "reason": "no-import-binding-or-type-symbol",
        }));
    }

    TsGraphOutput {
        nodes,
        edges,
        diagnostics,
    }
}

#[derive(Clone)]
struct TsSymbolCandidate {
    id: String,
    kind: TsSymbolKind,
}

enum TsCallTargetResolution {
    Resolved {
        target_id: String,
        confidence: f64,
        reason: &'static str,
    },
    Ambiguous {
        candidates: usize,
    },
    Unresolved,
}

/// 聚合同一 file → symbol 的多条 TYPE_USE 引用：保留首次出现位置与总次数。
struct TsTypeUseAgg {
    first_line: usize,
    first_full_text: Option<String>,
    confidence: f64,
    reason: &'static str,
    use_count: usize,
}

/// 聚合同一 caller → callee 的多个调用点：schema 要求 (source, type, target)
/// 三元组唯一，调用点行号聚合进 lines/callCount。
struct TsCallEdgeAgg {
    first_line: usize,
    callee: String,
    first_full_text: Option<String>,
    confidence: f64,
    reason: &'static str,
    call_count: usize,
    lines: Vec<usize>,
}

/// 聚合同一 file → target 的多条 import 语句：合并 names 与 lines，
/// 保证 IMPORTS 三元组唯一。
struct TsImportAgg {
    names: std::collections::BTreeSet<String>,
    lines: Vec<usize>,
    confidence: Option<f64>,
    reason: Option<String>,
}

/// 类型类符号白名单：TypeUse 只允许解析到类型声明，不指向普通值符号。
fn is_type_like_kind(kind: TsSymbolKind) -> bool {
    matches!(
        kind,
        TsSymbolKind::Class
            | TsSymbolKind::Interface
            | TsSymbolKind::Enum
            | TsSymbolKind::TypeAlias
            | TsSymbolKind::Namespace
            | TsSymbolKind::Component
    )
}

/// 解析 TypeUse 引用目标（两级名称匹配，不做完整类型推断）：
/// 1) 显式 import 绑定 → 导出文件内同名类型符号；
/// 2) 同文件同名类型符号。
/// 命中零或多个候选都不出边（歧义与 CALLS 的 Ambiguous 处理一致，
/// 符合 no-edge policy：宁可少边，不可 dangling 边）。
fn resolve_type_use_target(
    file: &PathBuf,
    name: &str,
    import_target_by_file_name: &BTreeMap<(PathBuf, String), PathBuf>,
    symbol_ids_by_file_name: &BTreeMap<(PathBuf, String), Vec<TsSymbolCandidate>>,
) -> Option<(String, f64, &'static str)> {
    // 1) import 绑定：import 语句已明确来源文件，目标唯一类型符号置信度高
    if let Some(target_file) = import_target_by_file_name.get(&(file.clone(), name.to_string())) {
        let candidates = symbol_ids_by_file_name
            .get(&(target_file.clone(), name.to_string()))
            .map(|c| {
                c.iter()
                    .filter(|c| is_type_like_kind(c.kind))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        if candidates.len() == 1 {
            return Some((candidates[0].id.clone(), 0.9, "imported-type"));
        }
        return None;
    }
    // 2) 同文件定义即使用，语法上确定性最高
    let candidates = symbol_ids_by_file_name
        .get(&(file.clone(), name.to_string()))
        .map(|c| {
            c.iter()
                .filter(|c| is_type_like_kind(c.kind))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    if candidates.len() == 1 {
        return Some((candidates[0].id.clone(), 0.95, "same-file-type"));
    }
    None
}

fn source_symbol_for_call(
    file: &PathBuf,
    line: usize,
    symbol_ranges_by_file: &BTreeMap<PathBuf, Vec<(usize, usize, String)>>,
) -> Option<String> {
    symbol_ranges_by_file.get(file).and_then(|ranges| {
        ranges
            .iter()
            .filter(|(start, end, _)| *start <= line && line <= *end)
            .min_by_key(|(start, end, _)| end.saturating_sub(*start))
            .map(|(_, _, id)| id.clone())
    })
}

fn resolve_call_target(
    file: &PathBuf,
    reference: &TsReference,
    import_target_by_file_name: &BTreeMap<(PathBuf, String), PathBuf>,
    symbol_ids_by_file_name: &BTreeMap<(PathBuf, String), Vec<TsSymbolCandidate>>,
    symbol_ids_by_name: &BTreeMap<String, Vec<TsSymbolCandidate>>,
) -> TsCallTargetResolution {
    let candidate_names = candidate_call_names(&reference.name);

    for (import_name, callee_name) in imported_call_lookup_keys(reference) {
        if let Some(target_file) = import_target_by_file_name.get(&(file.clone(), import_name)) {
            if let Some(resolved) = resolve_candidates(
                symbol_ids_by_file_name.get(&(target_file.clone(), callee_name)),
                reference,
                0.88,
                "imported-callee-name",
            ) {
                return resolved;
            }
        }
    }

    for name in &candidate_names {
        if let Some(resolved) = resolve_candidates(
            symbol_ids_by_file_name.get(&(file.clone(), name.clone())),
            reference,
            0.85,
            "same-file-callee-name",
        ) {
            return resolved;
        }
    }

    for name in &candidate_names {
        if let Some(resolved) = resolve_candidates(
            symbol_ids_by_name.get(name),
            reference,
            0.55,
            "project-unique-callee-name",
        ) {
            return resolved;
        }
    }

    TsCallTargetResolution::Unresolved
}

fn candidate_call_names(callee: &str) -> Vec<String> {
    let mut names = vec![callee.to_string()];
    if let Some(last) = callee.rsplit('.').next() {
        if last != callee {
            names.push(last.to_string());
        }
    }
    names
}

fn imported_call_lookup_keys(reference: &TsReference) -> Vec<(String, String)> {
    if let Some((prefix, last)) = reference.name.split_once('.') {
        return vec![(
            prefix.to_string(),
            last.rsplit('.').next().unwrap_or(last).to_string(),
        )];
    }
    vec![(reference.name.clone(), reference.name.clone())]
}

fn resolve_candidates(
    candidates: Option<&Vec<TsSymbolCandidate>>,
    reference: &TsReference,
    confidence: f64,
    reason: &'static str,
) -> Option<TsCallTargetResolution> {
    let candidates = candidates?;
    let preferred = preferred_symbol_kinds(reference);
    for preferred_kind in preferred {
        let matches = candidates
            .iter()
            .filter(|candidate| candidate.kind == *preferred_kind)
            .collect::<Vec<_>>();
        if matches.len() == 1 {
            return Some(TsCallTargetResolution::Resolved {
                target_id: matches[0].id.clone(),
                confidence,
                reason,
            });
        }
    }
    if candidates.len() == 1 {
        return Some(TsCallTargetResolution::Resolved {
            target_id: candidates[0].id.clone(),
            confidence,
            reason,
        });
    }
    if !candidates.is_empty() {
        return Some(TsCallTargetResolution::Ambiguous {
            candidates: candidates.len(),
        });
    }
    None
}

fn preferred_symbol_kinds(reference: &TsReference) -> &'static [TsSymbolKind] {
    match reference.kind {
        TsReferenceKind::NewExpression => &[TsSymbolKind::Class, TsSymbolKind::Component],
        TsReferenceKind::Call if reference.name.contains('.') => &[
            TsSymbolKind::Method,
            TsSymbolKind::Function,
            TsSymbolKind::Property,
        ],
        TsReferenceKind::Call => &[
            TsSymbolKind::Function,
            TsSymbolKind::Variable,
            TsSymbolKind::Class,
            TsSymbolKind::Method,
        ],
        _ => &[TsSymbolKind::Function],
    }
}
