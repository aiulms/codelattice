//! SecretStore 接口（P0 §5.1 / §7.2）。
//!
//! 前端永远只看到 secretRef 和连接状态，不拿到明文 Key。
//! OS 安全存储实现（Keychain/Keyring）属于 Tauri 层；本 trait 冻结契约，
//! 并带一个内存 fake 供测试（G4 gate）。

pub const MASKED: &str = "••••••••";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SecretError {
    NotFound,
    Io(String),
    InvalidRef,
}

pub type SecretResult<T> = Result<T, SecretError>;

/// secretRef 格式："keychain:<service>/<account>" 或 "secret:<id>"。
pub fn parse_secret_ref(reference: &str) -> Result<(String, String), SecretError> {
    if let Some(rest) = reference.strip_prefix("keychain:") {
        let parts: Vec<&str> = rest.splitn(2, '/').collect();
        if parts.len() == 2 && !parts[0].is_empty() && !parts[1].is_empty() {
            return Ok((parts[0].to_string(), parts[1].to_string()));
        }
    }
    if let Some(rest) = reference.strip_prefix("secret:") {
        if !rest.is_empty() {
            return Ok(("codelattice".to_string(), rest.to_string()));
        }
    }
    Err(SecretError::InvalidRef)
}

/// Secret 存储接口：set 返回 secretRef；get 只在 Rust 侧使用，绝不序列化回前端。
pub trait SecretStore: Send + Sync {
    fn set(&mut self, service: &str, account: &str, secret: &str) -> SecretResult<String>;
    fn get(&self, reference: &str) -> SecretResult<String>;
    fn delete(&mut self, reference: &str) -> SecretResult<()>;
    fn has(&self, reference: &str) -> bool;
}

/// 内存 fake（测试用）：进程内 HashMap。
#[derive(Debug, Default)]
pub struct MemorySecretStore {
    secrets: std::collections::HashMap<String, String>,
}

impl MemorySecretStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl SecretStore for MemorySecretStore {
    fn set(&mut self, service: &str, account: &str, secret: &str) -> SecretResult<String> {
        let reference = format!("keychain:{service}/{account}");
        self.secrets
            .insert(format!("{service}/{account}"), secret.to_string());
        Ok(reference)
    }

    fn get(&self, reference: &str) -> SecretResult<String> {
        let (service, account) = parse_secret_ref(reference)?;
        self.secrets
            .get(&format!("{service}/{account}"))
            .cloned()
            .ok_or(SecretError::NotFound)
    }

    fn delete(&mut self, reference: &str) -> SecretResult<()> {
        let (service, account) = parse_secret_ref(reference)?;
        if self
            .secrets
            .remove(&format!("{service}/{account}"))
            .is_some()
        {
            Ok(())
        } else {
            Err(SecretError::NotFound)
        }
    }

    fn has(&self, reference: &str) -> bool {
        self.get(reference).is_ok()
    }
}

/// 脱敏辅助：写日志前删除认证 header 与明文 Key。
pub fn redact_headers(headers: &str, sensitive: &[&str]) -> String {
    let mut out = String::new();
    for line in headers.lines() {
        let lower = line.to_lowercase();
        if sensitive
            .iter()
            .any(|s| lower.starts_with(&format!("{s}:")))
        {
            out.push_str("Authorization: <redacted>\n");
        } else {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_roundtrip_and_delete() {
        let mut store = MemorySecretStore::new();
        let ref_str = store.set("codelattice", "remote", "sk-abc").unwrap();
        assert_eq!(ref_str, "keychain:codelattice/remote");
        assert_eq!(store.get(&ref_str).unwrap(), "sk-abc");
        assert!(store.has(&ref_str));
        assert!(store.delete(&ref_str).is_ok());
        assert!(!store.has(&ref_str));
    }

    #[test]
    fn invalid_ref_is_rejected() {
        let store = MemorySecretStore::new();
        assert!(store.get("plain-sk").is_err());
        assert_eq!(store.get("plain-sk"), Err(SecretError::InvalidRef));
        assert!(store.get("keychain:only-service").is_err());
    }

    #[test]
    fn secret_ref_never_leaks_plaintext() {
        let mut store = MemorySecretStore::new();
        let reference = store.set("codelattice", "remote", "sk-abc").unwrap();
        // 前端可见的只有 ref 与掩码
        assert!(!reference.contains("sk-abc"));
        assert_eq!(MASKED, "••••••••");
    }

    #[test]
    fn redact_headers_strips_authorization() {
        let raw = "Host: example.com\nAuthorization: Bearer sk-abc\nContent-Type: application/json";
        let out = redact_headers(raw, &["authorization", "x-api-key"]);
        assert!(!out.contains("sk-abc"));
        assert!(out.contains("Authorization: <redacted>"));
        assert!(out.contains("Host: example.com"));
    }
}
