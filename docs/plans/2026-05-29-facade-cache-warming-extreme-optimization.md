# Facade Cache Warming 极致性能优化方案

> **状态**: Draft · **日期**: 2026-05-29 · **目标**: 将冷启动 facade cache warming 从 ~18.5s 压至 <3s
> **评估对象**: 交给其他智能体评审可行性、风险和优先级

---

## 0. 现状基准

以 CodeLattice 自分析（646 个 .rs 文件、44K nodes、60K edges）为基准：

```
总耗时 ~19.7s = 分析引擎 0.58s (3%) + facade cache warm ~18.5s (94%) + 其他 ~0.6s (3%)
```

**分析引擎并行化已达极致（36x），但端到端仅提升 5%。瓶颈完全在 facade cache warming。**

---

## 1. 瓶颈根因拆解（代码级定位）

### 1.1 JSON 中间层：数据穿越了 3 次

**当前数据流**:
```
inspect_project_model_with_options()    ← Rust struct (ProjectModelOutput)
    ↓ serde_json::to_value(&graph_output)   ← 转成 serde_json::Value 树
run_rust_analysis() 返回 Value
    ↓ build_warm_cache_entry_from_result()  ← Value 传入
    ↓ GraphView::build(&analyze_value)      ← 从 Value 里 .get("id").as_str() 一个个抠出来
    ↓ HashMap<String, Value>               ← 每个 node/edge 都 .clone()
```

**问题量化**:
- 44K nodes × clone + 60K edges × 2 (incoming + outgoing) = **~164K 次 `serde_json::Value` 深拷贝**
- 每个 node Value 含 properties（sourcePath, name, kind, lineStart 等），平均 200-500 bytes
- 总拷贝量估算：**50-80 MB 堆内存**，全是无意义的分配-复制-GC
- `.get("id").as_str()` 这种动态查询在热点路径上重复了 164K 次

**代码位置**:
- `crates/project-model/src/output.rs:1615` — `serde_json::to_value(&graph_output)`
- `crates/cli/src/mcp_server.rs:4065` — `GraphView::build(&analyze_value)` 逐 node 遍历
- `crates/cli/src/mcp_server.rs:4088-4158` — 遍历 nodes/edges 做 `.clone()`

### 1.2 文件系统 walk 至少 4 次

| walk | 函数 | 文件位置 | 目的 |
|------|------|----------|------|
| 1 | `scan_manifests()` | `project-model/src/manifest.rs:50` | 找 Cargo.toml |
| 2 | `collect_rs_files()` inside `scan_source_ownership()` | `project-model/src/source.rs:31` | 找 .rs 文件 |
| 3 | `scan_file_mtimes()` | `cli/src/mcp_server.rs:2016` | 找所有源文件拿 mtime |
| 4 | `DocScanner::build()` | `cli/src/mcp_server.rs` | 找 .md 文件 |
| 5? | `build_schedule()` inside `build_scheduler_metadata()` | `cli/src/mcp_server.rs:1851` | scheduler 可能再 walk |

每次 walk 都递归遍历整个目录树，跳过 `target/`, `.git/`, `node_modules/`。对 CodeLattice 自身，这是 5 次 `read_dir` 递归 × ~646 文件。

### 1.3 分析管线全串行

**当前 `inspect_project_model_with_options`** (`crates/project-model/src/output.rs:34`):

```rust
let scan = manifest::scan_manifests(root);                          // 串行
let source_result = source::scan_source_ownership(root, ...);       // 串行
let rr_result = root_resolution::scan_root_resolution(root, ...);   // 串行
let (symbols, ...) = extract_symbols_from_files(&*extractor, ...);  // 串行 for 循环
let (import_list, ...) = extract_and_resolve_imports(root, ...);    // 串行
let (call_list, ...) = extract_and_resolve_calls(root, ...);        // 串行（最重）
```

其中：
- `extract_symbols_from_files` (`item.rs:57`): 逐文件 for 循环，无 rayon
- `extract_and_resolve_calls` (`calls.rs:36`): 逐文件 for 循环，内部有 callee_index / import_bindings 查表，但这些索引是只读的，可以 `Arc` 共享

### 1.4 GraphView 内存布局浪费

```rust
// mcp_server.rs:4046
struct GraphView {
    nodes_by_id: HashMap<String, Value>,       // Value 是整个 node 的 JSON
    symbols_by_name: HashMap<String, Vec<Value>>,  // 又 clone 了一份 symbol nodes
    outgoing: HashMap<String, Vec<Value>>,     // edge clone
    incoming: HashMap<String, Vec<Value>>,     // edge 再 clone 一份
    diagnostics: Vec<Value>,
    // ...
}
```

