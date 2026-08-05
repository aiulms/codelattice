// query_store —— 有界 LRU QueryStore（返工第二轮 D-fix）。
//
// 独立类型，不再暴露裸 HashMap。同时限制 maxSnapshots 和 maxEstimatedBytes。
// pinned snapshot 不被逐出；全 pinned 且超限时拒绝加载。
use std::collections::HashMap;
use std::sync::Arc;

use understanding_gateway::graph_store::SnapshotGraphIndex;

/// 单条索引的估算字节数计算（节点 ~200B + 边 ~300B + HashMap overhead）。
fn estimate_bytes(idx: &SnapshotGraphIndex) -> u64 {
    let node_bytes = idx.nodes.len() as u64 * 200;
    let edge_bytes = idx.edges.len() as u64 * 300;
    // HashMap overhead: 每个 entry ~64B，node_by_id + relation_by_key + out + in = 4 maps
    let map_entries = idx.nodes.len() as u64 + idx.edges.len() as u64 * 3;
    node_bytes + edge_bytes + map_entries * 64
}

struct Entry {
    index: Arc<SnapshotGraphIndex>,
    bytes: u64,
    last_access: u64,
    pinned: bool,
}

/// 有界 LRU QueryStore。
pub struct QueryStore {
    entries: HashMap<String, Entry>,
    max_snapshots: usize,
    max_bytes: u64,
    clock: u64,
    // metrics
    hit_count: u64,
    miss_count: u64,
    eviction_count: u64,
}

/// 只读 metrics 快照。
#[derive(Debug, Clone, serde::Serialize)]
pub struct QueryStoreMetrics {
    pub current_snapshots: usize,
    pub current_estimated_bytes: u64,
    pub max_estimated_bytes: u64,
    pub hit_count: u64,
    pub miss_count: u64,
    pub eviction_count: u64,
    pub pinned_count: usize,
}

impl QueryStore {
    pub fn new(max_snapshots: usize, max_bytes: u64) -> Self {
        Self {
            entries: HashMap::new(),
            max_snapshots,
            max_bytes,
            clock: 0,
            hit_count: 0,
            miss_count: 0,
            eviction_count: 0,
        }
    }

    /// 获取索引（cache hit 更新 LRU 顺序）。
    pub fn get(&mut self, snapshot_id: &str) -> Option<Arc<SnapshotGraphIndex>> {
        self.clock += 1;
        if let Some(e) = self.entries.get_mut(snapshot_id) {
            e.last_access = self.clock;
            self.hit_count += 1;
            Some(e.index.clone())
        } else {
            self.miss_count += 1;
            None
        }
    }

    /// 插入索引。如果全 pinned 且超限 → 拒绝并返回错误。
    pub fn insert(
        &mut self,
        snapshot_id: String,
        index: Arc<SnapshotGraphIndex>,
        pinned_snapshots: &[String],
    ) -> Result<(), String> {
        // 如果已存在，更新（不重复计费）
        let bytes = estimate_bytes(&index);
        let id_ref = &snapshot_id;
        if self.entries.contains_key(id_ref) {
            self.clock += 1;
            let e = self.entries.get_mut(&snapshot_id).unwrap();
            e.index = index;
            e.bytes = bytes;
            e.last_access = self.clock;
            e.pinned = pinned_snapshots.contains(&snapshot_id);
            return Ok(());
        }

        // eviction：如果需要腾出空间
        self.evict_to_fit(bytes, pinned_snapshots)?;

        self.clock += 1;
        self.entries.insert(
            snapshot_id.clone(),
            Entry {
                index,
                bytes,
                last_access: self.clock,
                pinned: pinned_snapshots.contains(&snapshot_id),
            },
        );
        Ok(())
    }

    /// 从 store 中移除指定 snapshot。
    pub fn remove(&mut self, snapshot_id: &str) {
        if self.entries.remove(snapshot_id).is_some() {
            // 不计入 eviction_count（这是显式移除，不是 LRU 逐出）
        }
    }

