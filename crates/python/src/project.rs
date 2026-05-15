//! Python project root detection and source file discovery.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Kind of Python project detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PythonProjectKind {
    /// Has pyproject.toml.
    PyProject,
    /// Has setup.py.
    SetupPy,
    /// Has setup.cfg.
    SetupCfg,
    /// Has requirements.txt.
    Requirements,
    /// Has poetry.lock / uv.lock / pdm.lock.
    Lockfile,
    /// Fallback: only .py files found.
    Plain,
}

/// Minimal Python project model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PythonProject {
    /// Absolute path to the project root.
    pub root: PathBuf,
    /// Kind of project detected.
    pub kind: PythonProjectKind,
    /// All Python source files (.py) discovered.
    pub source_files: Vec<PathBuf>,
    /// Stub files (.pyi) discovered.
    pub stub_files: Vec<PathBuf>,
    /// Project marker files found (pyproject.toml, setup.py, etc.).
    pub marker_files: Vec<PathBuf>,
}

// ---------------------------------------------------------------------------
// Extension constants
// ---------------------------------------------------------------------------

/// Python source file extension.
const PY_EXTENSION: &str = "py";
/// Python stub file extension.
const PYI_EXTENSION: &str = "pyi";

/// Directories to exclude from file scanning.
const EXCLUDE_DIRS: &[&str] = &[
    ".git",
    ".gitnexus",
    "target",
    "build",
    "dist",
    "__pycache__",
    ".pytest_cache",
    ".mypy_cache",
    ".ruff_cache",
    ".venv",
    "venv",
    "env",
    ".tox",
    "site-packages",
    "node_modules",
];

// ---------------------------------------------------------------------------
// Project root detection
// ---------------------------------------------------------------------------

/// Walk up from `start` until a recognizable Python project marker is found.
///
/// Markers (in priority order):
/// 1. `pyproject.toml`
/// 2. `setup.py`
/// 3. `setup.cfg`
/// 4. `requirements.txt`
/// 5. `poetry.lock` / `uv.lock` / `pdm.lock`
/// 6. Fallback: current directory if it contains .py files
pub fn find_python_project_root(start: &Path) -> Option<PythonProject> {
    let mut current = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::env::current_dir().ok()?.join(start)
    };

    loop {
        let pyproject = current.join("pyproject.toml");
        let setup_py = current.join("setup.py");
        let setup_cfg = current.join("setup.cfg");
        let requirements = current.join("requirements.txt");
        let poetry_lock = current.join("poetry.lock");
        let uv_lock = current.join("uv.lock");
        let pdm_lock = current.join("pdm.lock");

        let mut marker_files = Vec::new();
        let mut kind = None;

        if pyproject.is_file() {
            kind = Some(PythonProjectKind::PyProject);
            marker_files.push(pyproject);
        } else if setup_py.is_file() {
            kind = Some(PythonProjectKind::SetupPy);
            marker_files.push(setup_py);
        } else if setup_cfg.is_file() {
            kind = Some(PythonProjectKind::SetupCfg);
            marker_files.push(setup_cfg);
        } else if requirements.is_file() {
            kind = Some(PythonProjectKind::Requirements);
            marker_files.push(requirements);
        } else if poetry_lock.is_file() || uv_lock.is_file() || pdm_lock.is_file() {
            kind = Some(PythonProjectKind::Lockfile);
            if poetry_lock.is_file() {
                marker_files.push(poetry_lock);
            }
            if uv_lock.is_file() {
                marker_files.push(uv_lock);
            }
            if pdm_lock.is_file() {
                marker_files.push(pdm_lock);
            }
        }

        if let Some(k) = kind {
            let (source_files, stub_files) = scan_python_files(&current);
            return Some(PythonProject {
                root: current,
                kind: k,
                source_files,
                stub_files,
                marker_files,
            });
        }

        // Fallback: .py files in this directory tree?
        if has_python_files(&current) {
            let (source_files, stub_files) = scan_python_files(&current);
            return Some(PythonProject {
                root: current,
                kind: PythonProjectKind::Plain,
                source_files,
                stub_files,
                marker_files: Vec::new(),
            });
        }

        current = current.parent()?.to_path_buf();
    }
}

/// List Python source and stub files from the project root.
pub fn list_python_source_files(
    project: &PythonProject,
) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    let (source_files, stub_files) = scan_python_files(&project.root);
    Ok((source_files, stub_files))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Check if any .py files exist in the directory tree.
fn has_python_files(root: &Path) -> bool {
    walk_for_extension(root, PY_EXTENSION)
}

/// Walk directory tree scanning for Python files.
fn scan_python_files(root: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let mut source_files = Vec::new();
    let mut stub_files = Vec::new();

    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !EXCLUDE_DIRS.contains(&name) && !name.starts_with('.') {
                        stack.push(path);
                    }
                }
            } else {
                let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                if ext == PY_EXTENSION {
                    source_files.push(path);
                } else if ext == PYI_EXTENSION {
                    stub_files.push(path);
                }
            }
        }
    }

    source_files.sort();
    stub_files.sort();
    (source_files, stub_files)
}

/// Walk directory tree checking if any file matches the given extension.
fn walk_for_extension(root: &Path, extension: &str) -> bool {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !EXCLUDE_DIRS.contains(&name) && !name.starts_with('.') {
                        stack.push(path);
                    }
                }
            } else if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|ext| ext == extension)
                .unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
}