同一个 node 在 `nodes_by_id` 和 `symbols_by_name` 里各存了一份完整的 `Value` clone。同一条 edge 在 `outgoing` 和 `incoming` 里也各存一份。**内存占用是实际数据的 3-4 倍**。

---

## 2. 优化方案（3 个层次，可独立交付）

### Phase A: 消灭 JSON 中间层（预计 -5~7s，改动中等，风险低）

**目标**: `ProjectModelOutput` → `GraphView` 直接构建，不经 `serde_json::Value`。

**当前**:
```
ProjectModelOutput → serde_json::to_value → serde_json::Value → GraphView::build
```

**改为**:
```
ProjectModelOutput → GraphView::build_from_model(pm_output)
```

**具体做法**:

1. **新建 `GraphView::build_from_model(pm: &ProjectModelOutput)`**
   - 直接遍历 `pm.symbols`, `pm.imports`, `pm.calls` 等 Rust struct
   - 用 `&str` 引用或 `Arc<str>` 代替 `String`
   - 索引结构改为 `HashMap<Arc<str>, usize>`（id → 在 Vec 中的位置），不做 clone

2. **保留 `GraphView::build(&Value)` 作为兼容层**
   - 非 Rust 语言（ArkTS/TS/Cangjie）仍走 JSON 路径
   - 但 GraphView 内部数据结构统一为 typed struct

3. **typed node/edge 替代 `serde_json::Value`**
   ```rust
   // 新设计
   struct GraphNode {
       id: Arc<str>,
       name: Arc<str>,
       kind: NodeKind,          // enum 而非 String
       label: &'static str,     // 编译期已知
       source_path: Option<Arc<str>>,
       line_start: Option<u32>,
       line_end: Option<u32>,
       properties: SmallVec<[(Arc<str>, Arc<str>); 4]>,  // 零分配常见情况
   }

   struct GraphEdge {
       source: Arc<str>,
       target: Arc<str>,
       edge_type: EdgeKind,     // enum 而非 String
       confidence: f64,
       reason: Arc<str>,
   }

   struct GraphView {
       nodes: Vec<GraphNode>,                    // 连续内存
       nodes_by_id: HashMap<Arc<str>, u32>,      // id → index
       symbols_by_name: HashMap<CompactString, SmallVec<[u32; 2]>>,  // name → indices
       outgoing: HashMap<Arc<str>, SmallVec<[u32; 4]>>,   // src_id → edge indices
       incoming: HashMap<Arc<str>, SmallVec<[u32; 4]>>,   // tgt_id → edge indices
       edges: Vec<GraphEdge>,                    // 连续内存
   }
   ```

4. **收益估算**:
   - 消除 ~164K 次 `Value` clone → ~0 次 clone（Arc 引用计数 + usize index）
   - 内存从 ~150MB（3-4x 膨胀）降到 ~40MB
   - HashMap 查询从 `.get("id").as_str()` 动态查询变成 `Arc<str>` 直接比较
   - 预计 GraphView 构建从 ~3-5s 降到 **<100ms**

**风险**: 低。不改变分析逻辑，只改数据表示层。`GraphView` 的所有 consumer（symbol_search, calls_from, impact_preview 等）改为通过 index 访问 node/edge 即可。

---

### Phase B: 统一文件发现 + 消除重复 walk（预计 -1~2s，改动小，风险极低）

**目标**: 一次 walk，所有下游 consumer 复用结果。

**具体做法**:

1. **新建 `FileDiscovery` struct**
   ```rust
   struct FileDiscovery {
       /// 所有源文件（相对路径 + mtime + size）
       source_files: Vec<DiscoveredFile>,
       /// 所有 manifest 文件
       manifests: Vec<DiscoveredFile>,
       /// 所有文档文件 (.md)
       docs: Vec<DiscoveredFile>,
       /// 构建时间戳
       discovered_at: Instant,
   }

   struct DiscoveredFile {
       relative_path: Arc<str>,
       absolute_path: PathBuf,
       extension: &'static str,
       mtime_ms: u64,
       size_bytes: u64,
   }
   ```

2. **在 `inspect_project_model_with_options` 入口处做一次 walk**
   ```rust
   let discovery = discover_all_files(root);
   let scan = manifest::scan_manifests_from_discovery(&discovery);
   let source_result = source::scan_source_ownership_from_discovery(&discovery, ...);
   // scan_file_mtimes → 直接用 discovery.source_files
   // DocScanner::build → 直接用 discovery.docs
   ```

