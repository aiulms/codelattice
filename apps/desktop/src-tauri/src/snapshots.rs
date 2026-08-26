// snapshots —— 快照库读取与发布（dev 阶段 + P0-C 规则）。
// 读取：CODELATTICE_SNAP_DIR 或仓库 fixtures/webui-snapshots（只读基线）。
// 发布：CODELATTICE_PUBLISH_DIR 或 target/workbench-snapshots（Desktop Analyzer 产物）。
// P0-C：immutable publish（temp + atomic rename）、pin、cleanup（保留 pinned +
// 最近 N 个，只清理发布目录，绝不触碰 fixtures 基线）。

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

pub fn snapshot_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CODELATTICE_SNAP_DIR") {
        return PathBuf::from(dir);
    }
    // src-tauri 位于 apps/desktop/src-tauri → 仓库根 = ../../../
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.join("../../../fixtures/webui-snapshots")
}

/// Desktop Analyzer 发布目录（独立于 fixtures 基线）。
pub fn publish_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("CODELATTICE_PUBLISH_DIR") {
        return PathBuf::from(dir);
    }
    let here = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    here.join("../../../target/workbench-snapshots")
}

fn entries() -> Vec<(PathBuf, Value)> {
    let mut out = Vec::new();
    for dir in [snapshot_dir(), publish_dir()] {
        if !dir.is_dir() {
            continue;
        }
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for entry in rd.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Some(meta) = meta_of(&path) {
                out.push((path, meta));
            }
        }
    }
    out
}

pub fn list_snapshots() -> Result<Vec<Value>, String> {
    let mut metas: Vec<Value> = entries().into_iter().map(|(_, m)| m).collect();
    metas.sort_by(|a, b| {
        let at = a.get("createdAt").and_then(Value::as_str).unwrap_or("");
        let bt = b.get("createdAt").and_then(Value::as_str).unwrap_or("");
        bt.cmp(at)
    });
    Ok(metas)
}

/// 标题要显示项目名。优先用快照自述的 root 目录名；脱敏占位符（`<...>`）
/// 和空值都不算项目名，此时退回文件名并去掉 `.snapshot` 后缀，避免出现
/// `shell-portable-smoke.snapshot` 或裸 job id。
fn root_label_of(data: &Value, path: &Path) -> String {
    let file_label = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("?")
        .trim_end_matches(".snapshot");
    let root = data.get("root").and_then(Value::as_str).unwrap_or("");
    if !root.is_empty() && !root.starts_with('<') {
        if let Some(name) = Path::new(root)
            .file_name()
            .and_then(|s| s.to_str())
            .filter(|s| !s.is_empty())
        {
            return name.to_string();
        }
    }
    file_label.to_string()
}

fn meta_of(path: &Path) -> Option<Value> {
    let data: Value = serde_json::from_str(&fs::read_to_string(path).ok()?).ok()?;
    let summary = data.get("summary").cloned().unwrap_or(Value::Null);
    let generated_at = data
        .get("generatedAt")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    // 语言同样按快照自述取：webui 快照写在 summary.language，CLI 原始产物写在顶层。
    let language = data
        .get("summary")
        .and_then(|s| s.get("language"))
        .and_then(Value::as_str)
        .or_else(|| data.get("language").and_then(Value::as_str))
        .unwrap_or("rust");
    Some(serde_json::json!({
        "id": path.file_stem().and_then(|s| s.to_str()).unwrap_or("?"),
        "rootLabel": root_label_of(&data, path),
        "language": language,
        "createdAt": generated_at,
        "summary": summary,
    }))
}

pub fn load_snapshot(snapshot_id: &str) -> Result<Value, String> {
    // id 白名单：只允许字母数字与常见分隔符，防路径穿越
    if !snapshot_id
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err("invalid snapshot id".to_string());
    }
    for dir in [snapshot_dir(), publish_dir()] {
        let path = dir.join(format!("{snapshot_id}.json"));
        if path.is_file() {
            return serde_json::from_str(&fs::read_to_string(&path).map_err(|e| e.to_string())?)
                .map_err(|e| format!("snapshot corrupt: {e}"));
        }
    }
    Err(format!("snapshot not found: {snapshot_id}"))
}

/// 原子发布（P0-C §8.2 / 验收 24）：temp + rename，失败不留下半写文件。
/// （Desktop Analyzer 当前经 supervisor 直接发布；本 API 为 snapshot store
/// 公共入口，测试覆盖其语义。）
#[allow(dead_code)]
pub fn publish_snapshot(snapshot_id: &str, payload: &[u8]) -> Result<PathBuf, String> {
    if snapshot_id.is_empty() {
        return Err("empty snapshot id".to_string());
    }
    let dir = publish_dir();
    fs::create_dir_all(&dir).map_err(|e| format!("mkdir publish dir failed: {e}"))?;
    let final_path = dir.join(format!("{snapshot_id}.json"));
    let tmp = dir.join(format!(".{snapshot_id}.tmp"));
    fs::write(&tmp, payload).map_err(|e| format!("write temp failed: {e}"))?;
    fs::rename(&tmp, &final_path).map_err(|e| format!("atomic rename failed: {e}"))?;
    Ok(final_path)
}

