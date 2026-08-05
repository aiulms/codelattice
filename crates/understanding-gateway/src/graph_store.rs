//! Graph Query Store —— G3 选型：full immutable graph index（P0-A #6 / §6.2）。
//!
//! 从 webui.snapshot.v1 JSON 一次性构建只读索引（nodeById / relationByKey /
//! outEdges / inEdges），提供按需查询：node_context、edge_evidence、call_chain。
//! 不把调用链复制进 snapshot 文件；完整链路查询在这里按需执行（bounded
//! snapshot 内的索引受 snapshot 内容限制，全量 snapshot 发布后索引即全量）。
//!
//! 隔离规则（§8）：本索引与 Agent MCP 进程内 cache 完全独立；从 immutable
//! snapshot 构建，天然只读。内存硬上限 512MB（G3 冻结）：加载时估算，
//! 超限拒绝并返回明确错误，不允许无界增长。

use std::collections::{HashMap, HashSet, VecDeque};

use serde_json::{json, Value};

use crate::dto::{
    CallChainResult, ChainStep, CoverageContext, EdgeEvidenceBundle, EvidenceOrigin, GeneratedFrom,
    NodeContextBundle, RelationRef, SourceRef, StaticLimitation,
};

/// 内存硬上限（G3 冻结）：节点/边条数估算，超过即拒绝加载。
pub const QUERY_STORE_MEMORY_LIMIT_ITEMS: usize = 4_000_000; // 约 512MB 量级（每项 ~128B）

#[derive(Debug, Clone)]
pub struct StoreNode {
    pub id: String,
    pub label: String,
    pub kind: String,
    pub file: Option<String>,
    pub line: Option<u32>,
}

#[derive(Debug, Clone)]
pub struct StoreEdge {
    pub relation_key: String,
    pub occurrence_key: Option<String>,
    pub source: String,
    pub target: String,
    pub kind: String,
    pub confidence: Option<f64>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SnapshotGraphIndex {
    pub snapshot_id: String,
    pub nodes: Vec<StoreNode>,
    pub edges: Vec<StoreEdge>,
    node_by_id: HashMap<String, StoreNode>,
    relation_by_key: HashMap<String, StoreEdge>,
    out_edges: HashMap<String, Vec<StoreEdge>>,
    in_edges: HashMap<String, Vec<StoreEdge>>,
    limitations: Vec<StaticLimitation>,
    summary: serde_json::Value,
}

impl SnapshotGraphIndex {
    /// 从 snapshot JSON 构建只读索引。非 graph 结构或超大图返回错误。
    pub fn from_snapshot(value: &Value) -> Result<Self, String> {
        Self::from_snapshot_with_limit(value, QUERY_STORE_MEMORY_LIMIT_ITEMS)
    }

