//! The community and SNMPv3 passwords are stored in the Secret Service (GNOME Keyring / KWallet) using `oo7`.
//! They must never be written to settings files or logs.

use std::fmt;
use std::future::Future;

use crate::settings::SecuritySettings;

/// Secret Service attributes always use the release app ID: development and release builds share the passwords (the user's decision).
const APPLICATION: &str = "io.github.noctqira.Anemobit";

/// Stored items (same names as on other platforms).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecretName {
    Community,
    AuthPassword,
    PrivPassword,
}

impl SecretName {
    #[cfg(test)]
    pub const ALL: [SecretName; 3] = [Self::Community, Self::AuthPassword, Self::PrivPassword];

    /// Value of the Secret Service attribute `name`.
    pub fn key(self) -> &'static str {
        match self {
            Self::Community => "community",
            Self::AuthPassword => "authPassword",
            Self::PrivPassword => "privPassword",
        }
    }

    /// Name shown in keyring apps (e.g. "Passwords and Keys").
    fn label(self) -> &'static str {
        match self {
            Self::Community => "Anemobit community",
            Self::AuthPassword => "Anemobit authentication password",
            Self::PrivPassword => "Anemobit privacy password",
        }
    }
}

/// Accessing the Secret Service failed (e.g. no keyring service, or the user declined to unlock).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SecretError(pub String);

impl fmt::Display for SecretError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "secret storage error: {}", self.0)
    }
}

impl std::error::Error for SecretError {}

/// Where passwords are stored, so tests can use an in-memory version (unit tests never touch the user's keyring).
pub trait SecretStore {
    fn get(&self, name: SecretName) -> impl Future<Output = Result<Option<String>, SecretError>>;

    /// An empty string means delete.
    fn set(&self, name: SecretName, value: &str) -> impl Future<Output = Result<(), SecretError>>;
}

/// Accesses the Secret Service with `oo7`.
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyringStore;

impl SecretStore for KeyringStore {
    async fn get(&self, name: SecretName) -> Result<Option<String>, SecretError> {
        let keyring = keyring().await?;
        let items = keyring
            .search_items(&attributes(name))
            .await
            .map_err(error)?;
        let Some(item) = items.first() else {
            return Ok(None);
        };
        if item.is_locked().await.map_err(error)? {
            item.unlock().await.map_err(error)?;
        }
        let secret = item.secret().await.map_err(error)?;
        String::from_utf8(secret.as_bytes().to_vec())
            .map(Some)
            .map_err(|_| SecretError(format!("{} is not valid UTF-8", name.key())))
    }

    async fn set(&self, name: SecretName, value: &str) -> Result<(), SecretError> {
        let keyring = keyring().await?;
        if value.is_empty() {
            keyring.delete(&attributes(name)).await.map_err(error)
        } else {
            keyring
                .create_item(name.label(), &attributes(name), value, true)
                .await
                .map_err(error)
        }
    }
}

/// Connects to the default keyring; if it's locked, the system asks the user to unlock it.
async fn keyring() -> Result<oo7::Keyring, SecretError> {
    let keyring = oo7::Keyring::new().await.map_err(error)?;
    keyring.unlock().await.map_err(error)?;
    Ok(keyring)
}

fn attributes(name: SecretName) -> [(&'static str, &'static str); 2] {
    [("application", APPLICATION), ("name", name.key())]
}

fn error(error: oo7::Error) -> SecretError {
    SecretError(error.to_string())
}

/// Reads the community and passwords from `store` into `security`.
pub async fn load_secrets(
    store: &impl SecretStore,
    security: &mut SecuritySettings,
) -> Result<(), SecretError> {
    security.community = store.get(SecretName::Community).await?.unwrap_or_default();
    security.auth_password = store
        .get(SecretName::AuthPassword)
        .await?
        .unwrap_or_default();
    security.priv_password = store
        .get(SecretName::PrivPassword)
        .await?
        .unwrap_or_default();
    Ok(())
}

/// Writes the community and passwords; empty fields are deleted. Passwords for the other security mode are kept when switching, so switching back is easy.
pub async fn save_secrets(
    store: &impl SecretStore,
    security: &SecuritySettings,
) -> Result<(), SecretError> {
    store
        .set(SecretName::Community, &security.community)
        .await?;
    store
        .set(SecretName::AuthPassword, &security.auth_password)
        .await?;
    store
        .set(SecretName::PrivPassword, &security.priv_password)
        .await
}

#[cfg(test)]
pub(crate) mod tests {
    use std::cell::RefCell;
    use std::collections::HashMap;

    use super::*;

    /// In-memory password store.
    #[derive(Debug, Default)]
    pub(crate) struct MemoryStore {
        pub(crate) items: RefCell<HashMap<SecretName, String>>,
    }

    impl SecretStore for MemoryStore {
        async fn get(&self, name: SecretName) -> Result<Option<String>, SecretError> {
            Ok(self.items.borrow().get(&name).cloned())
        }

        async fn set(&self, name: SecretName, value: &str) -> Result<(), SecretError> {
            let mut items = self.items.borrow_mut();
            if value.is_empty() {
                items.remove(&name);
            } else {
                items.insert(name, value.to_owned());
            }
            Ok(())
        }
    }

    fn run<F: Future>(future: F) -> F::Output {
        gtk::glib::MainContext::new().block_on(future)
    }

    #[test]
    fn save_then_load_round_trips() {
        let store = MemoryStore::default();
        let saved = SecuritySettings {
            community: "public".into(),
            auth_password: "authpass12".into(),
            priv_password: "privpass12".into(),
            ..Default::default()
        };
        run(save_secrets(&store, &saved)).unwrap();
        let mut loaded = SecuritySettings::default();
        run(load_secrets(&store, &mut loaded)).unwrap();
        assert_eq!(loaded, saved);
    }

    #[test]
    fn empty_fields_are_deleted() {
        let store = MemoryStore::default();
        store
            .items
            .borrow_mut()
            .insert(SecretName::PrivPassword, "old".into());
        let security = SecuritySettings {
            community: "public".into(),
            ..Default::default()
        };
        run(save_secrets(&store, &security)).unwrap();
        let items = store.items.borrow();
        assert_eq!(
            items.get(&SecretName::Community).map(String::as_str),
            Some("public")
        );
        assert!(!items.contains_key(&SecretName::PrivPassword));
        assert!(!items.contains_key(&SecretName::AuthPassword));
    }

    #[test]
    fn missing_items_load_as_empty() {
        let mut security = SecuritySettings {
            community: "stale".into(),
            ..Default::default()
        };
        run(load_secrets(&MemoryStore::default(), &mut security)).unwrap();
        assert_eq!(security, SecuritySettings::default());
    }

    #[test]
    fn names_match_other_platforms() {
        let keys: Vec<_> = SecretName::ALL.iter().map(|n| n.key()).collect();
        assert_eq!(keys, ["community", "authPassword", "privPassword"]);
        assert_eq!(APPLICATION, "io.github.noctqira.Anemobit");
    }
}