/// P0-C cleanup：只清理发布目录（不触碰 fixtures）；保留 pinned 与最近 max_keep 个。
pub fn cleanup_published(max_keep: usize, pinned: &[String]) -> Result<usize, String> {
    let dir = publish_dir();
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut all: Vec<(PathBuf, String)> = Vec::new(); // (path, createdAt)
    for entry in fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let created = meta_of(&path)
            .and_then(|m| {
                m.get("createdAt")
                    .and_then(Value::as_str)
                    .map(str::to_string)
            })
            .unwrap_or_default();
        all.push((path, created));
    }
    all.sort_by(|a, b| b.1.cmp(&a.1)); // 新的在前
    let mut removed = 0;
    for (i, (path, _)) in all.iter().enumerate() {
        let id = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        if pinned.contains(&id) {
            continue;
        }
        if i < max_keep {
            continue;
        }
        if fs::remove_file(path).is_ok() {
            removed += 1;
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::Mutex as StdMutex;

    // 环境变量共享 → 测试必须串行
    static ENV_LOCK: StdMutex<()> = StdMutex::new(());
    static COUNTER: AtomicU32 = AtomicU32::new(0);

    fn tmp_publish_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "cls-snapshot-store-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn payload(ts: &str) -> Vec<u8> {
        format!(
            r#"{{"generatedAt": "{}", "graph": {{"nodes": [], "edges": [], "summary": {{}}}}, "limitations": {{"notes": []}}}}"#,
            ts
        )
        .into_bytes()
    }

    #[test]
    fn root_label_prefers_project_name_over_file_name() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tmp_publish_dir();
        std::env::set_var("CODELATTICE_PUBLISH_DIR", &dir);
        let body = br#"{"generatedAt":"2026-08-24T00:00:00+00:00","root":"/Users/me/Desktop/open-nwe","language":"python","graph":{"nodes":[],"edges":[],"summary":{}}}"#;
        publish_snapshot("job-1756000000", body).unwrap();
        let metas = list_snapshots().unwrap();
        let meta = metas
            .iter()
            .find(|m| m.get("id").and_then(Value::as_str) == Some("job-1756000000"))
            .expect("published snapshot must be listed");
        assert_eq!(
            meta.get("rootLabel").and_then(Value::as_str),
            Some("open-nwe"),
            "标题要显示项目名，不是 job id"
        );
        assert_eq!(
            meta.get("language").and_then(Value::as_str),
            Some("python"),
            "语言取快照自述，不硬编码 rust"
        );
        let _ = fs::remove_dir_all(&dir);
        std::env::remove_var("CODELATTICE_PUBLISH_DIR");
    }

    #[test]
    fn root_label_falls_back_to_clean_file_name() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tmp_publish_dir();
        std::env::set_var("CODELATTICE_PUBLISH_DIR", &dir);
        // fixtures 基线做过 root 脱敏，只剩占位符；此时退回文件名并去掉 .snapshot 后缀。
        let body = br#"{"generatedAt":"2026-08-24T00:00:00+00:00","root":"<redacted-root>","summary":{"language":"shell"},"graph":{"nodes":[],"edges":[],"summary":{}}}"#;
        publish_snapshot("shell-portable-smoke.snapshot", body).unwrap();
        let metas = list_snapshots().unwrap();
        let meta = metas
            .iter()
            .find(|m| m.get("id").and_then(Value::as_str) == Some("shell-portable-smoke.snapshot"))
            .expect("published snapshot must be listed");
        assert_eq!(
            meta.get("rootLabel").and_then(Value::as_str),
            Some("shell-portable-smoke")
        );
        let _ = fs::remove_dir_all(&dir);
        std::env::remove_var("CODELATTICE_PUBLISH_DIR");
    }

    #[test]
    fn publish_is_atomic_and_listable() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tmp_publish_dir();
        // 用 env 注入隔离目录
        std::env::set_var("CODELATTICE_PUBLISH_DIR", &dir);
        let path = publish_snapshot("snap-a", &payload("2026-08-05T00:00:00+00:00")).unwrap();
        assert!(path.is_file());
        assert!(load_snapshot("snap-a").is_ok());
        // 没有 .tmp 残留
        assert!(!dir.join(".snap-a.tmp").exists());
        let _ = fs::remove_dir_all(&dir);
        std::env::remove_var("CODELATTICE_PUBLISH_DIR");
    }

    #[test]
    fn cleanup_keeps_pinned_and_recent() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tmp_publish_dir();
        std::env::set_var("CODELATTICE_PUBLISH_DIR", &dir);
        for (id, ts) in [
            ("snap-1", "2026-08-05T00:00:00+00:00"),
            ("snap-2", "2026-08-05T01:00:00+00:00"),
            ("snap-3", "2026-08-05T02:00:00+00:00"),
            ("snap-4", "2026-08-05T03:00:00+00:00"),
        ] {
            publish_snapshot(id, &payload(ts)).unwrap();
        }
        // 保留 pinned=["snap-2"] + 最近 2 个（snap-4, snap-3）→ snap-1 被清理
        let removed = cleanup_published(2, &["snap-2".to_string()]).unwrap();
        assert_eq!(removed, 1);
        assert!(load_snapshot("snap-1").is_err());
        assert!(load_snapshot("snap-2").is_ok(), "pinned 必须保留");
        assert!(load_snapshot("snap-3").is_ok());
        assert!(load_snapshot("snap-4").is_ok());
        let _ = fs::remove_dir_all(&dir);
        std::env::remove_var("CODELATTICE_PUBLISH_DIR");
    }
}