3. **`scan_file_mtimes` 直接从 discovery 返回**
   - 不再 walk 文件系统
   - `compute_manifest_hashes` 也直接从 `discovery.manifests` 读

**收益估算**:
- 从 5 次 walk 减到 1 次
- 文件系统 I/O 从 ~2-3s 降到 ~0.5s
- 代码更简洁

**风险**: 极低。纯粹的"一次遍历，多处复用"重构，不改变任何分析逻辑。

---

### Phase C: 分析管线 rayon 并行化（预计 -5~8s，改动中等，风险中等）

**目标**: symbol 提取和 call 解析文件级并行。

**具体做法**:

1. **`extract_symbols_from_files` → rayon 并行**
   ```rust
   // item.rs — 当前
   for input in inputs {
       let output = extractor.extract_items(input);
       all_symbols.extend(output.symbols);
   }

   // 改为
   let results: Vec<ItemExtractionOutput> = inputs
       .par_iter()
       .map(|input| extractor.extract_items(input))
       .collect();
   ```

   - `ItemExtractor::extract_items` 只依赖 `input`（单文件内容），天然线程安全
   - 646 文件 × ~2ms/文件 = ~1.3s 串行 → rayon 4 线程 ≈ **~350ms**

2. **`extract_and_resolve_calls` → rayon 并行**
   ```rust
   // calls.rs — 当前
   for so in source_ownership {
       let calls = extract_calls_from_file(&source_text, ..., &symbol_index, &import_bindings, ...);
       all_calls.extend(calls);
   }

   // 改为
   let all_calls: Vec<CallSite> = source_ownership
       .par_iter()
       .flat_map(|so| {
           let source_text = std::fs::read_to_string(&abs_path).unwrap_or_default();
           extract_calls_from_file(&source_text, ..., &symbol_index, &import_bindings, ...)
       })
       .collect();
   ```

   - `CalleeIndex` 和 `ImportBindingTable` 是只读索引，用 `Arc` 共享
   - `extract_calls_from_file` 是纯函数，只读 source_text + 索引
   - 预计 ~10s → rayon 4 线程 ≈ **~3s**

3. **管线阶段 overlap**
   - 当前是严格串行：manifest → source → symbol → import → call
   - 可以 overlap 的部分：
     - `scan_manifests` + `scan_source_ownership` 可以并行（互不依赖）
     - `build_module_path_map` 可以和 symbol 提取 overlap

**风险**: 中等。
- `extract_items` 的实现需要确认线程安全（tree-sitter Parser 是否可跨线程）
- 如果 tree-sitter Parser 不可 Send/Sync，需要 per-thread 创建 Parser（rayon `map_with` 可解决）
- `extract_calls_from_file` 内部有 `format!` 等操作，需要确认无全局可变状态

**风险缓解**:
- `create_best_extractor()` 改为返回 `Arc<dyn ItemExtractor + Send + Sync>`
- 或使用 rayon 的 `par_iter().map_with(|| create_best_extractor(), ...)` per-thread 初始化

---

## 3. 极致方案：三层叠加后的预期效果

| 阶段 | 当前耗时 | Phase A 后 | Phase A+B 后 | Phase A+B+C 后 |
|------|----------|-----------|-------------|----------------|
| 文件系统 walk | ~2.5s (5次) | ~2.5s | **~0.5s (1次)** | ~0.5s |
| manifest/source/module 解析 | ~1s | ~1s | ~1s | ~0.5s (overlap) |
| symbol 提取 (AST/text) | ~1.5s | ~1.5s | ~1.5s | **~0.4s (rayon)** |
| call 解析 | ~10s | ~10s | ~10s | **~3s (rayon)** |
| JSON 序列化 + GraphView 构建 | ~3-5s | **~0.1s** | ~0.1s | ~0.1s |
| scan_file_mtimes + DocScanner | ~1.5s | ~1.5s | **~0.3s** | ~0.3s |
| metadata/digest | ~0.5s | ~0.5s | ~0.3s | ~0.3s |
| **总计** | **~18.5s** | **~17s** | **~13.5s** | **~5s** |

**注意**: Phase A 单独收益看似不大（18.5→17），但它是 Phase C 的前提——只有消灭了 JSON 中间层，rayon 并行才有意义（否则并行生成的 Value 在 merge 时又变成瓶颈）。

**Phase A + B + C 叠加后，从 18.5s 降到 ~5s，约 3.7x 加速。**

---

## 4. 进一步极致：如果要做到 <3s

在 Phase A+B+C 基础上，还有这些可做：

### 4.1 持久化 typed graph（增量 warm 的终极方案）

