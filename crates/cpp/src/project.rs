//! C++ project root detection and source file discovery.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Types
// ---------------------------------------------------------------------------

/// Kind of C++ project detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CppProjectKind {
    /// Has compile_commands.json.
    CompileCommands,
    /// Has CMakeLists.txt.
    CMake,
    /// Has Makefile.
    Make,
    /// Has configure.ac (autotools).
    Autoconf,
    /// Fallback: only C++ files found.
    Plain,
}

/// Minimal C++ project model.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CppProject {
    /// Absolute path to the project root.
    pub root: PathBuf,
    /// Kind of project detected.
    pub kind: CppProjectKind,
    /// All C++ source files (.cpp, .cc, .cxx, etc.) discovered.
    pub source_files: Vec<PathBuf>,
    /// All C++ header files (.hpp, .hh, .hxx, .h, etc.) discovered.
    pub header_files: Vec<PathBuf>,
    /// Build system files found.
    pub build_files: Vec<PathBuf>,
}

// ---------------------------------------------------------------------------
// Extension constants
// ---------------------------------------------------------------------------

/// C++ source file extensions.
const CPP_SOURCE_EXTENSIONS: &[&str] = &["cpp", "cc", "cxx", "c++", "C"];
/// C++ header file extensions.
const CPP_HEADER_EXTENSIONS: &[&str] = &["hpp", "hh", "hxx", "h++", "H"];
/// C-compatible header extensions that may appear in C++ projects.
const CPP_C_HEADER_EXTENSIONS: &[&str] = &["h"];
/// Optional include/template extensions.
const CPP_OPTIONAL_EXTENSIONS: &[&str] = &["ixx", "ipp", "tpp"];

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

// ---------------------------------------------------------------------------
// Project root detection
// ---------------------------------------------------------------------------

/// Walk up from `start` until a recognizable C++ project marker is found.
///
/// Markers (in priority order):
/// 1. `compile_commands.json`
/// 2. `CMakeLists.txt`
/// 3. `Makefile` or `GNUmakefile`
/// 4. `configure.ac`
/// 5. Fallback: current directory if it contains C++ files
pub fn find_cpp_project_root(start: &Path) -> Option<CppProject> {
    let canonical = if start.is_absolute() {
        start.to_path_buf()
    } else {
        std::fs::canonicalize(start).ok()?
    };

    // Walk upward looking for markers
    let mut current = canonical.as_path();
    let mut found_build_files = Vec::new();

    loop {
        if current.join("compile_commands.json").is_file() {
            found_build_files.push(current.join("compile_commands.json"));
            return build_project(current, CppProjectKind::CompileCommands, &found_build_files);
        }
        if current.join("CMakeLists.txt").is_file() {
            found_build_files.push(current.join("CMakeLists.txt"));
            return build_project(current, CppProjectKind::CMake, &found_build_files);
        }
        if current.join("Makefile").is_file() {
            found_build_files.push(current.join("Makefile"));
            return build_project(current, CppProjectKind::Make, &found_build_files);
        }
        if current.join("GNUmakefile").is_file() {
            found_build_files.push(current.join("GNUmakefile"));
            return build_project(current, CppProjectKind::Make, &found_build_files);
        }
        if current.join("configure.ac").is_file() {
            found_build_files.push(current.join("configure.ac"));
            return build_project(current, CppProjectKind::Autoconf, &found_build_files);
        }

        match current.parent() {
            Some(parent) => current = parent,
            None => break,
        }
    }

    // Fallback: check if the original directory has C++ files
    if has_cpp_files(&canonical) {
        build_project(&canonical, CppProjectKind::Plain, &[])
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// File listing
// ---------------------------------------------------------------------------

/// List all C++ source and header files in the project.
pub fn list_cpp_source_files(project: &CppProject) -> Result<(Vec<PathBuf>, Vec<PathBuf>), String> {
    let mut source_files = Vec::new();
    let mut header_files = Vec::new();

    let all_extensions: Vec<&str> = CPP_SOURCE_EXTENSIONS
        .iter()
        .chain(CPP_HEADER_EXTENSIONS.iter())
        .chain(CPP_C_HEADER_EXTENSIONS.iter())
        .chain(CPP_OPTIONAL_EXTENSIONS.iter())
        .copied()
        .collect();

    walk_for_extensions(
        &project.root,
        &all_extensions,
        &mut source_files,
        &mut header_files,
    );

    source_files.sort();
    header_files.sort();

    Ok((source_files, header_files))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_project(root: &Path, kind: CppProjectKind, build_files: &[PathBuf]) -> Option<CppProject> {
    let mut project = CppProject {
        root: root.to_path_buf(),
        kind,
        source_files: Vec::new(),
        header_files: Vec::new(),
        build_files: build_files.to_vec(),
    };

    let (srcs, hdrs) = list_cpp_source_files(&project).ok()?;
    project.source_files = srcs;
    project.header_files = hdrs;

    if project.source_files.is_empty() && project.header_files.is_empty() {
        return None;
    }

    Some(project)
}

fn walk_for_extensions(
    root: &Path,
    extensions: &[&str],
    source_files: &mut Vec<PathBuf>,
    header_files: &mut Vec<PathBuf>,
) {
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
                let ext = match path.extension().and_then(|e| e.to_str()) {
                    Some(e) => e,
                    None => continue,
                };

                if !extensions.contains(&ext) {
                    continue;
                }

                if CPP_SOURCE_EXTENSIONS.contains(&ext) || CPP_OPTIONAL_EXTENSIONS.contains(&ext) {
                    source_files.push(path);
                } else if CPP_HEADER_EXTENSIONS.contains(&ext)
                    || CPP_C_HEADER_EXTENSIONS.contains(&ext)
                {
                    header_files.push(path);
                }
            }
        }
    }
}

/// Check if a directory tree contains any C++ source files.
fn has_cpp_files(root: &Path) -> bool {
    let extensions: Vec<&str> = CPP_SOURCE_EXTENSIONS
        .iter()
        .chain(CPP_HEADER_EXTENSIONS.iter())
        .copied()
        .collect();

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
                .map(|ext| extensions.contains(&ext))
                .unwrap_or(false)
            {
                return true;
            }
        }
    }
    false
}

/// Check if a directory tree has C++ files (for language detection).
/// Returns true if any .cpp/.cc/.cxx/.hpp/.hh/.hxx file exists.
pub fn directory_has_cpp_files(root: &Path) -> bool {
    has_cpp_files(root)
}

/// Check if a directory tree has C but NO C++ files.
/// Returns (has_c, has_cpp).
pub fn directory_c_cpp_status(root: &Path) -> (bool, bool) {
    let cpp_exts: Vec<&str> = CPP_SOURCE_EXTENSIONS
        .iter()
        .chain(CPP_HEADER_EXTENSIONS.iter())
        .copied()
        .collect();
    let c_exts = ["c", "h"];

    let mut has_c = false;
    let mut has_cpp = false;

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
                if cpp_exts.contains(&ext) {
                    has_cpp = true;
                } else if c_exts.contains(&ext) {
                    has_c = true;
                }
            }
            if has_c && has_cpp {
                return (true, true);
            }
        }
    }
    (has_c, has_cpp)
}

/// Check if a file has a C++ source extension.
pub fn is_cpp_source_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| CPP_SOURCE_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}

/// Check if a file has a C++ header extension (including .h).
pub fn is_cpp_header_file(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| CPP_HEADER_EXTENSIONS.contains(&ext) || CPP_C_HEADER_EXTENSIONS.contains(&ext))
        .unwrap_or(false)
}