    /// 标记/取消标记 pinned 状态。
    pub fn set_pinned(&mut self, snapshot_id: &str, pinned: bool) {
        if let Some(e) = self.entries.get_mut(snapshot_id) {
            e.pinned = pinned;
        }
    }

    pub fn metrics(&self) -> QueryStoreMetrics {
        let current_bytes: u64 = self.entries.values().map(|e| e.bytes).sum();
        let pinned_count = self.entries.values().filter(|e| e.pinned).count();
        QueryStoreMetrics {
            current_snapshots: self.entries.len(),
            current_estimated_bytes: current_bytes,
            max_estimated_bytes: self.max_bytes,
            hit_count: self.hit_count,
            miss_count: self.miss_count,
            eviction_count: self.eviction_count,
            pinned_count,
        }
    }

    /// 执行 LRU eviction 直到满足 bytes 和 count 上限。
    /// 全 pinned 且超限时返回错误。
    fn evict_to_fit(&mut self, incoming_bytes: u64, pinned: &[String]) -> Result<(), String> {
        let current_bytes: u64 = self.entries.values().map(|e| e.bytes).sum();

        // 检查 count 上限
        while self.entries.len() >= self.max_snapshots {
            if !self.try_evict_oldest(pinned) {
                return Err(format!(
                    "query_store full: all {} entries are pinned, cannot load new snapshot",
                    self.entries.len()
                ));
            }
        }

        // 检查 bytes 上限
        let mut projected = current_bytes + incoming_bytes;
        while projected > self.max_bytes {
            if !self.try_evict_oldest(pinned) {
                return Err(format!(
                    "query_store memory budget exceeded: projected {} bytes > max {} bytes; all remaining entries are pinned",
                    projected, self.max_bytes
                ));
            }
            projected = self.entries.values().map(|e| e.bytes).sum::<u64>() + incoming_bytes;
        }

        Ok(())
    }

    /// 尝试逐出最旧的非 pinned 条目。返回 false 如果没有可逐出的。
    fn try_evict_oldest(&mut self, pinned: &[String]) -> bool {
        // 找到 last_access 最小的非 pinned 条目
        let oldest = self
            .entries
            .iter()
            .filter(|(id, e)| !pinned.contains(id) && !e.pinned)
            .min_by_key(|(_, e)| e.last_access)
            .map(|(id, _)| id.clone());
        if let Some(id) = oldest {
            self.entries.remove(&id);
            self.eviction_count += 1;
            true
        } else {
            false
        }
    }
}

