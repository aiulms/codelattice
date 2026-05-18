//! Shell project discovery and source file scanning.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ShellProjectKind {
    ScriptDirectory,
    Plain,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShellProject {
    pub root: PathBuf,
    pub kind: ShellProjectKind,
    pub source_files: Vec<PathBuf>,
    pub marker_files: Vec<PathBuf>,
}

const SCRIPT_EXTENSIONS: &[&str] = &["sh", "bash", "zsh", "ksh", "bats"];
const EXCLUDE_DIRS: &[&str] = &[
    ".git",
    ".gitnexus",
    ".claude",
    ".opencode",
    "target",
    "build",
    "dist",
    "node_modules",
    ".venv",
    "venv",
    "__pycache__",
];

pub fn find_shell_project_root(start: &Path) -> Option<ShellProject> {
    let root = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(start)
    };
    let source_files = scan_shell_files(&root);
    if source_files.is_empty() {
        return None;
    }

    let mut marker_files = Vec::new();
    for marker in ["scripts", "bin"] {
        let p = root.join(marker);
        if p.is_dir() {
            marker_files.push(p);
        }
    }
    for marker in [
        "build.sh",
        "test.sh",
        "release.sh",
        "install.sh",
        "smoke.sh",
        "Makefile",
    ] {
        let p = root.join(marker);
        if p.is_file() {
            marker_files.push(p);
        }
    }

    let kind = if marker_files.is_empty() {
        ShellProjectKind::Plain
    } else {
        ShellProjectKind::ScriptDirectory
    };

    Some(ShellProject {
        root,
        kind,
        source_files,
        marker_files,
    })
}

pub fn list_shell_source_files(project: &ShellProject) -> Result<Vec<PathBuf>, String> {
    Ok(scan_shell_files(&project.root))
}

pub fn scan_shell_files(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !EXCLUDE_DIRS.contains(&name) && !name.starts_with(".tmp") {
                        stack.push(path);
                    }
                }
            } else if is_shell_script(&path) {
                out.push(path);
            }
        }
    }
    out.sort();
    out
}

pub fn is_shell_script(path: &Path) -> bool {
    if path
        .extension()
        .and_then(|e| e.to_str())
        .map(|ext| SCRIPT_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
    {
        return true;
    }

    if path.extension().is_some() {
        return false;
    }
    let Ok(text) = std::fs::read_to_string(path) else {
        return false;
    };
    text.lines()
        .next()
        .map(|line| {
            line.starts_with("#!")
                && ["sh", "bash", "zsh", "ksh"]
                    .iter()
                    .any(|shell| line.contains(shell))
        })
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_portable_smoke_shell_files() {
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("fixtures")
            .join("shell")
            .join("portable-smoke");
        let project = find_shell_project_root(&root).expect("shell project");
        assert_eq!(project.kind, ShellProjectKind::ScriptDirectory);
        assert!(project.source_files.len() >= 4);
        assert!(project.source_files.iter().any(|p| p.ends_with("build.sh")));
    }
}
