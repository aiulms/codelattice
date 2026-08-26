pub mod impact;

use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

// ── Types ────────────────────────────────────────────────────────────────

/// 扫描到的项目基本信息
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInfo {
    pub path: String,
    pub relative_path: String,
    pub name: String,
    pub language: String,
    pub supported: bool,
    pub manifest_file: String,
    /// 是否有 manifest 文件（Cargo.toml/package.json 等）。
    /// false 表示仅通过源文件扩展名推断，属于 source-only area。
    #[serde(default)]
    pub is_manifest_backed: bool,
}

/// 工作区图节点
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub path: String,
    pub relative_path: String,
    pub language: String,
    pub supported: bool,
    pub project_id: String,
    pub metadata: serde_json::Value,
}

/// 工作区图边
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceEdge {
    pub id: String,
    pub kind: String,
    pub source: String,
    pub target: String,
    pub confidence: f64,
    pub reason: String,
    pub evidence: Option<serde_json::Value>,
}

/// 工作区图摘要
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGraphSummary {
    pub node_count: usize,
    pub edge_count: usize,
    pub project_count: usize,
    pub cross_project_edge_count: usize,
    pub unsupported_boundary_count: usize,
    pub top_connected_projects: Vec<serde_json::Value>,
    pub bridge_scripts: Vec<serde_json::Value>,
    pub bridge_configs: Vec<serde_json::Value>,
}

/// 工作区图
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceGraph {
    pub schema_version: String,
    pub root: String,
    pub nodes: Vec<WorkspaceNode>,
    pub edges: Vec<WorkspaceEdge>,
    pub summary: WorkspaceGraphSummary,
    pub cautions: Vec<String>,
    pub generated_from: serde_json::Value,
}

// ── Constants ────────────────────────────────────────────────────────────

const MAX_WALK_DEPTH: usize = 5;
const MAX_ENTRIES: usize = 5000;

/// 支持的语言 → manifest 文件名
const SUPPORTED_MANIFESTS: &[(&str, &str)] = &[
    ("Cargo.toml", "rust"),
    ("package.json", "typescript"),
    ("pyproject.toml", "python"),
    ("setup.py", "python"),
    ("cjpm.toml", "cangjie"),
    ("CMakeLists.txt", "c"),
];

/// 不支持的语言 → manifest 文件名
const UNSUPPORTED_MANIFESTS: &[(&str, &str)] = &[
    (".csproj", "csharp"),
    (".sln", "csharp"),
    ("go.mod", "go"),
    ("build.gradle", "java"),
    ("pom.xml", "java"),
];

/// 目录扫描跳过列表
const SKIP_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "target",
    "dist",
    "build",
    "__pycache__",
    ".cache",
    ".next",
    ".venv",
    "venv",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
    ".codelattice-webui",
    "dogfood-output",
    "playwright-runtime",
    "bazel-out",
    "bazel-bin",
];

/// 源文件扩展名 → 语言（用于无 manifest 目录的语言检测）。
///
/// 已导出供跨 crate 复用：webui_snapshot 用它做节点语言身份标注。
/// 表内含当前不可分析语言（csharp/go/java/…）是有意的——身份标注 ≠ 可分析性；
/// 消费方不得在本表之外另建映射副本，两份表必漂。
pub const SOURCE_EXTENSIONS: &[(&str, &str)] = &[
    (".rs", "rust"),
    (".ts", "typescript"),
    (".tsx", "typescript"),
    (".js", "typescript"),
    (".py", "python"),
    (".cj", "cangjie"),
    (".c", "c"),
    (".cpp", "cpp"),
    (".h", "c"),
    (".hpp", "cpp"),
    (".sh", "shell"),
    (".cs", "csharp"),
    (".go", "go"),
    (".java", "java"),
    (".kt", "kotlin"),
    (".swift", "swift"),
];

// ── Language support classification ──────────────────────────────────────

fn is_language_supported(lang: &str) -> bool {
    matches!(
        lang,
        "rust" | "typescript" | "cangjie" | "arkts" | "c" | "cpp" | "python" | "shell"
    )
}

// ── scan_workspace_inventory ─────────────────────────────────────────────

/// 扫描工作区目录，检测所有项目和配置文件
pub fn scan_workspace_inventory(
    root: &Path,
    redact_root: bool,
) -> Result<Vec<ProjectInfo>, String> {
    if !root.is_dir() {
        return Err(format!("root is not a directory: {}", root.display()));
    }
    let root = root
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize root: {}", e))?;

    let mut projects = Vec::new();
    let mut visited = HashSet::new();
    let mut entry_count = 0;

    walk_dir_for_projects(
        &root,
        &root,
        0,
        &mut projects,
        &mut visited,
        &mut entry_count,
        redact_root,
    );

    Ok(projects)
}

fn walk_dir_for_projects(
    root: &Path,
    dir: &Path,
    depth: usize,
    projects: &mut Vec<ProjectInfo>,
    visited: &mut HashSet<PathBuf>,
    entry_count: &mut usize,
    redact_root: bool,
) {
    if depth > MAX_WALK_DEPTH || *entry_count >= MAX_ENTRIES {
        return;
    }

    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };

    let entries: Vec<_> = entries.flatten().collect();

    // 先检测当前目录是否为项目（有 manifest）
    if let Some(info) = detect_project_at(dir, root, redact_root) {
        projects.push(info);
        visited.insert(dir.to_path_buf());
        // 项目目录内部仍继续扫描（可能有嵌套子项目）
    }

    for entry in &entries {
        *entry_count += 1;
        if *entry_count > MAX_ENTRIES {
            return;
        }

        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        // 跳过常见非项目目录
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if SKIP_DIRS.contains(&name) || name.starts_with('.') {
                continue;
            }
        }

        if visited.contains(&path) {
            continue;
        }

        walk_dir_for_projects(
            root,
            &path,
            depth + 1,
            projects,
            visited,
            entry_count,
            redact_root,
        );
    }
}

/// 在指定目录检测是否包含 manifest 文件，返回项目信息
fn detect_project_at(dir: &Path, root: &Path, redact_root: bool) -> Option<ProjectInfo> {
    // 先检查支持的 manifest
    for (manifest_name, language) in SUPPORTED_MANIFESTS {
        let manifest_path = dir.join(manifest_name);
        if manifest_path.exists() {
            let name = dir
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            let relative = pathdiff_or_fallback(dir, root);
            return Some(ProjectInfo {
                path: redact_path(dir, root, redact_root),
                relative_path: relative,
                name,
                language: language.to_string(),
                supported: is_language_supported(language),
                manifest_file: manifest_name.to_string(),
                is_manifest_backed: true,
            });
        }
    }

    // 检查不支持的 manifest
    for (manifest_name, language) in UNSUPPORTED_MANIFESTS {
        if manifest_name.starts_with('.') {
            // .csproj, .sln — 检查目录中是否有匹配扩展名的文件
            let has_ext = fs::read_dir(dir).ok().map_or(false, |mut entries| {
                entries.any(|e| {
                    e.ok()
                        .and_then(|e| {
                            e.path()
                                .extension()
                                .and_then(|ext| ext.to_str().map(|s| s.to_string()))
                        })
                        .map_or(false, |ext| {
                            manifest_name
                                .trim_start_matches('.')
                                .eq_ignore_ascii_case(&ext)
                        })
                })
            });
            if has_ext {
                let name = dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let relative = pathdiff_or_fallback(dir, root);
                return Some(ProjectInfo {
                    path: redact_path(dir, root, redact_root),
                    relative_path: relative,
                    name,
                    language: language.to_string(),
                    supported: false,
                    manifest_file: manifest_name.to_string(),
                    is_manifest_backed: true,
                });
            }
        } else {
            let manifest_path = dir.join(manifest_name);
            if manifest_path.exists() {
                let name = dir
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                let relative = pathdiff_or_fallback(dir, root);
                return Some(ProjectInfo {
                    path: redact_path(dir, root, redact_root),
                    relative_path: relative,
                    name,
                    language: language.to_string(),
                    supported: false,
                    manifest_file: manifest_name.to_string(),
                    is_manifest_backed: true,
                });
            }
        }
    }

    // 无 manifest：尝试通过源文件扩展名检测语言
    detect_by_extensions(dir, root, redact_root)
}

