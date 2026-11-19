use super::McpError;
use std::collections::HashMap;
use std::sync::Mutex;

/// Storage for secret values. The only production implementation is the macOS Keychain.
pub trait SecretStore: Send + Sync {
    fn get(&self, account: &str) -> Result<Option<String>, McpError>;
    fn set(&self, account: &str, value: &str) -> Result<(), McpError>;
    fn delete(&self, account: &str) -> Result<(), McpError>;
}

/// Keychain account for one secret of one server.
pub fn account(server_id: &str, secret_name: &str) -> String {
    format!("{server_id}:{secret_name}")
}

pub const KEYCHAIN_SERVICE: &str = "dev.naurix.app.mcp";

/// In-memory store for tests and tooling. Never used by the app.
#[derive(Default)]
pub struct MemoryStore(Mutex<HashMap<String, String>>);

impl SecretStore for MemoryStore {
    fn get(&self, a: &str) -> Result<Option<String>, McpError> {
        Ok(self.0.lock().unwrap().get(a).cloned())
    }
    fn set(&self, a: &str, v: &str) -> Result<(), McpError> {
        self.0.lock().unwrap().insert(a.into(), v.into());
        Ok(())
    }
    fn delete(&self, a: &str) -> Result<(), McpError> {
        self.0.lock().unwrap().remove(a);
        Ok(())
    }
}

/// The OS credential store: macOS Keychain, Linux Secret Service, Windows Credential Manager
/// (generic passwords under [`KEYCHAIN_SERVICE`]).
#[derive(Default)]
pub struct KeychainStore;

#[cfg(target_os = "macos")]
impl SecretStore for KeychainStore {
    fn get(&self, a: &str) -> Result<Option<String>, McpError> {
        use security_framework::passwords::get_generic_password;
        match get_generic_password(KEYCHAIN_SERVICE, a) {
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            // errSecItemNotFound
            Err(e) if e.code() == -25300 => Ok(None),
            Err(e) => Err(McpError::Secret(e.to_string())),
        }
    }
    fn set(&self, a: &str, v: &str) -> Result<(), McpError> {
        security_framework::passwords::set_generic_password(KEYCHAIN_SERVICE, a, v.as_bytes())
            .map_err(|e| McpError::Secret(e.to_string()))
    }
    fn delete(&self, a: &str) -> Result<(), McpError> {
        match security_framework::passwords::delete_generic_password(KEYCHAIN_SERVICE, a) {
            Ok(()) => Ok(()),
            Err(e) if e.code() == -25300 => Ok(()),
            Err(e) => Err(McpError::Secret(e.to_string())),
        }
    }
}

/// Secret Service (Linux) and Credential Manager (Windows) through the `keyring` crate.
#[cfg(any(target_os = "linux", windows))]
mod os_store {
    use super::{McpError, SecretStore, KEYCHAIN_SERVICE};

    fn entry(a: &str) -> Result<keyring::Entry, McpError> {
        keyring::Entry::new(KEYCHAIN_SERVICE, a).map_err(err)
    }

    fn err(e: keyring::Error) -> McpError {
        let hint = if cfg!(target_os = "linux") {
            " Start or unlock a Secret Service keyring (GNOME Keyring or KWallet) and try again."
        } else {
            ""
        };
        McpError::Secret(format!("{e}.{hint}"))
    }

    impl SecretStore for super::KeychainStore {
        fn get(&self, a: &str) -> Result<Option<String>, McpError> {
            match entry(a)?.get_password() {
                Ok(v) => Ok(Some(v)),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(err(e)),
            }
        }
        fn set(&self, a: &str, v: &str) -> Result<(), McpError> {
            entry(a)?.set_password(v).map_err(err)
        }
        fn delete(&self, a: &str) -> Result<(), McpError> {
            match entry(a)?.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(err(e)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn memory_store_roundtrip() {
        let s = MemoryStore::default();
        assert_eq!(s.get("a").unwrap(), None);
        s.set("a", "v").unwrap();
        assert_eq!(s.get("a").unwrap().as_deref(), Some("v"));
        s.delete("a").unwrap();
        s.delete("a").unwrap();
        assert_eq!(s.get("a").unwrap(), None);
    }

    /// Talks to the real login Keychain. Run on purpose: `cargo test -p naurix-core keychain -- --ignored`.
    #[test]
    #[ignore = "uses the real macOS Keychain"]
    fn keychain_roundtrip() {
        let k = KeychainStore;
        let acct = format!("naurix-test-{}:SECRET", std::process::id());
        assert_eq!(k.get(&acct).unwrap(), None);
        k.set(&acct, "s3cret-é").unwrap();
        assert_eq!(k.get(&acct).unwrap().as_deref(), Some("s3cret-é"));
        k.set(&acct, "updated").unwrap();
        assert_eq!(k.get(&acct).unwrap().as_deref(), Some("updated"));
        k.delete(&acct).unwrap();
        assert_eq!(k.get(&acct).unwrap(), None);
    }
}
