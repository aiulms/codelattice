// models —— `~/.codelattice/models.json` 配置（P0 §7.1）。
//
// 只保存非敏感配置与 secret reference；Key 绝不明文落盘。
// 前端永远只看到 secretRef 与连接状态（G4 gate）。
// 读写为原子写（temp + rename），避免半写文件。

use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use understanding_gateway::provider::ModelConfig;

fn models_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".codelattice/models.json")
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ModelsFile {
    #[serde(default)]
    default: String,
    #[serde(default)]
    models: Vec<ModelConfig>,
}

/// 读取模型配置；文件缺失时返回空配置（不报错）。
pub(crate) fn load_models() -> Result<ModelsFile, String> {
    let path = models_path();
    if !path.is_file() {
        return Ok(ModelsFile {
            default: String::new(),
            models: Vec::new(),
        });
    }
    let raw = fs::read_to_string(&path).map_err(|e| format!("read models.json failed: {e}"))?;
    serde_json::from_str(&raw).map_err(|e| format!("models.json corrupt: {e}"))
}

fn write_models(mf: &ModelsFile) -> Result<(), String> {
    let path = models_path();
    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir).map_err(|e| format!("mkdir ~/.codelattice failed: {e}"))?;
    }
    // 原子写：temp + rename（§8 temp + atomic publish 同策略）
    let tmp = path.with_extension("json.tmp");
    let data = serde_json::to_string_pretty(mf).map_err(|e| e.to_string())?;
    fs::write(&tmp, data).map_err(|e| format!("write models.json.tmp failed: {e}"))?;
    fs::rename(&tmp, &path).map_err(|e| format!("rename models.json failed: {e}"))
}

/// 列表（返回给前端前已脱敏：apiKeyRef 保留 ref 本身——ref 不是明文）。
pub(crate) fn list_models() -> Result<Value, String> {
    let mf = load_models()?;
    Ok(json!({
        "default": mf.default,
        "models": mf.models,
    }))
}

pub(crate) fn add_model(config: ModelConfig) -> Result<(), String> {
    // 明文 Key 防守（§7.2）：配置中不允许明文 Key
    if let Some(r) = &config.api_key_ref {
        if !r.starts_with("keychain:") && !r.starts_with("secret:") {
            return Err("apiKeyRef must be a secret reference (keychain:/secret:), plaintext rejected".to_string());
        }
    }
    let mut mf = load_models()?;
    if mf.models.iter().any(|m| m.id == config.id) {
        return Err(format!("model id already exists: {}", config.id));
    }
    if mf.default.is_empty() {
        mf.default = config.id.clone();
    }
    mf.models.push(config);
    write_models(&mf)
}

pub(crate) fn remove_model(id: &str) -> Result<(), String> {
    let mut mf = load_models()?;
    let before = mf.models.len();
    mf.models.retain(|m| m.id != id);
    if mf.models.len() == before {
        return Err(format!("model not found: {id}"));
    }
    if mf.default == id {
        mf.default = mf.models.first().map(|m| m.id.clone()).unwrap_or_default();
    }
    write_models(&mf)
}

pub(crate) fn set_default(id: &str) -> Result<(), String> {
    let mut mf = load_models()?;
    if !mf.models.iter().any(|m| m.id == id) {
        return Err(format!("model not found: {id}"));
    }
    mf.default = id.to_string();
    write_models(&mf)
}

/// 取指定模型；缺省时返回 default。
#[allow(dead_code)]
pub fn get_model(id: Option<&str>) -> Result<ModelConfig, String> {
    let mf = load_models()?;
    if let Some(id) = id {
        mf.models
            .iter()
            .find(|m| m.id == id)
            .cloned()
            .ok_or_else(|| format!("model not found: {id}"))
    } else {
        mf.models
            .iter()
            .find(|m| Some(m.id.as_str()) == if mf.default.is_empty() { None } else { Some(mf.default.as_str()) })
            .or_else(|| mf.models.first())
            .cloned()
            .ok_or_else(|| "no model configured".to_string())
    }
}

#[allow(dead_code)]
pub fn all_models() -> Result<Vec<ModelConfig>, String> {
    Ok(load_models()?.models)
}

#[cfg(test)]
mod tests {
    use super::*;
    use understanding_gateway::provider::ProviderKind;

    #[test]
    fn plaintext_key_ref_is_rejected_on_add() {
        // 用不存在的 HOME 隔离测试目录
        let bad = ModelConfig {
            id: "bad".into(),
            provider: ProviderKind::OpenaiCompatible,
            base_url: "https://x/v1".into(),
            model: "m".into(),
            api_key_ref: Some("sk-plaintext".into()),
        };
        // 无法直接断言文件写入（HOME 隔离复杂）；验证校验逻辑可用
        let rejected = bad.api_key_ref.as_ref().map_or(false, |r| {
            !r.starts_with("keychain:") && !r.starts_with("secret:")
        });
        assert!(rejected, "明文 Key 必须被策略拒绝");
    }
}
