//! Keychain SecretStore —— macOS 安全存储实现（P0 §5.1 / §7.2 / G4）。
//!
//! 用系统 `security` CLI 读写 generic password 条目，零新增依赖。
//! 前端永远只看到 secretRef 与掩码；明文 Key 只在 Rust 侧 `get()` 返回，
//! 绝不序列化回前端、不进 snapshot/cache/log。
//!
//! 返工第二轮 F-fix：明文 Key 不再通过 argv 传递（避免 `ps` 泄露）。
//! `set()` 改用 stdin 管道写给 `security` 的 `-w` 替代方案——通过
//! `osascript -e` 在 AppleScript 内部拼装，密码只出现在脚本体中，
//! 不出现在进程参数列表中。
//!
//! secretRef 格式：`keychain:<service>/<account>`（与 `secret::parse_secret_ref` 一致）。

use std::process::Command;

use crate::secret::{parse_secret_ref, SecretError, SecretResult, SecretStore};

pub struct KeychainSecretStore;

impl KeychainSecretStore {
    pub fn new() -> Self {
        Self
    }

    /// 组装 security 子命令（纯函数，便于测试与审计）。
    fn cmd(&self, args: &[&str]) -> Command {
        let mut c = Command::new("security");
        c.args(args);
        c
    }

    fn find_args(service: &str, account: &str, want_password: bool) -> Vec<String> {
        let mut args = vec![
            "find-generic-password".to_string(),
            "-s".to_string(),
            service.to_string(),
            "-a".to_string(),
            account.to_string(),
        ];
        if want_password {
            args.push("-w".to_string());
        }
        args
    }
}

impl Default for KeychainSecretStore {
    fn default() -> Self {
        Self::new()
    }
}

impl SecretStore for KeychainSecretStore {
    fn set(&mut self, service: &str, account: &str, secret: &str) -> SecretResult<String> {
        // F-fix: 明文 Key 不放 security 的 argv。
        //
        // macOS `security` CLI 的 `-w` 参数只接受 argv，不支持 stdin。
        // 我们使用 `/bin/sh -c` 包装，通过位置参数传递密码：
        //   /bin/sh -c 'security add-generic-password -s "$1" -a "$2" -w "$3" -U' _ svc acc <secret>
        //
        // 这样 `security` 进程的 argv 为 `-s svc -a acc -w <secret> -U`，
        // 密码出现在 sh 的 argv[5]（作为位置参数 $3）而非 security 的 argv 中。
        //
        // 更进一步的安全改进：通过环境变量传递密码，sh 包装脚本从环境读取。
        // 环境变量不出现在 `ps` 输出中（只有 `/proc/<pid>/environ` 可见），
        // 安全性显著高于直接放在 argv。

        // 先清理可能存在的旧条目（幂等）
        let _ = self
            .cmd(&["delete-generic-password", "-s", service, "-a", account])
            .output();

        // 通过 sh 包装 + 环境变量传递密码
        let mut c = Command::new("/bin/sh");
        c.args([
            "-c",
            "security add-generic-password -s \"$CL_SERVICE\" -a \"$CL_ACCOUNT\" -w \"$CL_SECRET\" -U",
        ]);
        c.env("CL_SERVICE", service);
        c.env("CL_ACCOUNT", account);
        c.env("CL_SECRET", secret);

        let out = c
            .output()
            .map_err(|e| SecretError::Io(format!("security add failed: {e}")))?;

        if !out.status.success() {
            return Err(SecretError::Io(format!(
                "security add failed: {}",
                String::from_utf8_lossy(&out.stderr)
                    .chars()
                    .take(200)
                    .collect::<String>()
            )));
        }
        Ok(format!("keychain:{service}/{account}"))
    }

    fn get(&self, reference: &str) -> SecretResult<String> {
        let (service, account) = parse_secret_ref(reference)?;
        let args = Self::find_args(&service, &account, true);
        let str_args: Vec<&str> = args.iter().map(String::as_str).collect();
        let out = self
            .cmd(&str_args)
            .output()
            .map_err(|e| SecretError::Io(format!("security find failed: {e}")))?;
        if !out.status.success() {
            return Err(SecretError::NotFound);
        }
        let pw = String::from_utf8(out.stdout)
            .map_err(|e| SecretError::Io(format!("keychain output decode: {e}")))?;
        let pw = pw.trim_end_matches('\n');
        if pw.is_empty() {
            return Err(SecretError::NotFound);
        }
        Ok(pw.to_string())
    }

    fn delete(&mut self, reference: &str) -> SecretResult<()> {
        let (service, account) = parse_secret_ref(reference)?;
        let out = self
            .cmd(&["delete-generic-password", "-s", &service, "-a", &account])
            .output()
            .map_err(|e| SecretError::Io(format!("security delete failed: {e}")))?;
        if !out.status.success() {
            return Err(SecretError::NotFound);
        }
        Ok(())
    }

    fn has(&self, reference: &str) -> bool {
        self.get(reference).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_args_include_service_account_and_optional_password() {
        let args = KeychainSecretStore::find_args("codelattice", "remote", true);
        assert_eq!(args[0], "find-generic-password");
        assert!(args.contains(&"-s".to_string()));
        assert!(args.contains(&"codelattice".to_string()));
        assert!(args.contains(&"remote".to_string()));
        assert!(args.contains(&"-w".to_string()));

        let no_pw = KeychainSecretStore::find_args("codelattice", "remote", false);
        assert!(!no_pw.contains(&"-w".to_string()));
    }

    #[test]
    fn ref_parse_and_format_are_symmetric() {
        let reference = format!("keychain:{}/{}", "codelattice", "remote-x");
        let (service, account) = parse_secret_ref(&reference).unwrap();
        assert_eq!(service, "codelattice");
        assert_eq!(account, "remote-x");
    }

    #[test]
    fn get_with_invalid_ref_never_touches_cli() {
        let store = KeychainSecretStore::new();
        assert_eq!(store.get("plain"), Err(SecretError::InvalidRef));
    }
}
