//! Graph Schema v0 输出模块
//!
//! 把 ProjectModelOutput 映射成 graph-schema-v0.md 定义的 JSON one-shot graph。
//! 8 种 node types / 8 种 edge types，确定性输出。
//!
//! 映射策略：1:1，每个 Rust-core struct → 恰好一个 node type，不 merge 不 split。

use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

use crate::model::*;

// ============================================================
// 数据模型
// ============================================================

/// 8 种 node types
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NodeKind {
    Repository,
    Workspace,
    Package,
    Target,
    SourceFile,
    Module,
    Symbol,
    Diagnostic,
}

impl NodeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeKind::Repository => "repository",
            NodeKind::Workspace => "workspace",
            NodeKind::Package => "package",
            NodeKind::Target => "target",
            NodeKind::SourceFile => "source-file",
            NodeKind::Module => "module",
            NodeKind::Symbol => "symbol",
            NodeKind::Diagnostic => "diagnostic",
        }
    }
}

/// 8 种 edge types
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EdgeKind {
    ContainsWorkspace,
    ContainsPackage,
    HasTarget,
    OwnsSource,
    ResolvesTo,
    Defines,
    HasParent,
    Annotates,
}

impl EdgeKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            EdgeKind::ContainsWorkspace => "CONTAINS_WORKSPACE",
            EdgeKind::ContainsPackage => "CONTAINS_PACKAGE",
            EdgeKind::HasTarget => "HAS_TARGET",
            EdgeKind::OwnsSource => "OWNS_SOURCE",
            EdgeKind::ResolvesTo => "RESOLVES_TO",
            EdgeKind::Defines => "DEFINES",
            EdgeKind::HasParent => "HAS_PARENT",
            EdgeKind::Annotates => "ANNOTATES",
        }
    }
}

/// 单个 node
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphNode {
    pub id: String,
    pub label: String,
    pub properties: serde_json::Value,
}

/// 单条 edge
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    #[serde(rename = "type")]
    pub edge_type: String,
    #[serde(skip_serializing_if = "serde_json::Value::is_null")]
    pub properties: serde_json::Value,
}

/// Graph 统计
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphStats {
    pub node_count: u32,
    pub edge_count: u32,
    pub diagnostic_count: u32,
    pub symbol_count: u32,
}

/// Graph 输出顶层
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GraphOutput {
    pub schema_version: String,
    pub generated_at: String,
    pub root: serde_json::Value,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
    pub diagnostics: Vec<GraphNode>,
    pub stats: GraphStats,
}

// ============================================================
// 核心映射函数
// ============================================================