/// 通过源文件扩展名推断目录语言（仅用于无 manifest 的情况）
fn detect_by_extensions(dir: &Path, root: &Path, redact_root: bool) -> Option<ProjectInfo> {
    let entries = fs::read_dir(dir).ok()?;
    let mut lang_counts: HashMap<String, usize> = HashMap::new();
    let mut source_count = 0;

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext_with_dot = format!(".{}", ext.to_lowercase());
            for (suffix, lang) in SOURCE_EXTENSIONS {
                if ext_with_dot == *suffix {
                    *lang_counts.entry(lang.to_string()).or_insert(0) += 1;
                    source_count += 1;
                    break;
                }
            }
        }
    }

    // 至少有 2 个源文件才认为是项目目录
    if source_count < 2 {
        return None;
    }

    // 取最多的语言
    let best_lang = lang_counts
        .iter()
        .max_by_key(|(_, c)| *c)
        .map(|(l, _)| l.as_str())
        .unwrap_or("unknown");

    let name = dir
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();
    let relative = pathdiff_or_fallback(dir, root);

    Some(ProjectInfo {
        path: redact_path(dir, root, redact_root),
        relative_path: relative,
        name,
        language: best_lang.to_string(),
        supported: is_language_supported(best_lang),
        manifest_file: String::new(),
        is_manifest_backed: false,
    })
}

// ── inspect_workspace_inventory（多语言卡 2）──────────────────────────────
// 独立体检入口：与 scan_workspace_inventory 并行，不复用其"单目录单语言"摘要语义。
// 隔离约束：autoEntry / MCP 共享 scan_workspace_inventory 与 detect_by_extensions，
// 二者行为语义零改动；本节只新增类型与新函数，遍历防护复用既有常量。

/// 体检行：一行 = (relativePath, language)。unrecognized 行 language 为 None（缺席）。
/// 结构只在进程内传给 CLI 组装 JSON，不落盘不走网络，不 derive serde。
#[derive(Debug, Clone)]
pub struct InspectionArea {
    pub relative_path: String,
    pub name: Option<String>,
    pub language: Option<String>,
    pub confidence: &'static str,
    pub evidence: InspectionEvidence,
    pub source_file_count: u64,
    /// None = 可分析行；known-unsupported（认得出、不会做）/ unrecognized（自定义扩展名）
    pub recognition: Option<&'static str>,
}

#[derive(Debug, Clone)]
pub enum InspectionEvidence {
    Manifest { file: String },
    ExtensionHistogram { extension: String, count: u64 },
}

/// 体检结果三桶；桶归属按静态语言支持表，analyzable（这台二进制能不能跑）
/// 由 CLI 层按 feature 判定，本层只产语言与证据。
#[derive(Debug, Clone, Default)]
pub struct WorkspaceInspection {
    pub projects: Vec<InspectionArea>,
    pub source_only_areas: Vec<InspectionArea>,
    pub unsupported_areas: Vec<InspectionArea>,
}