**核心思想**: 第一次冷启动做完分析后，把 `GraphView` 序列化到磁盘（用 bincode/postcard，不用 JSON）。后续冷启动直接 mmap + deserialize，跳过整个分析管线。

```rust
// 序列化（一次性）
let graph_view = GraphView::build_from_model(&pm_output);
let bytes = postcard::to_allocvec(&graph_view)?;
std::fs::write(cache_path, &bytes)?;

// 后续启动（~50ms）
let bytes = std::fs::read(&cache_path)?;
let graph_view: GraphView = postcard::from_bytes(&bytes)?;
```

预估：44K nodes + 60K edges 的 typed struct 序列化后约 **5-10 MB**，postcard 反序列化 ~50ms。

**前提**: Phase A 必须完成（typed struct 才能高效序列化，`serde_json::Value` 序列化巨大且无意义）。

### 4.2 增量分析（只分析变更文件）

当前每次 warm 都做全量分析。如果配合 `FileDiscovery` 的 mtime 对比：
- 只对 mtime 变更的文件重新 extract_symbols + extract_calls
- 索引更新用增量 merge（BTreeMap 的 range 操作）
- 预估日常增量 warm：**<1s**

### 4.3 mmap 文件读取

对大文件（如 `calls.rs` 1858 行），`std::fs::read_to_string` 需要分配 + 拷贝。可以用 `memmap2` crate 做 mmap 读取，避免用户态拷贝。对 646 个文件总体收益约 -0.3s。

---

## 5. 推荐实施顺序

```
Phase A (消灭 JSON 中间层)
  ↓ 验证基准后
Phase B (统一文件发现)
  ↓ 验证基准后
Phase C (rayon 并行化)
  ↓ 验证基准后
Phase 4.1 (持久化 typed graph)
  ↓ 可选
Phase 4.2 (增量分析)
```

**每个 Phase 独立可验证**，不需要同时改。每个 Phase 完成后跑 `cargo test` + `scripts/codelattice-precommit-check.sh` + 自分析 benchmark 对比。

---

## 6. 不做的事（明确边界）

- **不做 trait solving / type inference** — stop-line
- **不做 macro expansion** — stop-line
- **不做外部 crate API symbol resolution** — stop-line
- **不改 GitNexus-RC schema** — 不改 graph edge/node 的语义
- **不做 `cargo metadata`** — stop-line
- **不引入 unsafe 图数据结构** — 优化靠减少冗余工作，不是靠 unsafe hack

---

## 7. 关键文件清单

| 文件 | 角色 | Phase |
|------|------|-------|
| `crates/cli/src/mcp_server.rs:4046-4172` | GraphView 定义 + build | A |
| `crates/cli/src/mcp_server.rs:1617-1775` | build_warm_cache_entry_from_result | A |
| `crates/cli/src/mcp_server.rs:2016-2053` | scan_file_mtimes | B |
| `crates/cli/src/lib.rs:1597-1631` | run_rust_analysis | A |
| `crates/project-model/src/output.rs:34-131` | inspect_project_model_with_options | B,C |
| `crates/project-model/src/output.rs:282-284` | emit_graph_output | A |
| `crates/project-model/src/graph.rs:148-end` | emit_graph (BTreeMap 构建) | A |
| `crates/project-model/src/item.rs:57-74` | extract_symbols_from_files | C |
| `crates/project-model/src/calls.rs:36-110` | extract_and_resolve_calls | C |
| `crates/project-model/src/source.rs:31-end` | scan_source_ownership | B |
| `crates/project-model/src/manifest.rs:50-end` | scan_manifests | B |

---

## 8. 给评估者的关键问题

1. **Phase A 的 typed node/edge 设计**: 用 `Arc<str>` + `SmallVec` 的方案是否合适？还是用 `interning`（string interner 如 `ustr` crate）更极致？
2. **Phase C 的 tree-sitter 线程安全**: 当前 `create_best_extractor()` 返回的 `Box<dyn ItemExtractor>` 是否 `Send + Sync`？tree-sitter `Parser` 是否需要 per-thread 创建？
3. **Phase 4.1 的序列化格式选择**: `postcard`（紧凑、零拷贝 deserialize）vs `bincode`（更成熟）vs `capnp`（真正的零拷贝 mmap）？
4. **向后兼容**: `GraphView` 从 `serde_json::Value` 迁移到 typed struct 后，所有 MCP 工具的 consumer 都需要改。是否有遗漏的 consumer？
5. **增量分析的正确性**: 增量 symbol/call merge 在有跨文件引用时，如何保证索引一致性？