/// 把 ProjectModelOutput 映射为 GraphOutput。
///
/// 核心策略：
/// - 每种 ProjectModel struct 生成恰好一种 node
/// - Nodes 按 id 去重（BTreeMap），先插入者胜出
/// - Edges 按 (edge_type, source, target) 去重
/// - Diagnostic ID 含计数器，按 (code, path) 分组递增
/// - 输出按 id / (type, source, target) 字母序排列，保证确定性
pub fn emit_graph(output: &ProjectModelOutput) -> GraphOutput {
    let repo_id = format!("repo:{}", output.repo_root);

    // 收集容器
    let mut nodes: BTreeMap<String, GraphNode> = BTreeMap::new();
    let mut edges: BTreeSet<(String, String, String)> = BTreeSet::new();
    let mut edge_list: BTreeMap<(String, String, String), serde_json::Value> = BTreeMap::new();

    // ---- Repository node ----
    // repo root 不变，ID 格式 repo:{repoRoot}
    insert_node(
        &mut nodes,
        GraphNode {
            id: repo_id.clone(),
            label: NodeKind::Repository.as_str().to_string(),
            properties: serde_json::json!({
                "repoRoot": output.repo_root,
            }),
        },
    );

    // ---- Workspace nodes ----
    // 每个 WorkspaceModel → 1 个 workspace node + 1 条 CONTAINS_WORKSPACE edge
    for ws in &output.workspaces {
        let ws_id = format!("workspace:{}", ws.manifest_path);
        insert_node(
            &mut nodes,
            GraphNode {
                id: ws_id.clone(),
                label: NodeKind::Workspace.as_str().to_string(),
                properties: serde_json::json!({
                    "manifestPath": ws.manifest_path,
                    "workspaceRoot": ws.workspace_root,
                    "rawMembers": ws.raw_members,
                    "expandedMembers": ws.expanded_members,
                }),
            },
        );
        insert_edge(
            &mut edges,
            &mut edge_list,
            EdgeKind::ContainsWorkspace.as_str(),
            &repo_id,
            &ws_id,
            serde_json::Value::Null,
        );
    }

    // ---- Package nodes ----
    // CONTAINS_PACKAGE 来源判断：is_workspace_member → source 为对应 workspace；否则 → Repository
    for pkg in &output.packages {
        let pkg_id = format!("package:{}", pkg.manifest_path);
        insert_node(
            &mut nodes,
            GraphNode {
                id: pkg_id.clone(),
                label: NodeKind::Package.as_str().to_string(),
                properties: serde_json::json!({
                    "name": pkg.name,
                    "manifestPath": pkg.manifest_path,
                    "packageRoot": pkg.package_root,
                    "targetCount": pkg.target_count,
                    "featureNames": pkg.feature_names,
                    "isWorkspaceMember": pkg.is_workspace_member,
                    "discoveryReason": pkg.discovery_reason,
                }),
            },
        );

        // CONTAINS_PACKAGE 来源判断
        let source_id = if pkg.is_workspace_member {
            // workspace member → source 为第一个 manifest path 匹配的 workspace
            find_workspace_for_package(&output.workspaces, &pkg.manifest_path)
                .unwrap_or_else(|| repo_id.clone())
        } else {
            repo_id.clone()
        };
        insert_edge(
            &mut edges,
            &mut edge_list,
            EdgeKind::ContainsPackage.as_str(),
            &source_id,
            &pkg_id,
            serde_json::Value::Null,
        );
    }

    // ---- Target nodes ----
    // 每个 TargetModel → 1 个 target node + 1 条 HAS_TARGET edge
    for tgt in &output.targets {
        let tgt_id = format!("target:{}::{}::{}", tgt.package_name, tgt.name, tgt.kind);
        insert_node(
            &mut nodes,
            GraphNode {
                id: tgt_id.clone(),
                label: NodeKind::Target.as_str().to_string(),
                properties: serde_json::json!({
                    "packageName": tgt.package_name,
                    "name": tgt.name,
                    "kind": tgt.kind,
                    "crateRootFile": tgt.crate_root_file,
                    "sourceRootDir": tgt.source_root_dir,
                }),
            },
        );

        let pkg_id = find_package_by_name(&output.packages, &tgt.package_name);
        if let Some(pid) = &pkg_id {
            insert_edge(
                &mut edges,
                &mut edge_list,
                EdgeKind::HasTarget.as_str(),
                pid,
                &tgt_id,
                serde_json::Value::Null,
            );
        }
    }

    // ---- SourceFile nodes ----
    // 有 target → OWNS_SOURCE 从 target → source file
    // 有 package 无 target → OWNS_SOURCE 从 package → source file（source-target-ambiguous）
    // 无 package → 不产 OWNS_SOURCE edge，只在 diagnostics 中标记 source-outside-package
    for so in &output.source_ownership {
        let file_id = format!("file:{}", so.source_path);
        insert_node(
            &mut nodes,
            GraphNode {
                id: file_id.clone(),
                label: NodeKind::SourceFile.as_str().to_string(),
                properties: serde_json::json!({
                    "sourcePath": so.source_path,
                    "package": so.package,
                    "target": so.target,
                    "ownershipReason": so.ownership_reason,
                    "confidence": so.confidence,
                }),
            },
        );

        let source_id = if let Some(ref target_name) = so.target {
            // 有 target → 找 target node
            find_target_node(&output.targets, &so.package, target_name)
        } else if let Some(ref pkg_name) = so.package {
            // 有 package 无 target → OWNS_SOURCE 从 package
            find_package_by_name(&output.packages, pkg_name)
        } else {
            // 无 package 归属 → 不产 edge
            None
        };

        if let Some(sid) = source_id {
            insert_edge(
                &mut edges,
                &mut edge_list,
                EdgeKind::OwnsSource.as_str(),
                &sid,
                &file_id,
                serde_json::Value::Null,
            );
        }
    }

    // ---- RootResolution → RESOLVES_TO + Module nodes ----
    // resolvedKind = Some("module") → 额外生成 Module node
    // resolved 成功 → RESOLVES_TO edge
    // resolved 失败 → 不产 edge（diagnostics 已由 scanner 产出）
    for rr in &output.root_resolution {
        let from_file_id = format!("file:{}", rr.source_path);

        // Module node：resolvedKind == "module" 时生成
        if rr.resolved_kind.as_deref() == Some("module") {
            let mod_id = format!("module:{}::{}", rr.source_path, rr.query_path);
            insert_node(
                &mut nodes,
                GraphNode {
                    id: mod_id.clone(),
                    label: NodeKind::Module.as_str().to_string(),
                    properties: serde_json::json!({
                        "sourcePath": rr.source_path,
                        "queryPath": rr.query_path,
                        "resolvedKind": rr.resolved_kind,
                    }),
                },
            );

            // RESOLVES_TO: SourceFile → Module
            if rr.resolved_path.is_some() {
                insert_edge(
                    &mut edges,
                    &mut edge_list,
                    EdgeKind::ResolvesTo.as_str(),
                    &from_file_id,
                    &mod_id,
                    serde_json::json!({
                        "confidence": rr.confidence,
                        "rootReason": rr.root_reason,
                    }),
                );
            }
        } else if let Some(ref resolved_path) = rr.resolved_path {
            // resolvedKind == "file" 且有 resolved_path → RESOLVES_TO: SourceFile → SourceFile
            let to_file_id = format!("file:{}", resolved_path);
            insert_edge(
                &mut edges,
                &mut edge_list,
                EdgeKind::ResolvesTo.as_str(),
                &from_file_id,
                &to_file_id,
                serde_json::json!({
                    "confidence": rr.confidence,
                    "rootReason": rr.root_reason,
                }),
            );
        }
        // resolved 失败（resolved_path == None）：不产 edge
    }

    // ---- Symbol nodes (when --include symbols) ----
    for sym in &output.symbols {
        let sym_id = format!("symbol:{}", sym.id);
        insert_node(
            &mut nodes,
            GraphNode {
                id: sym_id.clone(),
                label: NodeKind::Symbol.as_str().to_string(),
                properties: serde_json::json!({
                    "name": sym.name,
                    "symbolKind": sym.symbol_kind,
                    "sourcePath": sym.source_path,
                    "packageName": sym.package_name,
                    "targetName": sym.target_name,
                    "modulePath": sym.module_path,
                    "visibility": sym.visibility,
                    "lineStart": sym.line_start,
                    "lineEnd": sym.line_end,
                    "genericParams": sym.generic_params,
                    "isAsync": sym.is_async,
                    "isUnsafe": sym.is_unsafe,
                    "isConstFn": sym.is_const_fn,
                    "isPub": sym.is_pub,
                    "implDetails": sym.impl_details,
                }),
            },
        );

        // DEFINES edge: SourceFile → Symbol
        let file_id = format!("file:{}", sym.source_path);
        insert_edge(
            &mut edges,
            &mut edge_list,
            EdgeKind::Defines.as_str(),
            &file_id,
            &sym_id,
            serde_json::Value::Null,
        );

        // HAS_PARENT edge: 子 Symbol → 父 Symbol
        if let Some(ref parent_id) = sym.parent_id {
            let parent_sym_id = format!("symbol:{}", parent_id);
            insert_edge(
                &mut edges,
                &mut edge_list,
                EdgeKind::HasParent.as_str(),
                &sym_id,
                &parent_sym_id,
                serde_json::Value::Null,
            );
        }
    }

    // ---- Diagnostics ----
    // Diagnostic ID 格式：diag:{code}::{path}::{index}
    // index 按 (code, path) 分组内递增计数器，保证确定性
    let mut diag_counter: BTreeMap<(String, String), u32> = BTreeMap::new();
    let mut diagnostic_nodes: Vec<GraphNode> = Vec::new();

    for diag in &output.diagnostics {
        let key = (diag.code.clone(), diag.path.clone());
        let idx = *diag_counter
            .entry(key.clone())
            .and_modify(|c| *c += 1)
            .or_insert(0);
        let diag_id = format!("diag:{}::{}::{}", diag.code, diag.path, idx);

        let node = GraphNode {
            id: diag_id.clone(),
            label: NodeKind::Diagnostic.as_str().to_string(),
            properties: serde_json::json!({
                "code": diag.code,
                "severity": diag.severity,
                "message": diag.message,
                "path": diag.path,
                "confidence": diag.confidence,
                "reason": diag.reason,
                "relatedPaths": diag.related_paths,
                "suggestedAction": diag.suggested_action,
            }),
        };
        diagnostic_nodes.push(node.clone());
        // 同时加入主 nodes map 以供 ANNOTATES 引用
        insert_node(&mut nodes, node);

        // ANNOTATES edge: Diagnostic → 对应 node（按 path 推断）
        let annotated_id = infer_annotated_node(&nodes, &diag.path);
        if let Some(target_id) = annotated_id {
            insert_edge(
                &mut edges,
                &mut edge_list,
                EdgeKind::Annotates.as_str(),
                &diag_id,
                &target_id,
                serde_json::Value::Null,
            );
        }
    }

    // Symbol diagnostics → Diagnostic nodes + ANNOTATES edges
    for sd in &output.symbol_diagnostics {
        let key = (sd.code.clone(), sd.source_path.clone());
        let idx = *diag_counter
            .entry(key.clone())
            .and_modify(|c| *c += 1)
            .or_insert(0);
        let diag_id = format!("diag:{}::{}::{}", sd.code, sd.source_path, idx);

        let node = GraphNode {
            id: diag_id.clone(),
            label: NodeKind::Diagnostic.as_str().to_string(),
            properties: serde_json::json!({
                "code": sd.code,
                "severity": sd.severity,
                "message": sd.message,
                "path": sd.source_path,
                "symbolId": sd.symbol_id,
                "suggestedAction": sd.suggested_action,
            }),
        };
        diagnostic_nodes.push(node.clone());
        insert_node(&mut nodes, node);

        // ANNOTATES: 如果有 symbol_id → 指向 symbol node
        if let Some(ref sym_id) = sd.symbol_id {
            let target_id = format!("symbol:{}", sym_id);
            if nodes.contains_key(&target_id) {
                insert_edge(
                    &mut edges,
                    &mut edge_list,
                    EdgeKind::Annotates.as_str(),
                    &diag_id,
                    &target_id,
                    serde_json::Value::Null,
                );
            }
        }
    }

    // ---- 排序输出（确定性保证）----
    let sorted_nodes: Vec<GraphNode> = nodes.into_values().collect();
    let sorted_edges: Vec<GraphEdge> = edges
        .into_iter()
        .map(|(edge_type, source, target)| {
            let props = edge_list
                .get(&(edge_type.clone(), source.clone(), target.clone()))
                .cloned()
                .unwrap_or(serde_json::Value::Null);
            GraphEdge {
                source,
                target,
                edge_type,
                properties: props,
            }
        })
        .collect();
    diagnostic_nodes.sort_by(|a, b| a.id.cmp(&b.id));

    let symbol_count = sorted_nodes
        .iter()
        .filter(|n| n.label == NodeKind::Symbol.as_str())
        .count() as u32;
    let diag_count = diagnostic_nodes.len() as u32;
    let node_count = sorted_nodes.len() as u32;
    let edge_count = sorted_edges.len() as u32;

    GraphOutput {
        schema_version: "0.1.0".to_string(),
        generated_at: output.generated_at.clone(),
        root: serde_json::json!({
            "repoRoot": output.repo_root,
        }),
        nodes: sorted_nodes,
        edges: sorted_edges,
        diagnostics: diagnostic_nodes,
        stats: GraphStats {
            node_count,
            edge_count,
            diagnostic_count: diag_count,
            symbol_count,
        },
    }
}

