//! 语言检测模块 — 实现 --language auto 的简单检测逻辑
//!
//! 检测策略（零外部依赖）：
//! 1. 有 Cargo.toml → rust
//! 2. 有 cjpm.toml → cangjie
//! 3. 两者都有 → 报错要求显式指定
//! 4. 两者都没有 → 报错"无法检测语言"

use std::path::Path;

use crate::unified_types::DetectedLanguage;

/// 从项目根目录检测语言
///
/// 检查根目录下的 Cargo.toml 和 cjpm.toml 存在性。
pub fn detect_language(root: &Path) -> DetectedLanguage {
    let has_cargo = root.join("Cargo.toml").is_file();
    let has_cjpm = root.join("cjpm.toml").is_file();

    match (has_cargo, has_cjpm) {
        (true, false) => DetectedLanguage::Rust,
        (false, true) => DetectedLanguage::Cangjie,
        (true, true) => DetectedLanguage::Ambiguous,
        (false, false) => DetectedLanguage::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    /// 创建临时目录并可选写入 Cargo.toml / cjpm.toml
    fn setup_temp_dir(cargo: bool, cjpm: bool, suffix: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("lang-detect-{}-{}", suffix, std::process::id()));
        let _ = fs::remove_dir_all(&dir); // 先清理可能残留的目录
        fs::create_dir_all(&dir).unwrap();
        if cargo {
            fs::write(dir.join("Cargo.toml"), "[package]\nname = \"test\"\n").unwrap();
        }
        if cjpm {
            fs::write(dir.join("cjpm.toml"), "[package]\nname = \"test\"\n").unwrap();
        }
        dir
    }

    fn cleanup(dir: &PathBuf) {
        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn detect_cargo_project_as_rust() {
        let dir = setup_temp_dir(true, false, "rust");
        assert_eq!(detect_language(&dir), DetectedLanguage::Rust);
        cleanup(&dir);
    }

    #[test]
    fn detect_cjpm_project_as_cangjie() {
        let dir = setup_temp_dir(false, true, "cjpm");
        assert_eq!(detect_language(&dir), DetectedLanguage::Cangjie);
        cleanup(&dir);
    }

    #[test]
    fn detect_both_as_ambiguous() {
        let dir = setup_temp_dir(true, true, "both");
        assert_eq!(detect_language(&dir), DetectedLanguage::Ambiguous);
        cleanup(&dir);
    }

    #[test]
    fn detect_neither_as_unknown() {
        let dir = setup_temp_dir(false, false, "none");
        assert_eq!(detect_language(&dir), DetectedLanguage::Unknown);
        cleanup(&dir);
    }
}
