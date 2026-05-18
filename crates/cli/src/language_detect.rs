//! 语言检测模块 — 实现 --language auto 的简单检测逻辑
//!
//! 检测策略（零外部依赖）：
//! 1. 有 oh-package.json5 → arkts
//! 2. 有 Cargo.toml → rust
//! 3. 有 cjpm.toml → cangjie
//! 4. 有 tsconfig.json 或 package.json（非 ArkTS/非 Rust/非 Cangjie）→ typescript
//! 5. 有 C 项目标记且无 C++ 文件 → c
//! 6. 有 Python 项目标记 → python
//! 7. 只有 shell 脚本 / shebang → shell
//! 8. 多种存在 → 报错要求显式指定
//! 9. 都没有 → 报错"无法检测语言"

use std::path::Path;

use crate::unified_types::DetectedLanguage;

/// C++ file extensions — if found, C auto-detect returns Unknown.
const CPP_EXTENSIONS: &[&str] = &[
    "cpp", "cc", "cxx", "c++", "C", "hpp", "hh", "hxx", "h++", "H",
];

/// Check if any file in the root directory tree has a C++ extension.
fn has_cpp_files(root: &Path) -> bool {
    walk_for_extension(root, CPP_EXTENSIONS)
}

/// Walk directory tree checking if any file matches the given extensions.
fn walk_for_extension(root: &Path, extensions: &[&str]) -> bool {
    let mut stack = vec![root.to_path_buf()];
    let skip = [
        "target",
        "build",
        ".git",
        "node_modules",
        "dist",
        "cmake-build-debug",
        "cmake-build-release",
        ".gitnexus",
    ];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !skip.contains(&name) && !name.starts_with('.') {
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

/// Check if any .c or .h files exist in the tree.
fn has_c_files(root: &Path) -> bool {
    walk_for_extension(root, &["c", "h"])
}

/// Check if any .py files exist in the tree.
fn has_python_files(root: &Path) -> bool {
    walk_for_extension(root, &["py"])
}

fn has_shell_files(root: &Path) -> bool {
    if walk_for_extension(root, &["sh", "bash", "zsh", "ksh", "bats"]) {
        return true;
    }
    let mut stack = vec![root.to_path_buf()];
    let skip = [
        "target",
        "build",
        ".git",
        "node_modules",
        "dist",
        ".gitnexus",
        ".venv",
        "venv",
    ];
    while let Some(dir) = stack.pop() {
        let entries = match std::fs::read_dir(&dir) {
            Ok(rd) => rd,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !skip.contains(&name) && !name.starts_with(".tmp") {
                        stack.push(path);
                    }
                }
                continue;
            }
            if path.extension().is_none() {
                if let Ok(text) = std::fs::read_to_string(&path) {
                    if text
                        .lines()
                        .next()
                        .map(|l| {
                            l.starts_with("#!")
                                && ["sh", "bash", "zsh", "ksh"]
                                    .iter()
                                    .any(|shell| l.contains(shell))
                        })
                        .unwrap_or(false)
                    {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// 从项目根目录检测语言
///
/// 检查根目录下的清单文件存在性。
pub fn detect_language(root: &Path) -> DetectedLanguage {
    let has_cargo = root.join("Cargo.toml").is_file();
    let has_cjpm = root.join("cjpm.toml").is_file();
    let has_oh_pkg = root.join("oh-package.json5").is_file();
    let has_tsconfig = root.join("tsconfig.json").is_file();
    let has_pkg_json = root.join("package.json").is_file();

    // C project markers (lower priority than above)
    let has_cmake = root.join("CMakeLists.txt").is_file();
    let has_makefile = root.join("Makefile").is_file() || root.join("GNUmakefile").is_file();
    let has_autoconf = root.join("configure.ac").is_file();

    // Collect all detected language markers
    let mut detected = Vec::new();
    if has_oh_pkg {
        detected.push(DetectedLanguage::ArkTS);
    }
    if has_cargo {
        detected.push(DetectedLanguage::Rust);
    }
    if has_cjpm {
        detected.push(DetectedLanguage::Cangjie);
    }
    // TypeScript: only if not already claimed by ArkTS/Rust/Cangjie
    if has_tsconfig || (has_pkg_json && !has_oh_pkg && !has_cargo && !has_cjpm) {
        detected.push(DetectedLanguage::TypeScript);
    }
    // C: only if no stronger markers, has C markers/files, and NO C++ files
    if !has_cargo && !has_cjpm && !has_oh_pkg && !has_tsconfig && !has_pkg_json {
        let has_c_markers = has_cmake || has_makefile || has_autoconf || has_c_files(root);
        let has_cpp = has_cpp_files(root);
        if has_cpp {
            // C++ files present — detect as C++ (covers pure C++ and mixed C/C++)
            detected.push(DetectedLanguage::Cpp);
        } else if has_c_markers {
            // Pure C project, no C++ files
            detected.push(DetectedLanguage::C);
        }
    }

    // Python: check for Python markers
    let has_pyproject = root.join("pyproject.toml").is_file();
    let has_setup_py = root.join("setup.py").is_file();
    let has_setup_cfg = root.join("setup.cfg").is_file();
    let has_requirements = root.join("requirements.txt").is_file();
    let has_python_markers = has_pyproject
        || has_setup_py
        || has_setup_cfg
        || has_requirements
        || has_python_files(root);
    if has_python_markers {
        detected.push(DetectedLanguage::Python);
    }
    // Shell 是低优先级 glue 语言：只有没有更强项目语言时才自动识别，
    // 避免普通 Rust/TS/Python 仓库因 scripts/*.sh 变成 ambiguous。
    if detected.is_empty() && has_shell_files(root) {
        detected.push(DetectedLanguage::Shell);
    }

    match detected.len() {
        0 => DetectedLanguage::Unknown,
        1 => detected[0],
        _ => DetectedLanguage::Ambiguous,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn setup_temp_dir(
        cargo: bool,
        cjpm: bool,
        oh_pkg: bool,
        tsconfig: bool,
        suffix: &str,
    ) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lang-detect-{}-{}", suffix, std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        if cargo {
            fs::write(dir.join("Cargo.toml"), "[package]\nname = \"test\"\n").unwrap();
        }
        if cjpm {
            fs::write(dir.join("cjpm.toml"), "[package]\nname = \"test\"\n").unwrap();
        }
        if oh_pkg {
            fs::write(dir.join("oh-package.json5"), "{ name: 'test' }").unwrap();
        }
        if tsconfig {
            fs::write(dir.join("tsconfig.json"), "{}").unwrap();
        }
        dir
    }

    fn cleanup(dir: &PathBuf) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn detect_cargo_project_as_rust() {
        let dir = setup_temp_dir(true, false, false, false, "rust");
        assert_eq!(detect_language(&dir), DetectedLanguage::Rust);
        cleanup(&dir);
    }

    #[test]
    fn detect_cjpm_project_as_cangjie() {
        let dir = setup_temp_dir(false, true, false, false, "cjpm");
        assert_eq!(detect_language(&dir), DetectedLanguage::Cangjie);
        cleanup(&dir);
    }

    #[test]
    fn detect_oh_package_as_arkts() {
        let dir = setup_temp_dir(false, false, true, false, "arkts");
        assert_eq!(detect_language(&dir), DetectedLanguage::ArkTS);
        cleanup(&dir);
    }

    #[test]
    fn detect_tsconfig_as_typescript() {
        let dir = setup_temp_dir(false, false, false, true, "typescript");
        assert_eq!(detect_language(&dir), DetectedLanguage::TypeScript);
        cleanup(&dir);
    }

    #[test]
    fn detect_both_as_ambiguous() {
        let dir = setup_temp_dir(true, true, false, false, "both");
        assert_eq!(detect_language(&dir), DetectedLanguage::Ambiguous);
        cleanup(&dir);
    }

    #[test]
    fn detect_neither_as_unknown() {
        let dir = setup_temp_dir(false, false, false, false, "none");
        assert_eq!(detect_language(&dir), DetectedLanguage::Unknown);
        cleanup(&dir);
    }

    #[test]
    fn detect_shell_only_project_as_shell() {
        let dir = setup_temp_dir(false, false, false, false, "shell");
        fs::write(dir.join("build.sh"), "#!/usr/bin/env bash\necho ok\n").unwrap();
        assert_eq!(detect_language(&dir), DetectedLanguage::Shell);
        cleanup(&dir);
    }

    #[test]
    fn shell_scripts_do_not_make_rust_project_ambiguous() {
        let dir = setup_temp_dir(true, false, false, false, "rust-shell");
        fs::write(dir.join("build.sh"), "#!/usr/bin/env bash\necho ok\n").unwrap();
        assert_eq!(detect_language(&dir), DetectedLanguage::Rust);
        cleanup(&dir);
    }
}
