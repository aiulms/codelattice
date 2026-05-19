//! 跨项目影响分析：BFS 遍历工作区图，计算受影响的项目/资产/边界

use crate::{WorkspaceEdge, WorkspaceGraph, WorkspaceNode};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet, VecDeque};

// ── Types ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ImpactDirection {
    Upstream,
    Downstream,
    Both,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactTarget {
    pub node_id: Option<String>,
    pub project_id: Option<String>,
    pub path: Option<String>,
    pub snapshot_id: Option<String>,
    pub query: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactResult {
    pub schema_version: String,
    pub target: ResolvedTarget,
    pub summary: ImpactSummary,
    pub affected_projects: Vec<AffectedItem>,
    pub affected_assets: Vec<AffectedItem>,
    pub unsupported_boundaries: Vec<AffectedItem>,
    pub paths: Vec<ImpactPath>,
    pub risk_reasons: Vec<String>,
    pub review_checklist: Vec<String>,
    pub cautions: Vec<String>,
    pub generated_from: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedTarget {
    pub input: String,
    pub resolved_node_id: Option<String>,
    pub resolved_kind: String,
    pub label: String,
    pub path: String,
    pub resolution_confidence: f64,
    pub resolution_reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactSummary {
    pub risk_level: String,
    pub total_affected_projects: usize,
    pub total_affected_assets: usize,
    pub total_unsupported_boundaries: usize,
    pub total_impact_paths: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AffectedItem {
    pub node_id: String,
    pub kind: String,
    pub label: String,
    pub path: String,
    pub distance: usize,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpactPath {
    pub from: String,
    pub to: String,
    pub edges: Vec<PathEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PathEdge {
    pub edge_id: String,
    pub from: String,
    pub to: String,
    pub kind: String,
    pub confidence: f64,
}

// ── Target resolution ────────────────────────────────────────────────────

/// 按优先级解析目标节点
fn resolve_target<'a>(
    graph: &'a WorkspaceGraph,
    target: &ImpactTarget,
) -> (Option<&'a WorkspaceNode>, f64, String) {
    // 1. 精确 node_id 匹配 → 1.0
    if let Some(ref nid) = target.node_id {
        if let Some(node) = graph.nodes.iter().find(|n| n.id == *nid) {
            return (Some(node), 1.0, "exact-node-id-match".to_string());
        }
    }

    // 2. 精确 project_id 匹配 → 1.0
    if let Some(ref pid) = target.project_id {
        if let Some(node) = graph.nodes.iter().find(|n| n.project_id == *pid) {
            return (Some(node), 1.0, "exact-project-id-match".to_string());
        }
    }

    // 3. 精确 path 匹配 → 0.90
    if let Some(ref path) = target.path {
        if let Some(node) = graph
            .nodes
            .iter()
            .find(|n| n.path == *path || n.relative_path == *path)
        {
            return (Some(node), 0.90, "exact-path-match".to_string());
        }
    }

    // 4. 后缀 path 匹配 → 0.75
    if let Some(ref path) = target.path {
        if let Some(node) = graph
            .nodes
            .iter()
            .find(|n| n.path.ends_with(path) || n.relative_path.ends_with(path))
        {
            return (Some(node), 0.75, "suffix-path-match".to_string());
        }
    }

    // 5. query / label 匹配（case-insensitive） → 0.65
    let query = target
        .query
        .as_deref()
        .or(target.path.as_deref())
        .or(target.project_id.as_deref())
        .or(target.node_id.as_deref())
        .unwrap_or("");

    if !query.is_empty() {
        let query_lower = query.to_lowercase();
        if let Some(node) = graph
            .nodes
            .iter()
            .find(|n| n.label.to_lowercase() == query_lower)
        {
            return (Some(node), 0.65, "label-match".to_string());
        }

        // 6. 模糊子串匹配 → 0.45
        if let Some(node) = graph
            .nodes
            .iter()
            .find(|n| n.label.to_lowercase().contains(&query_lower))
        {
            return (Some(node), 0.45, "fuzzy-substring-match".to_string());
        }
    }

    // 7. 未匹配 → 0.0
    (None, 0.0, "no-match".to_string())
}

// ── BFS traversal ────────────────────────────────────────────────────────

/// 邻接表：node_id → [(edge, neighbor_node_id)]
type AdjList = HashMap<String, Vec<(WorkspaceEdge, String)>>;

fn build_forward_adj(edges: &[WorkspaceEdge]) -> AdjList {
    let mut adj: AdjList = HashMap::new();
    for e in edges {
        adj.entry(e.source.clone())
            .or_default()
            .push((e.clone(), e.target.clone()));
    }
    adj
}

fn build_reverse_adj(edges: &[WorkspaceEdge]) -> AdjList {
    let mut adj: AdjList = HashMap::new();
    for e in edges {
        adj.entry(e.target.clone())
            .or_default()
            .push((e.clone(), e.source.clone()));
    }
    adj
}

struct BfsState {
    visited: HashSet<String>,
    items: Vec<AffectedItem>,
    paths: Vec<ImpactPath>,
}

/// BFS 遍历：不沿 contains、adjacent_to、unsupported_boundary 继续传播
fn bfs_traverse(
    start_id: &str,
    adj: &AdjList,
    node_map: &HashMap<&str, &WorkspaceNode>,
    max_depth: usize,
) -> BfsState {
    let mut state = BfsState {
        visited: HashSet::new(),
        items: Vec::new(),
        paths: Vec::new(),
    };
    state.visited.insert(start_id.to_string());

    // BFS 队列：(node_id, depth, path_confidence, accumulated_path_edges)
    let mut queue: VecDeque<(String, usize, f64, Vec<PathEdge>)> = VecDeque::new();
    queue.push_back((start_id.to_string(), 0, 1.0, Vec::new()));

    while let Some((cur_id, depth, path_conf, path_edges)) = queue.pop_front() {
        if depth >= max_depth {
            continue;
        }

        let neighbors = match adj.get(&cur_id) {
            Some(n) => n,
            None => continue,
        };

        for (edge, neighbor_id) in neighbors {
            if state.visited.contains(neighbor_id) {
                continue;
            }

            // contains 边只在初始展开时用（workspace→project），不传播
            if edge.kind == "contains" && depth > 0 {
                continue;
            }

            // adjacent_to 和 unsupported_boundary 是弱边：只记录，不继续传播
            let is_weak = edge.kind == "adjacent_to" || edge.kind == "unsupported_boundary";

            // 弱边置信度上限 0.4
            let edge_conf = if is_weak {
                edge.confidence.min(0.4)
            } else {
                edge.confidence
            };

            let new_path_conf = path_conf.min(edge_conf);

            state.visited.insert(neighbor_id.clone());

            // 记录受影响项
            if let Some(&node) = node_map.get(neighbor_id.as_str()) {
                state.items.push(AffectedItem {
                    node_id: neighbor_id.clone(),
                    kind: node.kind.clone(),
                    label: node.label.clone(),
                    path: node.path.clone(),
                    distance: depth + 1,
                    confidence: new_path_conf,
                });
            }

            // 记录路径
            let mut new_edges = path_edges.clone();
            new_edges.push(PathEdge {
                edge_id: edge.id.clone(),
                from: cur_id.clone(),
                to: neighbor_id.clone(),
                kind: edge.kind.clone(),
                confidence: edge_conf,
            });

            if !path_edges.is_empty() || depth > 0 {
                state.paths.push(ImpactPath {
                    from: path_edges
                        .first()
                        .map(|p| p.from.clone())
                        .unwrap_or_else(|| start_id.to_string()),
                    to: neighbor_id.clone(),
                    edges: new_edges.clone(),
                });
            }

            // 弱边不继续传播
            if is_weak {
                continue;
            }

            queue.push_back((neighbor_id.clone(), depth + 1, new_path_conf, new_edges));
        }
    }

    state
}

// ── Risk level ───────────────────────────────────────────────────────────

fn compute_risk_level(affected_projects: usize, has_unsupported_boundary: bool) -> String {
    if affected_projects >= 10 {
        "critical".to_string()
    } else if affected_projects >= 4 || has_unsupported_boundary {
        "high".to_string()
    } else if affected_projects >= 2 {
        "medium".to_string()
    } else if affected_projects >= 1 {
        "low".to_string()
    } else {
        "unknown".to_string()
    }
}

// ── Review checklist generation ──────────────────────────────────────────

fn generate_review_checklist(
    affected_projects: &[AffectedItem],
    affected_assets: &[AffectedItem],
    unsupported: &[AffectedItem],
) -> Vec<String> {
    let mut checklist = Vec::new();

    for proj in affected_projects {
        checklist.push(format!(
            "Review changes in project '{}' ({})",
            proj.label, proj.path
        ));
    }

    for asset in affected_assets {
        checklist.push(format!(
            "Check impact on {} '{}' at {}",
            asset.kind, asset.label, asset.path
        ));
    }

    for bnd in unsupported {
        checklist.push(format!(
            "WARNING: unsupported boundary crossing to '{}' ({})",
            bnd.label, bnd.kind
        ));
    }

    // 去重
    checklist.sort();
    checklist.dedup();
    checklist
}

// ── Public API ───────────────────────────────────────────────────────────

/// 跨项目影响分析
pub fn cross_project_impact(
    graph: &WorkspaceGraph,
    target: &ImpactTarget,
    direction: ImpactDirection,
    max_depth: usize,
) -> ImpactResult {
    // 解析目标
    let (resolved_node, res_conf, res_reason) = resolve_target(graph, target);

    // 构建 node 查找表
    let node_map: HashMap<&str, &WorkspaceNode> =
        graph.nodes.iter().map(|n| (n.id.as_str(), n)).collect();

    let (input_desc, resolved_id, resolved_kind, resolved_label, resolved_path) =
        match resolved_node {
            Some(node) => (
                format_target_input(target),
                Some(node.id.clone()),
                node.kind.clone(),
                node.label.clone(),
                node.path.clone(),
            ),
            None => (
                format_target_input(target),
                None::<String>,
                "unknown".to_string(),
                String::new(),
                String::new(),
            ),
        };

    let resolved_target = ResolvedTarget {
        input: input_desc,
        resolved_node_id: resolved_id.clone(),
        resolved_kind: resolved_kind.clone(),
        label: resolved_label,
        path: resolved_path,
        resolution_confidence: res_conf,
        resolution_reason: res_reason,
    };

    // 如果目标未解析，返回空结果
    let Some(start_id) = &resolved_id else {
        return ImpactResult {
            schema_version: "workspace.impact.v1".to_string(),
            target: resolved_target,
            summary: ImpactSummary {
                risk_level: "unknown".to_string(),
                total_affected_projects: 0,
                total_affected_assets: 0,
                total_unsupported_boundaries: 0,
                total_impact_paths: 0,
            },
            affected_projects: Vec::new(),
            affected_assets: Vec::new(),
            unsupported_boundaries: Vec::new(),
            paths: Vec::new(),
            risk_reasons: Vec::new(),
            review_checklist: vec!["Target not resolved — no impact analysis possible".to_string()],
            cautions: vec![
                "static analysis only".to_string(),
                "no runtime proof".to_string(),
                "heuristic".to_string(),
            ],
            generated_from: serde_json::json!({
                "generator": "gitnexus-workspace-model",
                "version": env!("CARGO_PKG_VERSION"),
                "staticAnalysis": true,
                "runtimeVerified": false,
                "scriptsExecuted": false,
                "coverageVerified": false,
                "heuristic": true,
            }),
        };
    };

    // BFS 遍历
    let mut all_items: Vec<AffectedItem> = Vec::new();
    let mut all_paths: Vec<ImpactPath> = Vec::new();

    match direction {
        ImpactDirection::Downstream => {
            let fwd = build_forward_adj(&graph.edges);
            let state = bfs_traverse(start_id, &fwd, &node_map, max_depth);
            all_items = state.items;
            all_paths = state.paths;
        }
        ImpactDirection::Upstream => {
            let rev = build_reverse_adj(&graph.edges);
            let state = bfs_traverse(start_id, &rev, &node_map, max_depth);
            all_items = state.items;
            all_paths = state.paths;
        }
        ImpactDirection::Both => {
            let fwd = build_forward_adj(&graph.edges);
            let rev = build_reverse_adj(&graph.edges);

            let fwd_state = bfs_traverse(start_id, &fwd, &node_map, max_depth);
            let rev_state = bfs_traverse(start_id, &rev, &node_map, max_depth);

            all_items.extend(fwd_state.items);
            all_items.extend(rev_state.items);
            all_paths.extend(fwd_state.paths);
            all_paths.extend(rev_state.paths);

            // 去重（按 node_id）
            let mut seen_ids = HashSet::new();
            all_items.retain(|item| seen_ids.insert(item.node_id.clone()));
        }
    }

    // 分类受影响项
    let mut affected_projects = Vec::new();
    let mut affected_assets = Vec::new();
    let mut unsupported_boundaries = Vec::new();

    for item in all_items {
        match item.kind.as_str() {
            "project" => affected_projects.push(item),
            "unsupported" => unsupported_boundaries.push(item),
            _ => affected_assets.push(item),
        }
    }

    // 额外检查：受影响的项目中是否有 unsupported 的
    let has_unsupported = !unsupported_boundaries.is_empty()
        || affected_projects.iter().any(|item| {
            node_map
                .get(item.node_id.as_str())
                .map_or(false, |n| !n.supported)
        });

    // 风险等级
    let risk_level = compute_risk_level(affected_projects.len(), has_unsupported);

    // 风险理由
    let mut risk_reasons = Vec::new();
    if affected_projects.len() >= 4 {
        risk_reasons.push(format!(
            "high blast radius: {} affected projects",
            affected_projects.len()
        ));
    }
    if has_unsupported {
        risk_reasons
            .push("unsupported boundary crossing detected — limited visibility".to_string());
    }
    if risk_level == "critical" {
        risk_reasons.push("critical risk: >=10 projects affected".to_string());
    }
    if risk_reasons.is_empty() {
        risk_reasons.push(format!(
            "{} risk level based on {} affected projects",
            risk_level,
            affected_projects.len()
        ));
    }

    let review_checklist = generate_review_checklist(
        &affected_projects,
        &affected_assets,
        &unsupported_boundaries,
    );

    let total_projects = affected_projects.len();
    let total_assets = affected_assets.len();
    let total_unsupported = unsupported_boundaries.len();
    let total_paths = all_paths.len();

    ImpactResult {
        schema_version: "workspace.impact.v1".to_string(),
        target: resolved_target,
        summary: ImpactSummary {
            risk_level,
            total_affected_projects: total_projects,
            total_affected_assets: total_assets,
            total_unsupported_boundaries: total_unsupported,
            total_impact_paths: total_paths,
        },
        affected_projects,
        affected_assets,
        unsupported_boundaries,
        paths: all_paths,
        risk_reasons,
        review_checklist,
        cautions: vec![
            "static analysis only".to_string(),
            "no runtime proof".to_string(),
            "heuristic".to_string(),
        ],
        generated_from: serde_json::json!({
            "generator": "gitnexus-workspace-model",
            "version": env!("CARGO_PKG_VERSION"),
            "staticAnalysis": true,
            "runtimeVerified": false,
            "scriptsExecuted": false,
            "coverageVerified": false,
            "heuristic": true,
        }),
    }
}

fn format_target_input(target: &ImpactTarget) -> String {
    if let Some(ref nid) = target.node_id {
        return nid.clone();
    }
    if let Some(ref pid) = target.project_id {
        return pid.clone();
    }
    if let Some(ref p) = target.path {
        return p.clone();
    }
    if let Some(ref q) = target.query {
        return q.clone();
    }
    "<empty target>".to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;

    fn make_test_graph() -> WorkspaceGraph {
        let nodes = vec![
            WorkspaceNode {
                id: "workspace:root".to_string(),
                kind: "workspace".to_string(),
                label: "root".to_string(),
                path: ".".to_string(),
                relative_path: ".".to_string(),
                language: String::new(),
                supported: true,
                project_id: "workspace:root".to_string(),
                metadata: serde_json::json!({}),
            },
            WorkspaceNode {
                id: "project:rust-core".to_string(),
                kind: "project".to_string(),
                label: "rust-core".to_string(),
                path: "./rust-core".to_string(),
                relative_path: "rust-core".to_string(),
                language: "rust".to_string(),
                supported: true,
                project_id: "project:rust-core".to_string(),
                metadata: serde_json::json!({}),
            },
            WorkspaceNode {
                id: "project:web-ui".to_string(),
                kind: "project".to_string(),
                label: "web-ui".to_string(),
                path: "./web-ui".to_string(),
                relative_path: "web-ui".to_string(),
                language: "typescript".to_string(),
                supported: true,
                project_id: "project:web-ui".to_string(),
                metadata: serde_json::json!({}),
            },
            WorkspaceNode {
                id: "project:legacy-java".to_string(),
                kind: "project".to_string(),
                label: "legacy-java".to_string(),
                path: "./legacy-java".to_string(),
                relative_path: "legacy-java".to_string(),
                language: "java".to_string(),
                supported: false,
                project_id: "project:legacy-java".to_string(),
                metadata: serde_json::json!({}),
            },
        ];

        let edges = vec![
            WorkspaceEdge {
                id: "e1".to_string(),
                kind: "contains".to_string(),
                source: "workspace:root".to_string(),
                target: "project:rust-core".to_string(),
                confidence: 1.0,
                reason: "contains".to_string(),
                evidence: None,
            },
            WorkspaceEdge {
                id: "e2".to_string(),
                kind: "contains".to_string(),
                source: "workspace:root".to_string(),
                target: "project:web-ui".to_string(),
                confidence: 1.0,
                reason: "contains".to_string(),
                evidence: None,
            },
            WorkspaceEdge {
                id: "e3".to_string(),
                kind: "contains".to_string(),
                source: "workspace:root".to_string(),
                target: "project:legacy-java".to_string(),
                confidence: 1.0,
                reason: "contains".to_string(),
                evidence: None,
            },
            WorkspaceEdge {
                id: "e4".to_string(),
                kind: "depends_on".to_string(),
                source: "project:web-ui".to_string(),
                target: "project:rust-core".to_string(),
                confidence: 0.85,
                reason: "cargo-path-dependency".to_string(),
                evidence: None,
            },
            WorkspaceEdge {
                id: "e5".to_string(),
                kind: "unsupported_boundary".to_string(),
                source: "project:rust-core".to_string(),
                target: "project:legacy-java".to_string(),
                confidence: 0.45,
                reason: "boundary".to_string(),
                evidence: None,
            },
        ];

        WorkspaceGraph {
            schema_version: "workspace.graph.v1".to_string(),
            root: ".".to_string(),
            nodes,
            edges,
            summary: WorkspaceGraphSummary {
                node_count: 4,
                edge_count: 5,
                project_count: 3,
                cross_project_edge_count: 1,
                unsupported_boundary_count: 1,
                top_connected_projects: vec![],
                bridge_scripts: vec![],
                bridge_configs: vec![],
            },
            cautions: vec![],
            generated_from: serde_json::json!({}),
        }
    }

    #[test]
    fn test_resolve_target_exact_node_id() {
        let graph = make_test_graph();
        let target = ImpactTarget {
            node_id: Some("project:rust-core".to_string()),
            project_id: None,
            path: None,
            snapshot_id: None,
            query: None,
        };
        let (node, conf, reason) = resolve_target(&graph, &target);
        assert!(node.is_some());
        assert_eq!(conf, 1.0);
        assert_eq!(reason, "exact-node-id-match");
    }

    #[test]
    fn test_resolve_target_label_match() {
        let graph = make_test_graph();
        let target = ImpactTarget {
            node_id: None,
            project_id: None,
            path: None,
            snapshot_id: None,
            query: Some("web-ui".to_string()),
        };
        let (node, conf, reason) = resolve_target(&graph, &target);
        assert!(node.is_some());
        assert_eq!(conf, 0.65);
        assert_eq!(reason, "label-match");
    }

    #[test]
    fn test_resolve_target_no_match() {
        let graph = make_test_graph();
        let target = ImpactTarget {
            node_id: None,
            project_id: None,
            path: None,
            snapshot_id: None,
            query: Some("nonexistent".to_string()),
        };
        let (node, conf, _) = resolve_target(&graph, &target);
        assert!(node.is_none());
        assert_eq!(conf, 0.0);
    }

    #[test]
    fn test_cross_project_impact_downstream() {
        let graph = make_test_graph();
        let target = ImpactTarget {
            node_id: Some("project:rust-core".to_string()),
            project_id: None,
            path: None,
            snapshot_id: None,
            query: None,
        };
        let result = cross_project_impact(&graph, &target, ImpactDirection::Downstream, 3);
        // downstream from rust-core hits legacy-java via unsupported_boundary (weak edge)
        // legacy-java is a project node, so it goes into affected_projects not unsupported_boundaries
        // With only 1 affected project, risk is "low" unless there's an unsupported boundary
        assert_eq!(result.summary.risk_level, "high");
        // legacy-java is unsupported, so unsupported_boundaries should contain it
        assert!(
            !result.unsupported_boundaries.is_empty()
                || result.affected_projects.iter().any(|p| !graph
                    .nodes
                    .iter()
                    .find(|n| n.id == p.node_id)
                    .map_or(true, |n| n.supported))
        );
    }

    #[test]
    fn test_cross_project_impact_upstream() {
        let graph = make_test_graph();
        let target = ImpactTarget {
            node_id: Some("project:rust-core".to_string()),
            project_id: None,
            path: None,
            snapshot_id: None,
            query: None,
        };
        let result = cross_project_impact(&graph, &target, ImpactDirection::Upstream, 3);
        // upstream: web-ui depends_on rust-core
        assert!(result.affected_projects.len() >= 1);
    }

    #[test]
    fn test_risk_level_critical() {
        assert_eq!(compute_risk_level(10, false), "critical");
    }

    #[test]
    fn test_risk_level_high_unsupported() {
        assert_eq!(compute_risk_level(1, true), "high");
    }

    #[test]
    fn test_risk_level_medium() {
        assert_eq!(compute_risk_level(2, false), "medium");
    }

    #[test]
    fn test_risk_level_low() {
        assert_eq!(compute_risk_level(1, false), "low");
    }
}
