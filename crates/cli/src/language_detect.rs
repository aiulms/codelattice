//! 语言检测模块 — 实现 --language auto 的简单检测逻辑
//!
//! 检测策略（零外部依赖）：
//! 1. 有 oh-package.json5 → arkts
//! 2. 有 Cargo.toml → rust
//! 3. 有 cjpm.toml → cangjie
//! 4. 有 tsconfig.json 或 package.json（非 ArkTS/非 Rust/非 Cangjie）→ typescript
//! 5. 多种存在 → 报错要求显式指定
//! 6. 都没有 → 报错"无法检测语言"

use std::path::Path;

use crate::unified_types::DetectedLanguage;

/// 从项目根目录检测语言
///
/// 检查根目录下的清单文件存在性。
pub fn detect_language(root: &Path) -> DetectedLanguage {
    let has_cargo = root.join("Cargo.toml").is_file();
    let has_cjpm = root.join("cjpm.toml").is_file();
    let has_oh_pkg = root.join("oh-package.json5").is_file();
    let has_tsconfig = root.join("tsconfig.json").is_file();
    let has_pkg_json = root.join("package.json").is_file();

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
}