impl Default for QueryStore {
    fn default() -> Self {
        // 8 snapshots, 512MB aggregate
        Self::new(8, 512 * 1024 * 1024)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn make_index(n_nodes: usize) -> Arc<SnapshotGraphIndex> {
        let mut nodes = Vec::new();
        for i in 0..n_nodes {
            nodes.push(json!({
                "id": format!("n:{i}"),
                "label": format!("node_{i}"),
                "kind": "symbol",
            }));
        }
        let snapshot = json!({
            "schemaVersion": "webui.snapshot.v1",
            "generatedAt": "2026-01-01T00:00:00Z",
            "graph": {
                "status": "ok",
                "stability": "stable",
                "nodes": nodes,
                "edges": [],
                "summary": {"nodeCount": n_nodes, "edgeCount": 0, "symbolNodeCount": n_nodes, "fileNodeCount": 0, "callEdgeCount": 0},
                "truncated": false,
                "cautions": [],
            },
            "limitations": {"notes": []},
        });
        Arc::new(SnapshotGraphIndex::from_snapshot(&snapshot).unwrap())
    }

    #[test]
    fn cache_hit_updates_lru_order() {
        let mut store = QueryStore::new(3, u64::MAX);
        store.insert("s1".into(), make_index(10), &[]).unwrap();
        store.insert("s2".into(), make_index(10), &[]).unwrap();
        store.insert("s3".into(), make_index(10), &[]).unwrap();
        // 访问 s1 → s1 成为最近访问
        store.get("s1");
        // 插入 s4 → 应逐出 s2（最旧未访问）
        store.insert("s4".into(), make_index(10), &[]).unwrap();
        assert!(
            store.get("s1").is_some(),
            "s1 should survive (recently accessed)"
        );
        assert!(store.get("s2").is_none(), "s2 should be evicted (oldest)");
        assert!(store.get("s3").is_some());
        assert!(store.get("s4").is_some());
    }

    #[test]
    fn pinned_entry_not_evicted() {
        let mut store = QueryStore::new(2, u64::MAX);
        store
            .insert("s1".into(), make_index(10), &["s1".into()])
            .unwrap();
        store.insert("s2".into(), make_index(10), &[]).unwrap();
        // 插入 s3 → 应逐出 s2，保留 pinned s1
        store
            .insert("s3".into(), make_index(10), &["s1".into()])
            .unwrap();
        assert!(store.get("s1").is_some(), "pinned s1 must survive");
        assert!(store.get("s2").is_none(), "s2 should be evicted");
    }

    #[test]
    fn all_pinned_rejects_load() {
        let mut store = QueryStore::new(2, u64::MAX);
        store
            .insert("s1".into(), make_index(10), &["s1".into()])
            .unwrap();
        store
            .insert("s2".into(), make_index(10), &["s2".into()])
            .unwrap();
        // 两个都是 pinned，插入第三个应失败
        let result = store.insert("s3".into(), make_index(10), &["s1".into(), "s2".into()]);
        assert!(result.is_err(), "should reject when all pinned");
        assert!(result.unwrap_err().contains("pinned"));
    }

    #[test]
    fn byte_limit_effective() {
        // 每个索引 10 nodes × ~200B + maps ≈ 2640B。
        // 设 max=2640+100 → 只能放1个；第二个会逐出第一个（非 pinned），然后成功。
        // 要测试真正拒绝，需要全 pinned：两个 pinned 条目都放进去后再加第三个 → 拒绝。
        let mut store = QueryStore::new(100, 2640 * 2 + 100);
        store
            .insert("s1".into(), make_index(10), &["s1".into()])
            .unwrap();
        store
            .insert("s2".into(), make_index(10), &["s2".into()])
            .unwrap();
        // 两个 pinned 占满，第三个超 bytes 上限且全 pinned → 拒绝
        let result = store.insert("s3".into(), make_index(10), &["s1".into(), "s2".into()]);
        assert!(
            result.is_err(),
            "should reject: all pinned + byte limit exceeded"
        );
    }

    #[test]
    fn metrics_are_correct() {
        let mut store = QueryStore::new(5, u64::MAX);
        store.insert("s1".into(), make_index(10), &[]).unwrap();
        store.get("s1"); // hit
        store.get("s2"); // miss
        let m = store.metrics();
        assert_eq!(m.current_snapshots, 1);
        assert_eq!(m.hit_count, 1);
        assert_eq!(m.miss_count, 1);
        assert_eq!(m.eviction_count, 0);
        assert!(m.current_estimated_bytes > 0);
    }

    #[test]
    fn remove_deletes_entry() {
        let mut store = QueryStore::new(5, u64::MAX);
        store.insert("s1".into(), make_index(10), &[]).unwrap();
        store.remove("s1");
        assert!(store.get("s1").is_none());
        let m = store.metrics();
        assert_eq!(m.current_snapshots, 0);
    }

    #[test]
    fn duplicate_insert_does_not_double_charge() {
        let mut store = QueryStore::new(5, u64::MAX);
        store.insert("s1".into(), make_index(10), &[]).unwrap();
        let bytes_after_first = store.metrics().current_estimated_bytes;
        store.insert("s1".into(), make_index(10), &[]).unwrap();
        let bytes_after_second = store.metrics().current_estimated_bytes;
        assert_eq!(
            bytes_after_first, bytes_after_second,
            "duplicate insert must not double charge"
        );
    }
}
