//! C project root detection and source file discovery.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Kind of C project detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CProjectKind {
    /// Has compile_commands.json.
    CompileCommands,
    /// Has CMakeLists.txt.
    CMake,
    /// Has Makefile.
    Make,
    /// Has configure.ac (autotools).
    Autoconf,
    /// Fallback: only .c/.h files found.
    Plain,
}

/// Minimal C project model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CProject {
    /// Absolute path to the project root.
    pub root: PathBuf,
    /// Kind of project detected.
    pub kind: CProjectKind,
    /// All C source files (.c) discovered.
    pub source_files: Vec<PathBuf>,
    /// All C header files (.h) discovered.
    pub header_files: Vec<PathBuf>,
    /// Build system files found.
    pub build_files: Vec<PathBuf>,
}

// ---------------------------------------------------------------------------
// C++ extension check (CRITICAL — prevents C++ misidentification)
// ---------------------------------------------------------------------------

/// C++ file extensions that indicate a project is NOT pure C.
const CPP_EXTENSIONS: &[&str] = &[
    "cpp", "cc", "cxx", "c++", "C", // C++ source
    "hpp", "hh", "hxx", "h++", "H", // C++ headers
];

/// Check if a file has a C++ extension.
fn is_cpp_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| CPP_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Project root detection
// ---------------------------------------------------------------------------

/// C source file extensions.
const C_SOURCE_EXTENSIONS: &[&str] = &["c"];
/// C header file extensions.
const C_HEADER_EXTENSIONS: &[&str] = &["h"];
/// Optional include file extensions.
const C_INCLUDE_EXTENSIONS: &[&str] = &["inc"];

/// Directories to exclude from file scanning.
const EXCLUDE_DIRS: &[&str] = &[
    "target",
    "build",
    "cmake-build-debug",
    "cmake-build-release",
    ".git",
    ".gitnexus",
    "node_modules",
    "dist",
];

/// Walk up from `start` until a recognizable C project marker is found.
///
/// Markers (in priority order):
/// 1. compile_commands.json
/// 2. CMakeLists.txt
/// 3. Makefile / GNUmakefile
/// 4. configure.ac
///
/// If a marker is found, also checks for C++ files — if found, returns None
/// (ambiguous project, require explicit `--language c`).
///
/// Fallback: if no marker but .c/.h files exist without C++ files, returns Plain.
pub fn find_c_project_root(start: &Path) -> Option<CProject> {
    let mut current = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent()?.to_path_buf()
    };

    loop {
        // Check markers in priority order
        let kind = None
            .or_else(|| {
                current
                    .join("compile_commands.json")
                    .is_file()
                    .then_some(CProjectKind::CompileCommands)
            })
            .or_else(|| {
                current
                    .join("CMakeLists.txt")
                    .is_file()
                    .then_some(CProjectKind::CMake)
            })
            .or_else(|| {
                (current.join("Makefile").is_file() || current.join("GNUmakefile").is_file())
                    .then_some(CProjectKind::Make)
            })
            .or_else(|| {
                current
                    .join("configure.ac")
                    .is_file()
                    .then_some(CProjectKind::Autoconf)
            });

        if let Some(project_kind) = kind {
            // Found a marker — check for C++ files before accepting
            if has_cpp_files_in_tree(&current) {
                return None; // Ambiguous — C/C++ mixed, require explicit --language c
            }
            let (source_files, header_files, build_files) = scan_c_files(&current);
            if source_files.is_empty() && header_files.is_empty() {
                // No C files — probably not a C project
                // Walk up
            } else {
                return Some(CProject {
                    root: current,
                    kind: project_kind,
                    source_files,
                    header_files,
                    build_files,
                });
            }
        }

        // Fallback: check if this directory has .c/.h files (no marker needed)
        let (source_files, header_files, build_files) = scan_c_files(&current);
        if !source_files.is_empty() || !header_files.is_empty() {
            if has_cpp_files_in_tree(&current) {
                return None; // Ambiguous
            }
            return Some(CProject {
                root: current,
                kind: CProjectKind::Plain,
                source_files,
                header_files,
                build_files,
            });
        }

        // Walk up
        current = current.parent()?.to_path_buf();
    }
}

/// Check if any C++ files exist in the directory tree.
fn has_cpp_files_in_tree(root: &Path) -> bool {
    walk_dir_filtered(root, |entry| {
        entry.file_type().map(|ft| ft.is_file()).unwrap_or(false) && is_cpp_file(&entry.path())
    })
    .first()
    .is_some()
}

/// List all C source and header files in the project.
pub fn list_c_source_files(project: &CProject) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    let (source_files, header_files, _) = scan_c_files(&project.root);
    Ok((source_files, header_files))
}

// ---------------------------------------------------------------------------
// File scanning
// ---------------------------------------------------------------------------

fn scan_c_files(root: &Path) -> (Vec<PathBuf>, Vec<PathBuf>, Vec<PathBuf>) {
    let mut source_files = Vec::new();
    let mut header_files = Vec::new();
    let mut build_files = Vec::new();

    let entries = walk_dir_filtered(root, |entry| {
        entry.file_type().map(|ft| ft.is_file()).unwrap_or(false)
    });

    for path in entries {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Check build files first (before move)
        if matches!(
            name,
            "CMakeLists.txt"
                | "Makefile"
                | "GNUmakefile"
                | "configure.ac"
                | "compile_commands.json"
        ) {
            build_files.push(path);
        } else if C_SOURCE_EXTENSIONS.contains(&ext) {
            source_files.push(path);
        } else if C_HEADER_EXTENSIONS.contains(&ext) || C_INCLUDE_EXTENSIONS.contains(&ext) {
            header_files.push(path);
        }
    }

    (source_files, header_files, build_files)
}

/// Walk directory, excluding certain dirs, collecting entries matching predicate.
fn walk_dir_filtered(root: &Path, predicate: impl Fn(&std::fs::DirEntry) -> bool) -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut stack = vec![root.to_path_buf()];

    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !EXCLUDE_DIRS.contains(&name) && !name.starts_with('.') {
                    stack.push(path);
                }
            } else if predicate(&entry) {
                result.push(path);
            }
        }
    }

    result.sort();
    result
}