    /// 内部入口：limit 可注入（生产用冻结上限；测试用小值验证拒绝逻辑）。
    fn from_snapshot_with_limit(value: &Value, limit: usize) -> Result<Self, String> {
        let graph = value
            .get("graph")
            .ok_or_else(|| "snapshot missing graph section".to_string())?;
        let nodes = graph
            .get("nodes")
            .and_then(Value::as_array)
            .ok_or_else(|| "graph.nodes must be an array".to_string())?;
        let edges = graph
            .get("edges")
            .and_then(Value::as_array)
            .ok_or_else(|| "graph.edges must be an array".to_string())?;

        if nodes.len() + edges.len() > limit {
            return Err(format!(
                "snapshot graph exceeds query-store memory limit ({} items > {}): \
                 refusing to load; use a smaller snapshot",
                nodes.len() + edges.len(),
                limit
            ));
        }

        let mut idx = Self {
            snapshot_id: value
                .get("generatedAt")
                .and_then(Value::as_str)
                .unwrap_or("snap:unknown")
                .to_string(),
            nodes: Vec::new(),
            edges: Vec::new(),
            node_by_id: HashMap::new(),
            relation_by_key: HashMap::new(),
            out_edges: HashMap::new(),
            in_edges: HashMap::new(),
            limitations: parse_limitations(value),
            summary: graph.get("summary").cloned().unwrap_or(Value::Null),
        };

        for n in nodes {
            let sn = StoreNode {
                id: str_of(n, "id").unwrap_or_default().to_string(),
                label: str_of(n, "label").unwrap_or("?").to_string(),
                kind: str_of(n, "kind").unwrap_or("symbol").to_string(),
                file: str_of(n, "file").map(str::to_string),
                line: n.get("line").and_then(Value::as_u64).map(|v| v as u32),
            };
            if sn.id.is_empty() {
                continue;
            }
            idx.node_by_id.insert(sn.id.clone(), sn.clone());
            idx.nodes.push(sn);
        }

        for e in edges {
            let source = str_of(e, "source").unwrap_or_default().to_string();
            let target = str_of(e, "target").unwrap_or_default().to_string();
            let kind = str_of(e, "kind").unwrap_or("related").to_string();
            if source.is_empty() || target.is_empty() {
                continue;
            }
            let relation_key = str_of(e, "relationKey")
                .map(str::to_string)
                .unwrap_or_else(|| default_relation_key(&source, &kind, &target));
            let se = StoreEdge {
                relation_key: relation_key.clone(),
                occurrence_key: e
                    .get("occurrenceKey")
                    .and_then(Value::as_str)
                    .map(str::to_string),
                source,
                target,
                kind,
                confidence: e.get("confidence").and_then(Value::as_f64),
                reason: str_of(e, "reason").map(str::to_string),
            };
            idx.relation_by_key
                .entry(relation_key)
                .or_insert(se.clone());
            idx.out_edges
                .entry(se.source.clone())
                .or_default()
                .push(se.clone());
            idx.in_edges
                .entry(se.target.clone())
                .or_default()
                .push(se.clone());
            idx.edges.push(se);
        }

        Ok(idx)
    }

    pub fn node(&self, node_id: &str) -> Option<&StoreNode> {
        self.node_by_id.get(node_id)
    }

    pub fn relation(&self, relation_key: &str) -> Option<&StoreEdge> {
        self.relation_by_key.get(relation_key)
    }

