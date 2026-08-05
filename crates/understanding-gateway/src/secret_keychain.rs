//! macOS Keychain SecretStore（P0 §5.1 / §7.2 / G4）。
//!
//! 生产实现直接调用 Apple Security.framework；不启动 shell/`security` 子进程，
//! 因而明文不会进入 argv。前端只得到 secretRef 与掩码。

use crate::secret::{parse_secret_ref, SecretError, SecretResult, SecretStore};

#[cfg(target_os = "macos")]
use security_framework::passwords::{
    delete_generic_password, generic_password, set_generic_password, PasswordOptions,
};

pub struct KeychainSecretStore;

impl KeychainSecretStore {
    pub fn new() -> Self {
        Self
    }

    #[cfg(not(target_os = "macos"))]
    fn unsupported() -> SecretError {
        SecretError::Io("macOS Keychain is unavailable on this platform".to_string())
    }
}

impl Default for KeychainSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for KeychainSecretStore {
    fn set(&mut self, service: &str, account: &str, secret: &str) -> SecretResult<String> {
        #[cfg(target_os = "macos")]
        {
            set_generic_password(service, account, secret.as_bytes())
                .map_err(|error| SecretError::Io(format!("keychain set failed: {error}")))?;
            Ok(format!("keychain:{service}/{account}"))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (service, account, secret);
            Err(Self::unsupported())
        }
    }

    fn get(&self, reference: &str) -> SecretResult<String> {
        let (service, account) = parse_secret_ref(reference)?;
        #[cfg(target_os = "macos")]
        {
            let bytes = generic_password(PasswordOptions::new_generic_password(&service, &account))
                .map_err(|_| SecretError::NotFound)?;
            let value = String::from_utf8(bytes).map_err(|error| {
                SecretError::Io(format!("keychain value decode failed: {error}"))
            })?;
            if value.is_empty() {
                Err(SecretError::NotFound)
            } else {
                Ok(value)
            }
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (service, account);
            Err(Self::unsupported())
        }
    }

    fn delete(&mut self, reference: &str) -> SecretResult<()> {
        let (service, account) = parse_secret_ref(reference)?;
        #[cfg(target_os = "macos")]
        {
            delete_generic_password(&service, &account).map_err(|_| SecretError::NotFound)
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = (service, account);
            Err(Self::unsupported())
        }
    }

    fn has(&self, reference: &str) -> bool {
        self.get(reference).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ref_parse_and_format_are_symmetric() {
        let reference = format!("keychain:{}/{}", "codelattice", "remote-x");
        let (service, account) = parse_secret_ref(&reference).unwrap();
        assert_eq!(service, "codelattice");
        assert_eq!(account, "remote-x");
    }

    #[test]
    fn get_with_invalid_ref_never_touches_keychain() {
        let store = KeychainSecretStore::new();
        assert_eq!(store.get("plain"), Err(SecretError::InvalidRef));
    }

    #[test]
    fn production_keychain_implementation_never_spawns_a_secret_shell() {
        let source = include_str!("secret_keychain.rs");
        let forbidden = ["Command::new", "(\"/bin/sh\")"].concat();
        assert!(
            !source.contains(&forbidden),
            "Keychain writes must use Security.framework, never a shell carrying plaintext"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "writes and removes a temporary item in the user Keychain"]
    fn macos_keychain_roundtrip_live() {
        let mut store = KeychainSecretStore::new();
        let service = format!("com.codelattice.workbench.test.{}", std::process::id());
        let account = "temporary-roundtrip";
        let secret = "temporary-test-secret";
        let reference = store.set(&service, account, secret).unwrap();
        let result = (|| {
            assert!(store.has(&reference));
            assert_eq!(store.get(&reference).unwrap(), secret);
        })();
        let cleanup = store.delete(&reference);
        assert!(cleanup.is_ok(), "temporary Keychain item cleanup failed");
        result
    }
}