/// 单目录体检中间结果：直方图只按当前目录文件计，不递归子目录。
struct InspectionDirHist {
    /// 表内语言 → 当前目录文件数
    langs: std::collections::BTreeMap<String, u64>,
    /// 表外扩展名 → 当前目录文件数（unrecognized）
    unknown_exts: std::collections::BTreeMap<String, u64>,
    /// L1 manifest 命中（文件名、语言、静态可分析性）
    manifest: Option<(String, &'static str, bool)>,
}

/// 文件夹体检：逐目录产出多语言行，回答"这里有什么项目/语言/多少文件"。
pub fn inspect_workspace_inventory(root: &Path) -> Result<WorkspaceInspection, String> {
    if !root.is_dir() {
        return Err(format!("root is not a directory: {}", root.display()));
    }
    let root = root
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize root: {e}"))?;

    let mut dir_rows: BTreeMap<String, InspectionDirHist> = BTreeMap::new();
    let mut entry_count = 0usize;
    inspect_walk(&root, &root, 0, &mut dir_rows, &mut entry_count);

    let mut insp = WorkspaceInspection::default();
    // (manifest 目录, 语言)：嵌套压制与 sourceFileCount 树内求和的锚点
    let mut manifest_rows: Vec<(String, String)> = Vec::new();

    for (rel, hist) in &dir_rows {
        if let Some((file, lang, supported)) = &hist.manifest {
            // manifest 行 name 与 detect_project_at 同口径（目录名，根目录为 unknown）
            let name = Path::new(rel)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown")
                .to_string();
            let area = InspectionArea {
                relative_path: rel.clone(),
                name: Some(name),
                language: Some(lang.to_string()),
                confidence: "certain",
                evidence: InspectionEvidence::Manifest { file: file.clone() },
                // 先占 0，行生成后按树内直方图求和回填
                source_file_count: 0,
                recognition: if *supported {
                    None
                } else {
                    Some("known-unsupported")
                },
            };
            if *supported {
                insp.projects.push(area);
            } else {
                insp.unsupported_areas.push(area);
            }
            manifest_rows.push((rel.clone(), lang.to_string()));
        }

        for (lang, count) in &hist.langs {
            if is_language_supported(lang) {
                // L3 可分析区 ≥2 才报（对齐现有扫描器阈值；1 个 .py 不报）
                if *count >= 2 {
                    insp.source_only_areas.push(InspectionArea {
                        relative_path: rel.clone(),
                        name: None,
                        language: Some(lang.clone()),
                        confidence: "medium",
                        evidence: InspectionEvidence::ExtensionHistogram {
                            extension: extension_of_language(lang),
                            count: *count,
                        },
                        source_file_count: *count,
                        recognition: None,
                    });
                }
            } else {
                // unsupported 表内语言 ≥1 就报（1 个 .java 也要看见）
                insp.unsupported_areas.push(InspectionArea {
                    relative_path: rel.clone(),
                    name: None,
                    language: Some(lang.clone()),
                    confidence: "medium",
                    evidence: InspectionEvidence::ExtensionHistogram {
                        extension: extension_of_language(lang),
                        count: *count,
                    },
                    source_file_count: *count,
                    recognition: Some("known-unsupported"),
                });
            }
        }

        for (ext, count) in &hist.unknown_exts {
            // unrecognized：自定义扩展名，可能是生成物；language 省略，≥1 就报
            insp.unsupported_areas.push(InspectionArea {
                relative_path: rel.clone(),
                name: None,
                language: None,
                confidence: "low",
                evidence: InspectionEvidence::ExtensionHistogram {
                    extension: ext.clone(),
                    count: *count,
                },
                source_file_count: *count,
                recognition: Some("unrecognized"),
            });
        }
    }

    // manifest 行 sourceFileCount = 项目树内该语言直方图求和（不读内容，只汇总已扫计数）
    let manifest_count = insp
        .projects
        .iter_mut()
        .chain(insp.unsupported_areas.iter_mut())
        .filter(|a| matches!(a.evidence, InspectionEvidence::Manifest { .. }))
        .map(|a| {
            (
                a.relative_path.clone(),
                a.language.clone().unwrap_or_default(),
            )
        })
        .collect::<Vec<_>>();
    for (mpath, mlang) in &manifest_count {
        let total: u64 = dir_rows
            .iter()
            .filter(|(d, _)| path_within(d, mpath))
            .filter_map(|(_, h)| h.langs.get(mlang))
            .sum();
        let target = insp
            .projects
            .iter_mut()
            .chain(insp.unsupported_areas.iter_mut())
            .find(|a| {
                a.relative_path == *mpath
                    && matches!(a.evidence, InspectionEvidence::Manifest { .. })
                    && a.language.as_deref() == Some(mlang.as_str())
            });
        if let Some(area) = target {
            area.source_file_count = total;
        }
    }

    // 嵌套压制：manifest 项目只压同语言的直方图重复上报（backend 是 rust →
    // 不再报 backend/src 为 rust sourceOnly）；不同语言、尤其 unsupported /
    // unrecognized，即使罩在 manifest 树里也单独成行（unrecognized 行 language
    // 为 None，天然不会被压）。只压直方图行，不动其他 manifest 行。
    let suppress = |area: &InspectionArea| -> bool {
        let Some(lang) = area.language.as_deref() else {
            return false;
        };
        if !matches!(area.evidence, InspectionEvidence::ExtensionHistogram { .. }) {
            return false;
        }
        manifest_rows
            .iter()
            .any(|(mpath, mlang)| mlang == lang && path_within(&area.relative_path, mpath))
    };
    insp.source_only_areas.retain(|a| !suppress(a));
    insp.unsupported_areas.retain(|a| !suppress(a));

    Ok(insp)
}

/// 体检遍历：复用 SKIP_DIRS / MAX_WALK_DEPTH / MAX_ENTRIES 与点目录跳过，
// 与 walk_dir_for_projects 同防护但互不影响（共享扫描器语义零改动）。
fn inspect_walk(
    root: &Path,
    dir: &Path,
    depth: usize,
    dir_rows: &mut BTreeMap<String, InspectionDirHist>,
    entry_count: &mut usize,
) {
    if depth > MAX_WALK_DEPTH || *entry_count >= MAX_ENTRIES {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let entries: Vec<_> = entries.flatten().collect();

    let rel = pathdiff_or_fallback(dir, root);
    let hist = inspect_dir_histogram(dir, &entries);
    dir_rows.insert(rel, hist);

    for entry in &entries {
        *entry_count += 1;
        if *entry_count > MAX_ENTRIES {
            return;
        }
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if SKIP_DIRS.contains(&name) || name.starts_with('.') {
                continue;
            }
        }
        inspect_walk(root, &path, depth + 1, dir_rows, entry_count);
    }
}

/// 当前目录 manifest 检测 + 扩展名直方图。
/// manifest 匹配迭代 SUPPORTED/UNSUPPORTED_MANIFESTS 同表同优先序，
/// 不复用 detect_by_extensions 的"≥2 才报 + 只报最多语言"旧语义（那是共享面）。
fn inspect_dir_histogram(dir: &Path, entries: &[fs::DirEntry]) -> InspectionDirHist {
    let mut hist = InspectionDirHist {
        langs: BTreeMap::new(),
        unknown_exts: BTreeMap::new(),
        manifest: None,
    };

    'outer: for (table, supported_table) in
        [(SUPPORTED_MANIFESTS, true), (UNSUPPORTED_MANIFESTS, false)]
    {
        for (manifest_name, language) in table {
            let hit = if manifest_name.starts_with('.') {
                // .csproj/.sln —— 目录内有同名扩展名的文件即命中（对齐 detect_project_at）
                entries.iter().any(|e| {
                    e.path()
                        .extension()
                        .and_then(|x| x.to_str())
                        .map_or(false, |ext| {
                            manifest_name
                                .trim_start_matches('.')
                                .eq_ignore_ascii_case(&ext)
                        })
                })
            } else {
                dir.join(manifest_name).exists()
            };
            if hit {
                hist.manifest = Some((
                    manifest_name.to_string(),
                    *language,
                    supported_table && is_language_supported(language),
                ));
                break 'outer;
            }
        }
    }

    let manifest_names: HashSet<&str> = SUPPORTED_MANIFESTS
        .iter()
        .chain(UNSUPPORTED_MANIFESTS)
        .map(|(n, _)| *n)
        .collect();

    for entry in entries {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // manifest 文件已被 L1 消费，不进直方图（防止 Cargo.toml 被报成 unrecognized .toml）
        if manifest_names.contains(name) {
            continue;
        }
        // 无扩展名文件没有扩展名证据，跳过（不发明）
        let Some(ext) = path.extension().and_then(|x| x.to_str()) else {
            continue;
        };
        let dotted = format!(".{}", ext.to_lowercase());
        match SOURCE_EXTENSIONS
            .iter()
            .find(|(suffix, _)| *suffix == dotted)
        {
            Some((_, lang)) => *hist.langs.entry(lang.to_string()).or_insert(0) += 1,
            None => *hist.unknown_exts.entry(dotted).or_insert(0) += 1,
        }
    }
    hist
}

/// 语言 → 代表扩展名（直方图 evidence 用；扩展名表内该语言任一扩展名均可，
/// 取首个命中，保证与共享表一致而不是另写一份）。
fn extension_of_language(lang: &str) -> String {
    SOURCE_EXTENSIONS
        .iter()
        .find(|(_, l)| *l == lang)
        .map(|(ext, _)| ext.to_string())
        .unwrap_or_default()
}

/// child 是否位于 parent 的目录树内（路径段边界匹配；"." 罩全部）。
fn path_within(child: &str, parent: &str) -> bool {
    if parent == "." || parent == child {
        return true;
    }
    child.starts_with(&format!("{parent}/"))
}

// ── build_workspace_graph ────────────────────────────────────────────────

/// 构建工作区图：节点 + 边 + 摘要
pub fn build_workspace_graph(root: &Path, redact_root: bool) -> Result<WorkspaceGraph, String> {
    let root = root
        .canonicalize()
        .map_err(|e| format!("cannot canonicalize root: {}", e))?;

    let projects = scan_workspace_inventory(&root, redact_root)?;

    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut edge_id_counter: usize = 0;

    // 工作区根节点
    let root_id = "workspace:root".to_string();
    let root_path = redact_path(&root, &root, redact_root);
    nodes.push(WorkspaceNode {
        id: root_id.clone(),
        kind: "workspace".to_string(),
        label: root
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("workspace")
            .to_string(),
        path: root_path.clone(),
        relative_path: ".".to_string(),
        language: String::new(),
        supported: true,
        project_id: root_id.clone(),
        metadata: serde_json::json!({}),
    });

    // 为每个 inventory entry 创建节点。只有 manifest-backed entries 是 project；
    // source-only areas 只是目录线索，不能参与 projectCount 或项目间依赖。
    let mut project_node_ids: HashMap<String, String> = HashMap::new(); // relative_path → node_id
    for proj in &projects {
        let node_kind = if proj.is_manifest_backed {
            "project"
        } else {
            "source_area"
        };
        let node_id = format!("{}:{}", node_kind, proj.relative_path.replace('/', ":"));
        if proj.is_manifest_backed {
            project_node_ids.insert(proj.relative_path.clone(), node_id.clone());
        }

        nodes.push(WorkspaceNode {
            id: node_id.clone(),
            kind: node_kind.to_string(),
            label: proj.name.clone(),
            path: proj.path.clone(),
            relative_path: proj.relative_path.clone(),
            language: proj.language.clone(),
            supported: proj.supported,
            project_id: node_id.clone(),
            metadata: serde_json::json!({
                "manifest_file": proj.manifest_file,
                "manifestBacked": proj.is_manifest_backed,
                "sourceOnly": !proj.is_manifest_backed,
            }),
        });

        // workspace → project/source_area contains 边
        edge_id_counter += 1;
        edges.push(WorkspaceEdge {
            id: format!("edge:{}", edge_id_counter),
            kind: "contains".to_string(),
            source: root_id.clone(),
            target: node_id.clone(),
            confidence: 1.0,
            reason: "workspace-contains-project".to_string(),
            evidence: None,
        });
    }

    // 构建所有节点 ID 集合，用于 dangling edge 检查
    let all_node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    // 路径 → 节点 ID 映射（用于解析引用）
    let _path_to_node_id: HashMap<String, String> = nodes
        .iter()
        .map(|n| (n.relative_path.clone(), n.id.clone()))
        .collect();
    // 绝对路径 → 节点 ID
    let abs_path_to_node_id: HashMap<String, String> = {
        let root_str = root.to_str().unwrap_or("");
        nodes
            .iter()
            .filter(|n| n.path.starts_with('.') || n.path.starts_with(root_str))
            .map(|n| {
                // 还原绝对路径
                let abs = if n.path.starts_with('.') {
                    format!("{}/{}", root_str, n.path.trim_start_matches("./"))
                } else {
                    n.path.clone()
                };
                (abs, n.id.clone())
            })
            .collect()
    };

    // 解析项目间依赖。source-only area 不代表独立项目，跳过。
    for proj in &projects {
        if !proj.is_manifest_backed {
            continue;
        }
        let proj_dir = root.join(&proj.relative_path);
        let proj_node_id = project_node_ids
            .get(&proj.relative_path)
            .cloned()
            .unwrap_or_default();

        match proj.language.as_str() {
            "rust" => {
                resolve_rust_deps(
                    &proj_dir,
                    &root,
                    &proj_node_id,
                    &project_node_ids,
                    &all_node_ids,
                    &mut edges,
                    &mut edge_id_counter,
                    redact_root,
                );
            }
            "typescript" => {
                resolve_ts_deps(
                    &proj_dir,
                    &root,
                    &proj_node_id,
                    &project_node_ids,
                    &all_node_ids,
                    &abs_path_to_node_id,
                    &mut edges,
                    &mut edge_id_counter,
                    redact_root,
                );
            }
            _ => {}
        }
    }

    // 检测配置文件节点（Dockerfile、CI YAML、Makefile）
    detect_config_nodes(
        &root,
        &root,
        &root_id,
        &all_node_ids,
        &mut nodes,
        &mut edges,
        &mut edge_id_counter,
        redact_root,
    );

    // 检测脚本节点（.sh 文件）
    detect_script_nodes(
        &root,
        &root,
        &root_id,
        &all_node_ids,
        &mut nodes,
        &mut edges,
        &mut edge_id_counter,
        redact_root,
    );

    // 更新 all_node_ids / abs path index（新增 config/script 节点后）。
    // config_refs 需要解析 workflow -> script 这类文件节点引用，不能继续使用
    // 只包含 project 节点的早期索引。
    let all_node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    let abs_path_to_node_id: HashMap<String, String> = {
        let root_str = root.to_string_lossy().to_string();
        nodes
            .iter()
            .map(|n| {
                let abs = if n.path.starts_with('.') {
                    format!("{}/{}", root_str, n.path.trim_start_matches("./"))
                } else {
                    n.path.clone()
                };
                (abs, n.id.clone())
            })
            .collect()
    };

    // 为配置/脚本节点解析引用边（config_refs, script_refs）
    resolve_config_refs(
        &root,
        &root,
        &root_id,
        &nodes,
        &all_node_ids,
        &abs_path_to_node_id,
        &mut edges,
        &mut edge_id_counter,
        redact_root,
    );

    // 过滤 dangling edges：确保 source/target 都指向已有节点
    edges.retain(|e| all_node_ids.contains(&e.source) && all_node_ids.contains(&e.target));

    // 相邻项目边 adjacent_to（弱边）
    let project_ids: Vec<&str> = project_node_ids.values().map(|s| s.as_str()).collect();
    for i in 0..project_ids.len() {
        for j in (i + 1)..project_ids.len() {
            edge_id_counter += 1;
            edges.push(WorkspaceEdge {
                id: format!("edge:{}", edge_id_counter),
                kind: "adjacent_to".to_string(),
                source: project_ids[i].to_string(),
                target: project_ids[j].to_string(),
                confidence: 0.35,
                reason: "sibling-projects".to_string(),
                evidence: None,
            });
        }
    }

    // supported ↔ unsupported 边界 unsupported_boundary
    let supported_projects: Vec<&WorkspaceNode> = nodes
        .iter()
        .filter(|n| n.kind == "project" && n.supported)
        .collect();
    let unsupported_projects: Vec<&WorkspaceNode> = nodes
        .iter()
        .filter(|n| n.kind == "project" && !n.supported)
        .collect();

    for sup in &supported_projects {
        for unsup in &unsupported_projects {
            edge_id_counter += 1;
            edges.push(WorkspaceEdge {
                id: format!("edge:{}", edge_id_counter),
                kind: "unsupported_boundary".to_string(),
                source: sup.id.clone(),
                target: unsup.id.clone(),
                confidence: 0.45,
                reason: "supported-unsupported-boundary".to_string(),
                evidence: None,
            });
        }
    }

    // 最终 dangling edge 过滤
    let all_node_ids: HashSet<String> = nodes.iter().map(|n| n.id.clone()).collect();
    edges.retain(|e| all_node_ids.contains(&e.source) && all_node_ids.contains(&e.target));

    // 计算摘要
    let summary = compute_summary(&nodes, &edges, &project_node_ids);

    let cautions = vec![
        "static analysis only".to_string(),
        "no runtime proof".to_string(),
        "scripts executed: false".to_string(),
        "heuristic dependency detection".to_string(),
    ];

    Ok(WorkspaceGraph {
        schema_version: "workspace.graph.v1".to_string(),
        root: root_path,
        nodes,
        edges,
        summary,
        cautions,
        generated_from: serde_json::json!({
            "generator": "gitnexus-workspace-model",
            "version": env!("CARGO_PKG_VERSION"),
            "staticAnalysis": true,
            "runtimeVerified": false,
            "scriptsExecuted": false,
            "coverageVerified": false,
            "heuristic": true,
        }),
    })
}

// ── Rust dependency resolution ───────────────────────────────────────────

fn resolve_rust_deps(
    proj_dir: &Path,
    root: &Path,
    proj_node_id: &str,
    project_node_ids: &HashMap<String, String>,
    all_node_ids: &HashSet<String>,
    edges: &mut Vec<WorkspaceEdge>,
    edge_counter: &mut usize,
    _redact_root: bool,
) {
    let cargo_path = proj_dir.join("Cargo.toml");
    let content = match fs::read_to_string(&cargo_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let doc: toml::Value = match content.parse() {
        Ok(v) => v,
        Err(_) => return,
    };

    // workspace members
    if let Some(members) = doc
        .get("workspace")
        .and_then(|w| w.get("members"))
        .and_then(|m| m.as_array())
    {
        for member in members {
            if let Some(member_path) = member.as_str() {
                add_dep_edge(
                    proj_node_id,
                    member_path,
                    root,
                    project_node_ids,
                    all_node_ids,
                    edges,
                    edge_counter,
                    "depends_on",
                    0.85,
                    "cargo-workspace-member",
                );
            }
        }
    }

    // path dependencies in [dependencies]
    if let Some(deps) = doc.get("dependencies").and_then(|d| d.as_table()) {
        for (_name, val) in deps {
            if let Some(path) = val.get("path").and_then(|p| p.as_str()) {
                add_dep_edge(
                    proj_node_id,
                    path,
                    root,
                    project_node_ids,
                    all_node_ids,
                    edges,
                    edge_counter,
                    "depends_on",
                    0.85,
                    "cargo-path-dependency",
                );
            }
        }
    }

    // [dev-dependencies] path deps
    if let Some(deps) = doc.get("dev-dependencies").and_then(|d| d.as_table()) {
        for (_name, val) in deps {
            if let Some(path) = val.get("path").and_then(|p| p.as_str()) {
                add_dep_edge(
                    proj_node_id,
                    path,
                    root,
                    project_node_ids,
                    all_node_ids,
                    edges,
                    edge_counter,
                    "depends_on",
                    0.80,
                    "cargo-dev-path-dependency",
                );
            }
        }
    }
}

// ── TypeScript dependency resolution ─────────────────────────────────────

fn resolve_ts_deps(
    proj_dir: &Path,
    root: &Path,
    proj_node_id: &str,
    project_node_ids: &HashMap<String, String>,
    all_node_ids: &HashSet<String>,
    _abs_path_to_node_id: &HashMap<String, String>,
    edges: &mut Vec<WorkspaceEdge>,
    edge_counter: &mut usize,
    _redact_root: bool,
) {
    let pkg_path = proj_dir.join("package.json");
    let content = match fs::read_to_string(&pkg_path) {
        Ok(c) => c,
        Err(_) => return,
    };

    let pkg: serde_json::Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(_) => return,
    };

    // workspaces 字段
    if let Some(ws) = pkg.get("workspaces") {
        let patterns = extract_workspace_patterns(ws);
        for pattern in patterns {
            add_dep_edge(
                proj_node_id,
                &pattern,
                root,
                project_node_ids,
                all_node_ids,
                edges,
                edge_counter,
                "depends_on",
                0.85,
                "npm-workspace-pattern",
            );
        }
    }

    // file: dependencies
    for field in &["dependencies", "devDependencies"] {
        if let Some(deps) = pkg.get(field).and_then(|d| d.as_object()) {
            for (_name, val) in deps {
                if let Some(version) = val.as_str() {
                    if version.starts_with("file:") {
                        let rel = version.trim_start_matches("file:").trim_start_matches('/');
                        add_dep_edge(
                            proj_node_id,
                            rel,
                            root,
                            project_node_ids,
                            all_node_ids,
                            edges,
                            edge_counter,
                            "depends_on",
                            0.85,
                            "npm-file-dependency",
                        );
                    }
                }
            }
        }
    }

    // tsconfig.json paths → imports 边
    let tsconfig_path = proj_dir.join("tsconfig.json");
    if let Ok(tscontent) = fs::read_to_string(&tsconfig_path) {
        // 简单解析：不用完整 JSON parser 处理 comment（TS config 可能有注释）
        // 仅尝试 serde_json 解析
        if let Ok(tsconfig) = serde_json::from_str::<serde_json::Value>(&tscontent) {
            if let Some(paths) = tsconfig
                .get("compilerOptions")
                .and_then(|co| co.get("paths"))
                .and_then(|p| p.as_object())
            {
                for (_alias, targets) in paths {
                    if let Some(arr) = targets.as_array() {
                        for target in arr {
                            if let Some(t) = target.as_str() {
                                // 提取顶级路径段
                                let clean = t.trim_start_matches(".*/");
                                if let Some(first) = clean.split('/').next() {
                                    if !first.is_empty() && first != "*" {
                                        // 查找是否有对应项目节点
                                        let candidate_rel = first.to_string();
                                        if let Some(target_id) =
                                            project_node_ids.get(&candidate_rel)
                                        {
                                            *edge_counter += 1;
                                            let edge = WorkspaceEdge {
                                                id: format!("edge:{}", edge_counter),
                                                kind: "imports".to_string(),
                                                source: proj_node_id.to_string(),
                                                target: target_id.clone(),
                                                confidence: 0.65,
                                                reason: "tsconfig-path-reference".to_string(),
                                                evidence: Some(serde_json::json!({
                                                    "alias": _alias,
                                                    "target": t,
                                                })),
                                            };
                                            // 仅添加如果 source 和 target 都存在
                                            if all_node_ids.contains(proj_node_id)
                                                && all_node_ids.contains(target_id)
                                            {
                                                edges.push(edge);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn extract_workspace_patterns(ws: &serde_json::Value) -> Vec<String> {
    let mut patterns = Vec::new();
    match ws {
        serde_json::Value::Array(arr) => {
            for v in arr {
                if let Some(s) = v.as_str() {
                    // "packages/*" → "packages"
                    if let Some(dir) = s.split('/').next() {
                        if !dir.contains('*') {
                            patterns.push(dir.to_string());
                        }
                    }
                    // "*" → 跳过（匹配所有子目录，太宽泛）
                }
            }
        }
        serde_json::Value::Object(obj) => {
            // npm workspaces config: { "packages": [...] }
            if let Some(packages) = obj.get("packages").and_then(|p| p.as_array()) {
                for v in packages {
                    if let Some(s) = v.as_str() {
                        if let Some(dir) = s.split('/').next() {
                            if !dir.contains('*') {
                                patterns.push(dir.to_string());
                            }
                        }
                    }
                }
            }
        }
        _ => {}
    }
    patterns
}

// ── Add dependency edge helper ───────────────────────────────────────────

fn add_dep_edge(
    source_id: &str,
    rel_path: &str,
    _root: &Path,
    project_node_ids: &HashMap<String, String>,
    all_node_ids: &HashSet<String>,
    edges: &mut Vec<WorkspaceEdge>,
    edge_counter: &mut usize,
    kind: &str,
    confidence: f64,
    reason: &str,
) {
    // 规范化路径：去掉前导 ./
    let clean = rel_path.trim_start_matches("./").trim_start_matches("../");
    if clean.is_empty() {
        return;
    }

    // 精确匹配 relative_path
    if let Some(target_id) = project_node_ids.get(clean) {
        if all_node_ids.contains(source_id) && all_node_ids.contains(target_id) {
            *edge_counter += 1;
            edges.push(WorkspaceEdge {
                id: format!("edge:{}", edge_counter),
                kind: kind.to_string(),
                source: source_id.to_string(),
                target: target_id.clone(),
                confidence,
                reason: reason.to_string(),
                evidence: None,
            });
        }
        return;
    }

    // 尝试前缀匹配（对于 glob pattern 的部分匹配）
    for (rp, nid) in project_node_ids {
        if rp.starts_with(clean) || clean.starts_with(rp.as_str()) {
            if all_node_ids.contains(source_id) && all_node_ids.contains(nid) {
                *edge_counter += 1;
                edges.push(WorkspaceEdge {
                    id: format!("edge:{}", edge_counter),
                    kind: kind.to_string(),
                    source: source_id.to_string(),
                    target: nid.clone(),
                    confidence: confidence * 0.9, // 前缀匹配略降置信度
                    reason: format!("{}-prefix-match", reason),
                    evidence: Some(serde_json::json!({
                        "raw_path": rel_path,
                    })),
                });
                return; // 只匹配一个
            }
        }
    }
}

// ── Config node detection ────────────────────────────────────────────────

fn detect_config_nodes(
    dir: &Path,
    root: &Path,
    root_id: &str,
    all_node_ids: &HashSet<String>,
    nodes: &mut Vec<WorkspaceNode>,
    edges: &mut Vec<WorkspaceEdge>,
    edge_counter: &mut usize,
    redact_root: bool,
) {
    // Dockerfile
    let dockerfile = dir.join("Dockerfile");
    if dockerfile.exists() {
        add_file_node(
            "config",
            "Dockerfile",
            &dockerfile,
            root,
            root_id,
            all_node_ids,
            nodes,
            edges,
            edge_counter,
            redact_root,
        );
    }

    // Makefile
    let makefile = dir.join("Makefile");
    if makefile.exists() {
        add_file_node(
            "config",
            "Makefile",
            &makefile,
            root,
            root_id,
            all_node_ids,
            nodes,
            edges,
            edge_counter,
            redact_root,
        );
    }

    // CI YAML (.github/workflows/*.yml)
    let workflows = dir.join(".github").join("workflows");
    if workflows.is_dir() {
        if let Ok(entries) = fs::read_dir(&workflows) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "yml" || ext == "yaml" {
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");
                        add_file_node(
                            "workflow",
                            name,
                            &path,
                            root,
                            root_id,
                            all_node_ids,
                            nodes,
                            edges,
                            edge_counter,
                            redact_root,
                        );
                    }
                }
            }
        }
    }
}

fn detect_script_nodes(
    dir: &Path,
    root: &Path,
    root_id: &str,
    all_node_ids: &HashSet<String>,
    nodes: &mut Vec<WorkspaceNode>,
    edges: &mut Vec<WorkspaceEdge>,
    edge_counter: &mut usize,
    redact_root: bool,
) {
    // 根目录 scripts/ 目录
    let scripts_dir = dir.join("scripts");
    if scripts_dir.is_dir() {
        if let Ok(entries) = fs::read_dir(&scripts_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if let Some(ext) = path.extension() {
                    if ext == "sh" {
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");
                        add_file_node(
                            "script",
                            name,
                            &path,
                            root,
                            root_id,
                            all_node_ids,
                            nodes,
                            edges,
                            edge_counter,
                            redact_root,
                        );
                    }
                }
            }
        }
    }

    // 根目录 .sh 文件
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() {
                if let Some(ext) = path.extension() {
                    if ext == "sh" {
                        let name = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("unknown");
                        add_file_node(
                            "script",
                            name,
                            &path,
                            root,
                            root_id,
                            all_node_ids,
                            nodes,
                            edges,
                            edge_counter,
                            redact_root,
                        );
                    }
                }
            }
        }
    }
}

fn add_file_node(
    kind: &str,
    name: &str,
    abs_path: &Path,
    root: &Path,
    root_id: &str,
    _all_node_ids: &HashSet<String>,
    nodes: &mut Vec<WorkspaceNode>,
    edges: &mut Vec<WorkspaceEdge>,
    edge_counter: &mut usize,
    redact_root: bool,
) {
    let relative = pathdiff_or_fallback(abs_path, root);
    let node_id = format!("{}:{}", kind, relative.replace('/', ":"));

    nodes.push(WorkspaceNode {
        id: node_id.clone(),
        kind: kind.to_string(),
        label: name.to_string(),
        path: redact_path(abs_path, root, redact_root),
        relative_path: relative.clone(),
        language: String::new(),
        supported: true,
        project_id: node_id.clone(),
        metadata: serde_json::json!({}),
    });

    *edge_counter += 1;
    edges.push(WorkspaceEdge {
        id: format!("edge:{}", edge_counter),
        kind: "contains".to_string(),
        source: root_id.to_string(),
        target: node_id.clone(),
        confidence: 1.0,
        reason: format!("workspace-contains-{}", kind),
        evidence: None,
    });
}

// ── Config/script reference resolution ───────────────────────────────────

/// 从配置/脚本文件内容中提取本地路径引用，创建引用边
fn resolve_config_refs(
    _dir: &Path,
    root: &Path,
    _root_id: &str,
    nodes: &[WorkspaceNode],
    all_node_ids: &HashSet<String>,
    abs_path_to_node_id: &HashMap<String, String>,
    edges: &mut Vec<WorkspaceEdge>,
    edge_counter: &mut usize,
    _redact_root: bool,
) {
    // 从文件内容提取引用，查找对应节点创建边
    let root_str = match root.to_str() {
        Some(s) => s,
        None => return,
    };

    for node in nodes {
        if node.kind != "config" && node.kind != "workflow" && node.kind != "script" {
            continue;
        }

        // 从 relative_path 还原绝对路径
        let abs = if node.relative_path.starts_with('/') {
            node.relative_path.clone()
        } else {
            format!("{}/{}", root_str, node.relative_path)
        };
        let content = match fs::read_to_string(&abs) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let refs = extract_local_refs(&content);

        for r in refs {
            // 规范化引用路径
            let clean = r.trim_start_matches("./").trim_start_matches('/');
            let abs_ref = format!("{}/{}", root_str, clean);

            // 查找对应节点
            let target_id = abs_path_to_node_id
                .get(&abs_ref)
                .or_else(|| abs_path_to_node_id.get(&format!("{}/", abs_ref)));

            if let Some(tid) = target_id {
                if all_node_ids.contains(&node.id) && all_node_ids.contains(tid) {
                    *edge_counter += 1;
                    let edge_kind = if node.kind == "script" {
                        "script_refs"
                    } else {
                        "config_refs"
                    };
                    edges.push(WorkspaceEdge {
                        id: format!("edge:{}", edge_counter),
                        kind: edge_kind.to_string(),
                        source: node.id.clone(),
                        target: tid.clone(),
                        confidence: 0.75,
                        reason: format!("{}-local-ref", node.kind),
                        evidence: Some(serde_json::json!({
                            "raw_ref": r,
                        })),
                    });
                }
            }
        }
    }
}

/// 从文件内容中提取本地路径引用（最小化解析）
fn extract_local_refs(content: &str) -> Vec<String> {
    let mut refs = Vec::new();
    let mut seen = HashSet::new();

    for line in content.lines() {
        let line = line.trim();

        // Dockerfile: COPY <src> <dest>
        if line.starts_with("COPY ") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let src = parts[1];
                if !src.starts_with('/') && !src.contains(':') && src.contains('/') {
                    if seen.insert(src.to_string()) {
                        refs.push(src.to_string());
                    }
                }
            }
        }

        // Makefile / scripts / CI: 引用 ./scripts/xxx、scripts/xxx 或 path/to。
        for word in line.split_whitespace() {
            let clean = word
                .trim_end_matches(';')
                .trim_end_matches('&')
                .trim_end_matches('|')
                .trim_end_matches('"')
                .trim_end_matches('\'')
                .trim_end_matches(',');
            let is_local_path = clean.contains('/')
                && !clean.starts_with('/')
                && !clean.starts_with('-')
                && !clean.contains(':')
                && !clean.contains("${")
                && clean.len() > 3;
            if is_local_path && seen.insert(clean.to_string()) {
                refs.push(clean.to_string());
            }
        }

        // Shell snippets: cd rust-core / cd ../pkg / cd "ts-ui".
        let parts: Vec<&str> = line.split_whitespace().collect();
        for pair in parts.windows(2) {
            if pair[0] == "cd" {
                let clean = pair[1]
                    .trim_matches('"')
                    .trim_matches('\'')
                    .trim_end_matches(';')
                    .trim_end_matches('&')
                    .trim_end_matches('|');
                if !clean.is_empty()
                    && clean != "."
                    && clean != ".."
                    && !clean.starts_with('/')
                    && !clean.starts_with('-')
                    && !clean.contains('$')
                    && seen.insert(clean.to_string())
                {
                    refs.push(clean.to_string());
                }
            }
        }
    }

    refs
}

// ── Summary computation ──────────────────────────────────────────────────

fn compute_summary(
    nodes: &[WorkspaceNode],
    edges: &[WorkspaceEdge],
    project_node_ids: &HashMap<String, String>,
) -> WorkspaceGraphSummary {
    let node_count = nodes.len();
    let edge_count = edges.len();
    let project_count = project_node_ids.len();

    // 跨项目边计数
    let cross_project_edge_count = edges
        .iter()
        .filter(|e| {
            matches!(
                e.kind.as_str(),
                "depends_on" | "imports" | "script_refs" | "config_refs"
            ) && !e.kind.is_empty()
        })
        .count();

    let unsupported_boundary_count = edges
        .iter()
        .filter(|e| e.kind == "unsupported_boundary")
        .count();

    // top connected projects
    let mut connection_counts: HashMap<&str, usize> = HashMap::new();
    for e in edges {
        if matches!(e.kind.as_str(), "depends_on" | "imports") {
            *connection_counts.entry(&e.source).or_insert(0) += 1;
            *connection_counts.entry(&e.target).or_insert(0) += 1;
        }
    }
    let mut top_connected: Vec<(String, usize)> = connection_counts
        .iter()
        .map(|(k, v)| (k.to_string(), *v))
        .collect();
    top_connected.sort_by(|a, b| b.1.cmp(&a.1));
    top_connected.truncate(5);
    let top_connected_projects: Vec<serde_json::Value> = top_connected
        .iter()
        .map(|(id, count)| {
            serde_json::json!({
                "node_id": id,
                "connection_count": count,
            })
        })
        .collect();

    // bridge scripts / configs
    let bridge_scripts: Vec<serde_json::Value> = edges
        .iter()
        .filter(|e| e.kind == "script_refs")
        .map(|e| {
            serde_json::json!({
                "source": e.source,
                "target": e.target,
            })
        })
        .collect();

    let bridge_configs: Vec<serde_json::Value> = edges
        .iter()
        .filter(|e| e.kind == "config_refs")
        .map(|e| {
            serde_json::json!({
                "source": e.source,
                "target": e.target,
            })
        })
        .collect();

    WorkspaceGraphSummary {
        node_count,
        edge_count,
        project_count,
        cross_project_edge_count,
        unsupported_boundary_count,
        top_connected_projects,
        bridge_scripts,
        bridge_configs,
    }
}

// ── Path utilities ───────────────────────────────────────────────────────

fn redact_path(path: &Path, root: &Path, redact: bool) -> String {
    if !redact {
        return path.to_str().unwrap_or("?").to_string();
    }
    let root_str = root.to_str().unwrap_or("");
    let path_str = path.to_str().unwrap_or("?");
    if path_str.starts_with(root_str) {
        format!(".{}", &path_str[root_str.len()..])
    } else {
        path_str.to_string()
    }
}

/// 简易 pathdiff：计算 dir 相对于 root 的相对路径
fn pathdiff_or_fallback(dir: &Path, root: &Path) -> String {
    let dir_str = dir.to_str().unwrap_or("");
    let root_str = root.to_str().unwrap_or("");
    if dir_str.starts_with(root_str) {
        let rel = &dir_str[root_str.len()..];
        let rel = rel.trim_start_matches('/');
        if rel.is_empty() {
            ".".to_string()
        } else {
            rel.to_string()
        }
    } else {
        dir_str.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn test_is_language_supported() {
        assert!(is_language_supported("rust"));
        assert!(is_language_supported("typescript"));
        assert!(is_language_supported("python"));
        assert!(is_language_supported("shell"));
        assert!(!is_language_supported("go"));
        assert!(!is_language_supported("java"));
    }

    #[test]
    fn test_pathdiff_or_fallback() {
        let root = Path::new("/Users/foo/workspace");
        let dir = Path::new("/Users/foo/workspace/project-a");
        assert_eq!(pathdiff_or_fallback(dir, root), "project-a");

        let root2 = Path::new("/Users/foo/workspace");
        let dir2 = Path::new("/Users/foo/workspace");
        assert_eq!(pathdiff_or_fallback(dir2, root2), ".");
    }

    #[test]
    fn test_redact_path() {
        let root = Path::new("/Users/foo/workspace");
        let dir = Path::new("/Users/foo/workspace/project-a");
        assert_eq!(redact_path(dir, root, true), "./project-a");
        assert_eq!(
            redact_path(dir, root, false),
            "/Users/foo/workspace/project-a"
        );
    }

    #[test]
    fn test_extract_local_refs() {
        let content = "COPY ./src /app\nRUN bash ./scripts/build.sh\nrun: bash scripts/build-core.sh\ncd rust-core && cargo build\n";
        let refs = extract_local_refs(content);
        assert!(refs.contains(&"./src".to_string()));
        assert!(refs.contains(&"./scripts/build.sh".to_string()));
        assert!(refs.contains(&"scripts/build-core.sh".to_string()));
        assert!(refs.contains(&"rust-core".to_string()));
    }

    // ── inspect_workspace_inventory 单元测试（多语言卡 2）──────────────────
    // 手工 tempdir（本 crate 无 tempfile dev-dep，Cargo.toml 不在 write set）

    static INSPECT_COUNTER: AtomicU32 = AtomicU32::new(0);

    fn inspect_tmp_root(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "cls-inspect-{}-{}-{}",
            std::process::id(),
            tag,
            INSPECT_COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_file(root: &Path, rel: &str, content: &str) {
        let p = root.join(rel);
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(p, content).unwrap();
    }

    fn touch(root: &Path, rel: &str) {
        write_file(root, rel, "");
    }

    /// 卡 2 TDD 基准树：与 fixtures/mixed/inspect-smoke 同构。
    fn build_inspect_tree(root: &Path) {
        write_file(
            root,
            "backend/Cargo.toml",
            "[package]\nname = \"backend\"\n",
        );
        touch(root, "backend/src/main.rs");
        touch(root, "backend/src/lib.rs");
        touch(root, "backend/legacy/Foo.java");
        touch(root, "scripts/tools/a.py");
        touch(root, "scripts/tools/b.py");
        touch(root, "scripts/tools/c.py");
        touch(root, "scripts/tools/run.sh");
        touch(root, "scripts/tools/util.sh");
        touch(root, "legacy/Gadget.java");
        touch(root, "legacy/Helper.java");
        touch(root, "gen/out.xyzfoo");
        touch(root, "gen/data.xyzfoo");
        touch(root, "solo/only.py");
    }

    #[test]
    fn inspect_one_row_per_language_in_same_directory() {
        let root = inspect_tmp_root("rows");
        build_inspect_tree(&root);
        let insp = inspect_workspace_inventory(&root).unwrap();

        // 同目录多语言 → 两行，count 各自正确（一行 = (relativePath, language)）
        let py = insp
            .source_only_areas
            .iter()
            .find(|a| a.relative_path == "scripts/tools" && a.language.as_deref() == Some("python"))
            .expect("python 行必须在场");
        assert_eq!(py.source_file_count, 3);
        assert_eq!(py.confidence, "medium");
        match &py.evidence {
            InspectionEvidence::ExtensionHistogram { extension, count } => {
                assert_eq!(extension, ".py");
                assert_eq!(*count, 3);
            }
            other => panic!("evidence 应为直方图: {other:?}"),
        }
        let sh = insp
            .source_only_areas
            .iter()
            .find(|a| a.relative_path == "scripts/tools" && a.language.as_deref() == Some("shell"))
            .expect("shell 行必须在场");
        assert_eq!(sh.source_file_count, 2);

        // 1 个 .py 不报（L3 ≥2，别和 java ≥1 混淆）
        assert!(!insp
            .source_only_areas
            .iter()
            .any(|a| a.relative_path == "solo"));

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn inspect_manifest_project_row_and_nested_suppression() {
        let root = inspect_tmp_root("nest");
        build_inspect_tree(&root);
        let insp = inspect_workspace_inventory(&root).unwrap();

        // L1 manifest 行：certain + manifest evidence + name
        let backend = insp
            .projects
            .iter()
            .find(|a| a.relative_path == "backend")
            .expect("backend manifest 行必须在场");
        assert_eq!(backend.language.as_deref(), Some("rust"));
        assert_eq!(backend.confidence, "certain");
        assert_eq!(backend.name.as_deref(), Some("backend"));
        match &backend.evidence {
            InspectionEvidence::Manifest { file } => assert_eq!(file, "Cargo.toml"),
            other => panic!("evidence 应为 manifest: {other:?}"),
        }
        // sourceFileCount = 项目树内该语言直方图求和（src 两个 .rs）
        assert_eq!(backend.source_file_count, 2);

        // 嵌套压制：只压同语言 L3 —— backend/src 的 rust sourceOnly 行不报
        assert!(!insp
            .source_only_areas
            .iter()
            .any(|a| a.relative_path == "backend/src"));
        // 不同语言不被吞：manifest 树内嵌 1 个 .java 也必须单独成行
        let nested_java = insp
            .unsupported_areas
            .iter()
            .find(|a| a.relative_path == "backend/legacy")
            .expect("manifest 树内 java 行必须在场");
        assert_eq!(nested_java.language.as_deref(), Some("java"));
        assert_eq!(nested_java.source_file_count, 1);

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn inspect_unsupported_thresholds_and_recognition_tiers() {
        let root = inspect_tmp_root("unsup");
        build_inspect_tree(&root);
        let insp = inspect_workspace_inventory(&root).unwrap();

        // known-unsupported：≥1 就报（legacy 两个 .java）
        let java = insp
            .unsupported_areas
            .iter()
            .find(|a| a.relative_path == "legacy")
            .expect("java 区行必须在场");
        assert_eq!(java.language.as_deref(), Some("java"));
        assert_eq!(java.confidence, "medium");
        assert_eq!(java.recognition, Some("known-unsupported"));
        assert_eq!(java.source_file_count, 2);

        // unrecognized：language 省略（None）、confidence low、recognition 必有
        let gen = insp
            .unsupported_areas
            .iter()
            .find(|a| a.relative_path == "gen")
            .expect("unrecognized 区行必须在场");
        assert_eq!(gen.language, None, "unrecognized 行必须省略 language");
        assert_eq!(gen.confidence, "low");
        assert_eq!(gen.recognition, Some("unrecognized"));
        match &gen.evidence {
            InspectionEvidence::ExtensionHistogram { extension, count } => {
                assert_eq!(extension, ".xyzfoo");
                assert_eq!(*count, 2);
            }
            other => panic!("evidence 应为直方图: {other:?}"),
        }

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn inspect_reuses_skip_dirs_protection() {
        let root = inspect_tmp_root("skip");
        // 若不复用 SKIP_DIRS，node_modules 会被扫成 typescript 区（扫描爆炸）
        touch(&root, "web/node_modules/pkg/index.ts");
        touch(&root, "web/node_modules/pkg/util.ts");
        let insp = inspect_workspace_inventory(&root).unwrap();
        assert!(
            !insp
                .source_only_areas
                .iter()
                .any(|a| a.relative_path.contains("node_modules")),
            "node_modules 必须被跳过: {:?}",
            insp.source_only_areas
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn inspect_unsupported_manifest_lands_in_unsupported_bucket() {
        let root = inspect_tmp_root("gomanifest");
        // go.mod 是 manifest 背书但语言不可分析 → certain 行进 unsupported 桶；
        // 树内 .go 直方图行同语言被压，不重复上报
        write_file(&root, "svc/go.mod", "module svc\n");
        touch(&root, "svc/main.go");
        touch(&root, "svc/util.go");
        let insp = inspect_workspace_inventory(&root).unwrap();

        let go = insp
            .unsupported_areas
            .iter()
            .find(|a| a.relative_path == "svc")
            .expect("go manifest 行必须在场");
        assert_eq!(go.language.as_deref(), Some("go"));
        assert_eq!(go.confidence, "certain", "manifest 背书恒 certain");
        assert_eq!(go.recognition, Some("known-unsupported"));
        assert_eq!(go.source_file_count, 2);
        match &go.evidence {
            InspectionEvidence::Manifest { file } => assert_eq!(file, "go.mod"),
            other => panic!("evidence 应为 manifest: {other:?}"),
        }
        // 不可能出现 svc 的 go sourceOnly 行（可分析桶永远不会有 go）
        assert!(!insp
            .source_only_areas
            .iter()
            .any(|a| a.relative_path == "svc"));

        let _ = fs::remove_dir_all(&root);
    }
}
