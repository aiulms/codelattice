// commands/secrets —— SecretStore 命令（返工 G-fix 拆分）。
use serde_json::json;
use serde_json::Value;
use tauri::State;

use crate::AppState;

#[tauri::command]
pub fn workbench_secret_set(
    state: State<AppState>,
    service: String,
    account: String,
    secret: String,
) -> Result<Value, String> {
    let mut store = state.gateway.lock().unwrap();
    let reference = {
        let store = store.secret_store.as_mut();
        store.set(&service, &account, &secret).map_err(|e| format!("{e:?}"))?
    };
    Ok(json!({
        "secretRef": reference,
        "masked": understanding_gateway::secret::MASKED,
        "stored": true,
    }))
}

#[tauri::command]
pub fn workbench_secret_delete(
    state: State<AppState>,
    secret_ref: String,
) -> Result<Value, String> {
    let mut gw = state.gateway.lock().unwrap();
    let ok = gw
        .secret_store
        .as_mut()
        .delete(&secret_ref)
        .map(|_| true)
        .unwrap_or(false);
    Ok(json!({"ok": ok}))
}
