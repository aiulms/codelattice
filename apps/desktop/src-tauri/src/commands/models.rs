// commands/models —— 模型池管理命令（返工 G-fix 拆分）。
use serde_json::{json, Value};
use tauri::State;

use understanding_gateway::provider::{ModelAdapter, ModelConfig};
use understanding_gateway::provider_http::HttpModelAdapter;

use crate::models;
use crate::AppState;

use super::common;

#[tauri::command]
pub fn workbench_models_list() -> Result<Value, String> {
    models::list_models()
}

#[tauri::command]
pub fn workbench_models_add(state: State<AppState>, config: Value) -> Result<Value, String> {
    let cfg: ModelConfig = serde_json::from_value(config).map_err(|e| e.to_string())?;
    models::add_model(cfg.clone())?;
    let mut gw = state.gateway.lock().unwrap();
    gw.register_model(cfg);
    Ok(json!({"ok": true}))
}

#[tauri::command]
pub fn workbench_models_remove(state: State<AppState>, id: String) -> Result<Value, String> {
    // 返工修复：删除模型前先取出 api_key_ref，删除后清理关联 secret
    let model = models::get_model(Some(&id)).ok();
    let secret_ref = model.and_then(|m| m.api_key_ref.clone());
    models::remove_model(&id)?;
    let mut gw = state.gateway.lock().unwrap();
    let (removed, purged) = gw.unregister_model(&id);
    let secret_cleaned = if let Some(reference) = &secret_ref {
        gw.secret_store.as_mut().delete(reference).is_ok()
    } else {
        false
    };
    Ok(json!({"ok": removed, "purgedCacheEntries": purged, "secretCleaned": secret_cleaned}))
}

#[tauri::command]
pub fn workbench_models_set_default(id: String) -> Result<Value, String> {
    models::set_default(&id)?;
    Ok(json!({"ok": true}))
}

/// 编辑已配置模型：先落盘再同步 gateway 内存池。
/// unregister + register 而非直接 add —— ModelPool::add 拒绝重复 id，
/// 且配置变更后旧模型关联缓存应一并失效。
#[tauri::command]
pub fn workbench_models_update(state: State<AppState>, config: Value) -> Result<Value, String> {
    let cfg: ModelConfig = serde_json::from_value(config).map_err(|e| e.to_string())?;
    models::update_model(cfg.clone())?;
    let mut gw = state.gateway.lock().unwrap();
    gw.unregister_model(&cfg.id);
    gw.register_model(cfg);
    Ok(json!({"ok": true}))
}

/// 连接测试：authenticated ping 携带解析后的凭证（§7.2 返工修复）。
#[tauri::command]
pub fn workbench_models_test(state: State<AppState>, id: String) -> Result<Value, String> {
    let model = models::get_model(Some(&id))?;
    let api_key = common::resolve_api_key(&state, &model)?;
    let adapter = HttpModelAdapter::new(model)?;
    let status = adapter.ping_with_key(api_key.as_deref());
    Ok(json!({
        "provider": format!("{:?}", adapter.kind()),
        "ok": status.ok,
        "detail": status.detail,
    }))
}
