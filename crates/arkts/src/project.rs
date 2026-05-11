//! ArkTS project detection and source file discovery.

use std::path::{Path, PathBuf};

/// Check if a directory looks like an ArkTS project.
pub fn detect_arkts_project(dir: &Path) -> bool {
    dir.join("oh-package.json5").is_file()
}

/// Walk up from `start` until an `oh-package.json5` is found.
pub fn find_arkts_project_root(start: &Path) -> Option<PathBuf> {
    let mut current = if start.is_dir() {
        start.to_path_buf()
    } else {
        start.parent()?.to_path_buf()
    };

    loop {
        if current.join("oh-package.json5").is_file() {
            return Some(current);
        }
        if !current.pop() {
            return None;
        }
    }
}

/// List all `.ets` source files under the given directory.
///
/// Skips `node_modules/`, `.preview/`, `build/`, `oh_modules/`, and hidden directories.
pub fn list_arkts_source_files(dir: &Path) -> Result<Vec<PathBuf>, std::io::Error> {
    let mut files = Vec::new();
    list_recursive(dir, &mut files)?;
    files.sort();
    Ok(files)
}

fn list_recursive(dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), std::io::Error> {
    let entries = std::fs::read_dir(dir)?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            if name.starts_with('.')
                || matches!(
                    name,
                    "node_modules" | "oh_modules" | "build" | ".preview" | "hvigor"
                )
            {
                continue;
            }
            list_recursive(&path, files)?;
        } else if path.extension().and_then(|e| e.to_str()) == Some("ets") {
            files.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_detect_arkts_project() {
        let dir = std::env::temp_dir().join("arkts-detect-test");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        assert!(!detect_arkts_project(&dir));
        fs::write(dir.join("oh-package.json5"), "{}").unwrap();
        assert!(detect_arkts_project(&dir));
        let _ = fs::remove_dir_all(&dir);
    }
}