// ============================================================
// Helpers
// ============================================================

/// 插入 node，按 id 去重，先插入者胜出
fn insert_node(map: &mut BTreeMap<String, GraphNode>, node: GraphNode) {
    map.entry(node.id.clone()).or_insert(node);
}

/// 插入 edge，按 (edge_type, source, target) 去重
/// 如果已存在同名 edge，不做任何事（先到者胜出）
fn insert_edge(
    set: &mut BTreeSet<(String, String, String)>,
    map: &mut BTreeMap<(String, String, String), serde_json::Value>,
    edge_type: &str,
    source: &str,
    target: &str,
    properties: serde_json::Value,
) {
    let key = (
        edge_type.to_string(),
        source.to_string(),
        target.to_string(),
    );
    if set.insert(key.clone()) {
        map.insert(key, properties);
    }
}

/// 找到 package 所属的 workspace（通过 manifest path 前缀匹配）
fn find_workspace_for_package(workspaces: &[WorkspaceModel], pkg_manifest: &str) -> Option<String> {
    for ws in workspaces {
        for member in &ws.expanded_members {
            // pkg manifest 在 member 目录下
            if pkg_manifest.starts_with(member) || pkg_manifest == member {
                return Some(format!("workspace:{}", ws.manifest_path));
            }
        }
    }
    None
}