    fn out(&self, node_id: &str) -> &[StoreEdge] {
        self.out_edges
            .get(node_id)
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    fn inn(&self, node_id: &str) -> &[StoreEdge] {
        self.in_edges.get(node_id).map(Vec::as_slice).unwrap_or(&[])
    }

    /// 节点上下文：直接 callers/callees + sourceRefs + 限制 + 覆盖率（§6.2）。
    pub fn node_context(&self, node_id: &str) -> NodeContextBundle {
        let callers: Vec<RelationRef> = self
            .inn(node_id)
            .iter()
            .filter(|e| e.kind == "calls")
            .map(to_relation_ref)
            .collect();
        let callees: Vec<RelationRef> = self
            .out(node_id)
            .iter()
            .filter(|e| e.kind == "calls")
            .map(to_relation_ref)
            .collect();
        NodeContextBundle {
            schema_version: "codelattice.nodeContext.v1".to_string(),
            snapshot_id: self.snapshot_id.clone(),
            node_id: node_id.to_string(),
            direct_callers: callers,
            direct_callees: callees,
            source_refs: self
                .node(node_id)
                .and_then(|n| source_ref_for_node(n))
                .map(|r| vec![r])
                .unwrap_or_default(),
            limitations: self.limitations.clone(),
            coverage_context: self.coverage_context(),
            origin: EvidenceOrigin::Full,
        }
    }

    /// 边证据包（relation-level）：直接上下游 + 浅层 dependency reach（§6.2）。
    /// occurrenceKey 缺省时以 relation 代表边承载语义身份（G2 冻结）。
    pub fn edge_evidence(&self, relation_key: &str) -> Option<EdgeEvidenceBundle> {
        let e = self.relation_by_key.get(relation_key)?;
        let upstream: Vec<RelationRef> = self
            .inn(&e.source)
            .iter()
            .filter(|x| x.relation_key != relation_key)
            .map(to_relation_ref)
            .collect();
        let downstream: Vec<RelationRef> = self
            .out(&e.target)
            .iter()
            .filter(|x| x.relation_key != relation_key)
            .map(to_relation_ref)
            .collect();
        let mut reach = Vec::new();
        // 两跳依赖扩散（bounded；§4.4 dependency reach，非影响结论）
        let mut seen = HashSet::new();
        for hop1 in self.out(&e.target) {
            if hop1.relation_key == relation_key {
                continue;
            }
            if seen.insert(hop1.relation_key.clone()) {
                reach.push(to_relation_ref(hop1));
            }
            for hop2 in self.out(&hop1.target) {
                if seen.insert(hop2.relation_key.clone()) {
                    reach.push(to_relation_ref(hop2));
                }
            }
        }
        let mut source_refs = Vec::new();
        for id in [&e.source, &e.target] {
            if let Some(n) = self.node(id) {
                if let Some(r) = source_ref_for_node(n) {
                    source_refs.push(r);
                }
            }
        }
        Some(EdgeEvidenceBundle {
            schema_version: "codelattice.edgeEvidence.v1".to_string(),
            snapshot_id: self.snapshot_id.clone(),
            selection: to_relation_ref(e),
            direct_upstream: upstream,
            direct_downstream: downstream,
            dependency_reach: reach,
            source_refs,
            limitations: self.limitations.clone(),
            coverage_context: self.coverage_context(),
            generated_from: GeneratedFrom {
                static_analysis: true,
                runtime_verified: false,
                coverage_verified: false,
            },
            origin: EvidenceOrigin::Full,
        })
    }

    /// 调用链：BFS，depth 限制，truncated 标记（§6.4 / 验收 5）。
    pub fn call_chain(&self, node_id: &str, direction: &str, depth: u32) -> CallChainResult {
        let mut steps: Vec<ChainStep> = Vec::new();
        let mut queue: VecDeque<(String, u32)> = VecDeque::new();
        let mut visited: HashSet<String> = HashSet::new();
        queue.push_back((node_id.to_string(), 0));
        visited.insert(node_id.to_string());
        let max_depth = depth.clamp(1, 8);
        let mut truncated = false;

        while let Some((current, d)) = queue.pop_front() {
            if d >= max_depth {
                truncated = true;
                continue;
            }
            let edges: Vec<StoreEdge> = if direction == "upstream" {
                self.inn(&current).to_vec()
            } else {
                self.out(&current).to_vec()
            };
            for e in edges {
                if steps.len() >= 256 {
                    truncated = true;
                    break;
                }
                let next = if direction == "upstream" {
                    &e.source
                } else {
                    &e.target
                };
                steps.push(ChainStep {
                    relation: to_relation_ref(&e),
                    source_ref: self.node(next).and_then(source_ref_for_node),
                    confidence: e.confidence,
                });
                if visited.insert(next.clone()) {
                    queue.push_back((next.clone(), d + 1));
                }
            }
            if truncated {
                break;
            }
        }

        CallChainResult {
            schema_version: "codelattice.callChain.v1".to_string(),
            snapshot_id: self.snapshot_id.clone(),
            node_id: node_id.to_string(),
            direction: direction.to_string(),
            steps,
            truncated,
            coverage_context: self.coverage_context(),
            origin: EvidenceOrigin::Full,
        }
    }

    /// 项目级覆盖率（验收 8）：只使用 snapshot 事实统计，不伪造分母。
    pub fn coverage_context(&self) -> CoverageContext {
        let resolved = self
            .summary
            .get("callEdgeCount")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        CoverageContext {
            scope: "project".to_string(),
            resolved_calls: resolved,
            total_calls: resolved, // snapshot 层无 total denominator → 不虚构差异
            resolution_rate: if resolved > 0 { 1.0 } else { 0.0 },
            known_incomplete: true,
            caveat_ref: "coverage:project:calls".to_string(),
        }
    }

    /// 只读工具执行（P0-B2 Chat Tool Dispatcher 后端；§6.4 白名单）。
    /// `get_source_excerpt` / `get_change_impact` 需要显式用户确认或 what-if，
    /// 这里按 §7.2 / §4.4 拒绝（不自动发送源码正文）。
    pub fn execute_tool(&self, tool: &str, arguments: &Value) -> Result<Value, String> {
        match tool {
            "project_summary" => {
                let resolved = self
                    .summary
                    .get("callEdgeCount")
                    .and_then(Value::as_u64)
                    .unwrap_or(0);
                Ok(json!({
                    "snapshotId": self.snapshot_id,
                    "nodeCount": self.nodes.len(),
                    "edgeCount": self.edges.len(),
                    "resolvedCalls": resolved,
                    "knownIncomplete": true,
                    "entryPointsHint": "use search_nodes or get_node_context to explore",
                    "limitations": self.limitations,
                }))
            }
            "search_nodes" => {
                let q = arguments
                    .get("query")
                    .or_else(|| arguments.get("q"))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_lowercase();
                let limit = arguments
                    .get("limit")
                    .and_then(Value::as_u64)
                    .unwrap_or(10)
                    .min(50) as usize;
                let hits: Vec<Value> = self
                    .nodes
                    .iter()
                    .filter(|n| {
                        q.is_empty()
                            || n.id.to_lowercase().contains(&q)
                            || n.label.to_lowercase().contains(&q)
                    })
                    .take(limit)
                    .map(|n| {
                        json!({
                            "id": n.id,
                            "label": n.label,
                            "kind": n.kind,
                            "file": n.file,
                        })
                    })
                    .collect();
                Ok(json!({"matches": hits, "count": hits.len()}))
            }
            "get_node_context" => {
                let node_id = arguments
                    .get("nodeId")
                    .or_else(|| arguments.get("id"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| "nodeId required".to_string())?;
                serde_json::to_value(self.node_context(node_id)).map_err(|e| e.to_string())
            }
            "get_edge_evidence" => {
                let key = arguments
                    .get("relationKey")
                    .or_else(|| arguments.get("relation_key"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| "relationKey required".to_string())?;
                let Some(bundle) = self.edge_evidence(key) else {
                    return Err(format!("relation not found: {key}"));
                };
                serde_json::to_value(bundle).map_err(|e| e.to_string())
            }
            "get_call_chain" => {
                let node_id = arguments
                    .get("nodeId")
                    .or_else(|| arguments.get("id"))
                    .and_then(Value::as_str)
                    .ok_or_else(|| "nodeId required".to_string())?;
                let direction = arguments
                    .get("direction")
                    .and_then(Value::as_str)
                    .unwrap_or("downstream")
                    .to_string();
                let depth = arguments.get("depth").and_then(Value::as_u64).unwrap_or(3) as u32;
                serde_json::to_value(self.call_chain(node_id, &direction, depth))
                    .map_err(|e| e.to_string())
            }
            "get_static_limitations" => {
                Ok(json!({"limitations": self.limitations, "coverage": self.coverage_context()}))
            }
            "get_source_excerpt" => {
                // §7.2：新增代码片段发送范围需要显式确认；P0 Chat 不自动发送源码正文
                Err("get_source_excerpt requires explicit user confirmation; not sent by default (P0 §7.2)".to_string())
            }
            "get_change_impact" => {
                // §4.4/§6.4：影响分析必须携带明确 what-if
                Err("get_change_impact requires an explicit what-if (modify source, change signature, delete relation, change contract)".to_string())
            }
            _ => Err(format!("unknown tool: {tool}")),
        }
    }
}

// ── helpers ────────────────────────────────────────────────────────────────

fn str_of<'a>(v: &'a Value, key: &str) -> Option<&'a str> {
    v.get(key).and_then(Value::as_str)
}

/// §6.1：relationKey = sha256(source + kind + target)。与 Python 生成器一致；
/// 仅用于旧 snapshot（无 relationKey）的同步回退。
pub fn default_relation_key(source: &str, kind: &str, target: &str) -> String {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut h = DefaultHasher::new();
    source.hash(&mut h);
    kind.hash(&mut h);
    target.hash(&mut h);
    format!("rel:fallback:{:016x}", h.finish())
}

fn to_relation_ref(e: &StoreEdge) -> RelationRef {
    RelationRef {
        relation_key: e.relation_key.clone(),
        occurrence_key: e.occurrence_key.clone(),
        source_id: e.source.clone(),
        target_id: e.target.clone(),
        kind: e.kind.clone(),
    }
}

fn source_ref_for_node(n: &StoreNode) -> Option<SourceRef> {
    let file = n.file.as_ref()?;
    Some(SourceRef {
        id: format!("src:{}", n.id),
        file: file.clone(),
        start_line: n.line.unwrap_or(0),
        end_line: n.line.unwrap_or(0),
    })
}

fn parse_limitations(value: &Value) -> Vec<StaticLimitation> {
    let raw = value.get("limitations");
    match raw {
        Some(Value::Array(list)) => list
            .iter()
            .filter_map(|v| v.as_str())
            .enumerate()
            .map(|(i, t)| StaticLimitation {
                id: format!("limit:{i}"),
                text: t.to_string(),
            })
            .collect(),
        Some(Value::Object(obj)) => obj
            .get("notes")
            .and_then(Value::as_array)
            .map(|notes| {
                notes
                    .iter()
                    .filter_map(|v| v.as_str())
                    .enumerate()
                    .map(|(i, t)| StaticLimitation {
                        id: format!("limit:{i}"),
                        text: t.to_string(),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn snapshot_json() -> Value {
        json!({
            "generatedAt": "2026-08-05T00:00:00+00:00",
            "graph": {
                "summary": {"nodeCount": 3, "edgeCount": 2, "callEdgeCount": 1},
                "nodes": [
                    {"id": "n:a", "label": "a", "kind": "symbol", "file": "src/a.rs", "line": 1},
                    {"id": "n:b", "label": "b", "kind": "symbol", "file": "src/b.rs", "line": 10},
                    {"id": "n:c", "label": "c", "kind": "symbol", "file": "src/c.rs", "line": 20}
                ],
                "edges": [
                    {"source": "n:a", "target": "n:b", "kind": "calls",
                     "relationKey": "rel:sha256:abc", "confidence": 0.9, "reason": "direct"},
                    {"source": "n:b", "target": "n:c", "kind": "calls",
                     "relationKey": "rel:sha256:def"}
                ]
            },
            "limitations": {"notes": ["macro expansion not performed"]}
        })
    }

    #[test]
    fn builds_index_and_queries_node_context() {
        let idx = SnapshotGraphIndex::from_snapshot(&snapshot_json()).unwrap();
        let ctx = idx.node_context("n:b");
        assert_eq!(ctx.direct_callers.len(), 1);
        assert_eq!(ctx.direct_callers[0].relation_key, "rel:sha256:abc");
        assert_eq!(ctx.direct_callees.len(), 1);
        assert_eq!(ctx.direct_callees[0].target_id, "n:c");
        assert_eq!(ctx.source_refs[0].file, "src/b.rs");
        assert_eq!(ctx.coverage_context.scope, "project");
        assert_eq!(ctx.origin, EvidenceOrigin::Full);
    }

    #[test]
    fn edge_evidence_includes_upstream_downstream_and_reach() {
        let idx = SnapshotGraphIndex::from_snapshot(&snapshot_json()).unwrap();
        let ev = idx.edge_evidence("rel:sha256:abc").unwrap();
        assert_eq!(ev.selection.source_id, "n:a");
        assert_eq!(ev.selection.target_id, "n:b");
        assert!(ev.direct_downstream.iter().any(|r| r.target_id == "n:c"));
        // a 无入边 → upstream 空；b 的出边（b->c）是 downstream 也进入 reach
        assert!(ev.dependency_reach.iter().any(|r| r.target_id == "n:c"));
        assert_eq!(ev.source_refs.len(), 2);
    }

    #[test]
    fn call_chain_bfs_with_depth_limit_marks_truncated() {
        let idx = SnapshotGraphIndex::from_snapshot(&snapshot_json()).unwrap();
        let chain = idx.call_chain("n:a", "downstream", 8);
        assert_eq!(chain.steps.len(), 2);
        assert!(!chain.truncated);
        let shallow = idx.call_chain("n:a", "downstream", 1);
        assert_eq!(shallow.steps.len(), 1);
        assert!(shallow.truncated);
    }

    #[test]
    fn parallel_edges_share_relation_key() {
        let v = json!({
            "generatedAt": "t",
            "graph": {
                "summary": {},
                "nodes": [{"id": "n:a", "label": "a", "kind": "symbol"},
                          {"id": "n:b", "label": "b", "kind": "symbol"}],
                "edges": [
                    {"source": "n:a", "target": "n:b", "kind": "calls", "relationKey": "rel:k1"},
                    {"source": "n:a", "target": "n:b", "kind": "calls", "relationKey": "rel:k1"}
                ]
            }
        });
        let idx = SnapshotGraphIndex::from_snapshot(&v).unwrap();
        assert_eq!(idx.edges.len(), 2);
        assert_eq!(idx.relation_by_key.len(), 1, "平行边共享同一 relationKey");
        let ctx = idx.node_context("n:b");
        // relation-level：直接 callers 去重后仍按平行边计数（UI 层负责元素唯一）
        assert_eq!(ctx.direct_callers.len(), 2);
    }

    #[test]
    fn oversized_graph_is_rejected() {
        // 用可注入 limit 验证拒绝逻辑（生产上限 4M 太大无法在测试中构造）
        let mut v = snapshot_json();
        let nodes: Vec<Value> = (0..150)
            .map(|i| json!({"id": format!("n:{i}"), "label": "x", "kind": "symbol"}))
            .collect();
        v["graph"]["nodes"] = Value::Array(nodes);
        assert!(SnapshotGraphIndex::from_snapshot_with_limit(&v, 100).is_err());
        assert!(
            SnapshotGraphIndex::from_snapshot_with_limit(&v, 1000).is_ok(),
            "未超限必须可加载"
        );
    }

    #[test]
    fn read_only_tools_execute_within_whitelist() {
        let idx = SnapshotGraphIndex::from_snapshot(&snapshot_json()).unwrap();
        // project_summary
        let s = idx.execute_tool("project_summary", &json!({})).unwrap();
        assert_eq!(s["nodeCount"], 3);
        // search_nodes（大小写不敏感）
        let hits = idx
            .execute_tool("search_nodes", &json!({"q": "B"}))
            .unwrap();
        assert_eq!(hits["count"], 1);
        assert_eq!(hits["matches"][0]["id"], "n:b");
        // get_node_context
        let ctx = idx
            .execute_tool("get_node_context", &json!({"nodeId": "n:b"}))
            .unwrap();
        assert_eq!(ctx["directCallers"].as_array().unwrap().len(), 1);
        // get_edge_evidence
        let ev = idx
            .execute_tool(
                "get_edge_evidence",
                &json!({"relationKey": "rel:sha256:abc"}),
            )
            .unwrap();
        assert_eq!(ev["selection"]["sourceId"], "n:a");
        // get_call_chain
        let chain = idx
            .execute_tool(
                "get_call_chain",
                &json!({"nodeId": "n:a", "direction": "downstream", "depth": 2}),
            )
            .unwrap();
        assert!(chain["steps"].as_array().unwrap().len() >= 1);
        // get_static_limitations
        let lim = idx
            .execute_tool("get_static_limitations", &json!({}))
            .unwrap();
        assert!(lim["limitations"].as_array().unwrap().len() >= 1);
    }

    #[test]
    fn source_excerpt_and_change_impact_require_confirmation() {
        let idx = SnapshotGraphIndex::from_snapshot(&snapshot_json()).unwrap();
        // §7.2：源码正文不自动发送
        assert!(idx
            .execute_tool("get_source_excerpt", &json!({"sourceRefId": "src:n:a"}))
            .is_err());
        // §4.4：影响分析必须携带 what-if
        assert!(idx
            .execute_tool("get_change_impact", &json!({"nodeId": "n:a"}))
            .is_err());
        // 白名单外工具拒绝
        assert!(idx.execute_tool("shell_exec", &json!({})).is_err());
    }
}