/// 通过 package name 找 package node id
fn find_package_by_name(packages: &[PackageModel], name: &str) -> Option<String> {
    for pkg in packages {
        if pkg.name == name {
            return Some(format!("package:{}", pkg.manifest_path));
        }
    }
    None
}

/// 通过 package_name + target_name 找 target node id
fn find_target_node(
    targets: &[TargetModel],
    pkg_name: &Option<String>,
    tgt_name: &str,
) -> Option<String> {
    if let Some(ref pn) = pkg_name {
        for tgt in targets {
            if tgt.package_name == *pn && tgt.name == tgt_name {
                return Some(format!(
                    "target:{}::{}::{}",
                    tgt.package_name, tgt.name, tgt.kind
                ));
            }
        }
    }
    None
}

/// 推断 diagnostic path 对应的 annotated node id
/// 优先级：file > package > repository
fn infer_annotated_node(nodes: &BTreeMap<String, GraphNode>, path: &str) -> Option<String> {
    // 优先匹配 file node
    let file_id = format!("file:{}", path);
    if nodes.contains_key(&file_id) {
        return Some(file_id);
    }
    // 匹配 package node（path 是 manifest path）
    let pkg_id = format!("package:{}", path);
    if nodes.contains_key(&pkg_id) {
        return Some(pkg_id);
    }
    // 匹配 workspace node
    let ws_id = format!("workspace:{}", path);
    if nodes.contains_key(&ws_id) {
        return Some(ws_id);
    }
    // fallback：repo node 一定存在
    for (id, node) in nodes {
        if node.label == NodeKind::Repository.as_str() {
            return Some(id.clone());
        }
    }
    None
}
