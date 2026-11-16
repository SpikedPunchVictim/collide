//! The keychain port (ts-0099-bw; 0.7, Secrets): where Collide keeps a secret, behind one trait.
//!
//! A secret lives in the OS keychain and nowhere else: not in a settings file, an argument, a log or a journal. This
//! module is the port and its stores:
//! - [`Keychain`]: `set`, `contains`, `get` and `delete`, by the name of an item;
//! - [`MemoryKeychain`]: the double every test uses;
//! - [`MacKeychain`]: the macOS store, on `security-framework` (owner, 2026-11-16), the only code that names it;
//! - [`system_keychain`]: the store a daemon uses when it runs for a person.
//!
//! Items live under one service name, [`SERVICE`]. A pack's secret is the item [`pack_item`] names; an agent's
//! reference (`agents.<id>.secrets.<ENV>`) is the item name the user wrote. [`Secret`] never prints its value, and no
//! error here holds one. The endpoint token ([`crate::ui_tools::KeychainTokens`], Stage 6b) and the secrets of a launch
//! use this port; the load checks use only `contains`, so a value is not read until it is launched with.

use std::collections::BTreeMap;
use std::fmt;
use std::sync::{Arc, Mutex, PoisonError};

/// The service name every item is kept under.
pub const SERVICE: &str = "collide";

/// A secret's value. It prints as `<secret>` in `Debug` and `Display`, so a log line that names one holds nothing;
/// [`Secret::expose`] is the one way to the text.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Secret {
        Secret(value.into())
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<secret>")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<secret>")
    }
}

/// Why a store did not do what it was asked. Each names the item and never holds a value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeychainError {
    /// `get` of an item that is not there.
    NotFound { item: String },
    /// There is no store on this platform.
    Unavailable { reason: String },
    /// The store refused, or failed. `what` is what was being done.
    Failed {
        item: String,
        what: &'static str,
        reason: String,
    },
}

impl fmt::Display for KeychainError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeychainError::NotFound { item } => write!(f, "the keychain has no item `{item}`"),
            KeychainError::Unavailable { reason } => write!(f, "there is no keychain: {reason}"),
            KeychainError::Failed { item, what, reason } => {
                write!(
                    f,
                    "the keychain could not {what} the item `{item}`: {reason}"
                )
            }
        }
    }
}

impl std::error::Error for KeychainError {}

/// A store of secrets by item name. One value per item.
pub trait Keychain: Send + Sync {
    /// Keeps `secret` under `item`, in place of what was there.
    fn set(&self, item: &str, secret: &Secret) -> Result<(), KeychainError>;
    /// Whether `item` is there. It does not read the value.
    fn contains(&self, item: &str) -> Result<bool, KeychainError>;
    /// The value of `item`, or [`KeychainError::NotFound`] naming it.
    fn get(&self, item: &str) -> Result<Secret, KeychainError>;
    /// Removes `item`. An item that is not there is not an error.
    fn delete(&self, item: &str) -> Result<(), KeychainError>;
}

/// The item a pack's secret setting is kept under: `pack.<pack>.<key>`.
pub fn pack_item(pack: &str, key: &str) -> String {
    format!("pack.{pack}.{key}")
}

/// A store in memory, for tests. Each one is its own store.
#[derive(Default)]
pub struct MemoryKeychain {
    items: Mutex<BTreeMap<String, Secret>>,
}

impl MemoryKeychain {
    pub fn new() -> MemoryKeychain {
        MemoryKeychain::default()
    }

    fn items(&self) -> std::sync::MutexGuard<'_, BTreeMap<String, Secret>> {
        self.items.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl Keychain for MemoryKeychain {
    fn set(&self, item: &str, secret: &Secret) -> Result<(), KeychainError> {
        self.items().insert(item.to_string(), secret.clone());
        Ok(())
    }

    fn contains(&self, item: &str) -> Result<bool, KeychainError> {
        Ok(self.items().contains_key(item))
    }

    fn get(&self, item: &str) -> Result<Secret, KeychainError> {
        self.items()
            .get(item)
            .cloned()
            .ok_or_else(|| KeychainError::NotFound {
                item: item.to_string(),
            })
    }

    fn delete(&self, item: &str) -> Result<(), KeychainError> {
        self.items().remove(item);
        Ok(())
    }
}

/// The store of a platform with no keychain port yet: every call says so.
#[cfg(not(target_os = "macos"))]
pub struct UnavailableKeychain;

#[cfg(not(target_os = "macos"))]
impl UnavailableKeychain {
    fn unavailable() -> KeychainError {
        KeychainError::Unavailable {
            reason: "Collide has a keychain store for macOS only, so far".to_string(),
        }
    }
}

#[cfg(not(target_os = "macos"))]
impl Keychain for UnavailableKeychain {
    fn set(&self, _: &str, _: &Secret) -> Result<(), KeychainError> {
        Err(UnavailableKeychain::unavailable())
    }

    fn contains(&self, _: &str) -> Result<bool, KeychainError> {
        Err(UnavailableKeychain::unavailable())
    }

    fn get(&self, _: &str) -> Result<Secret, KeychainError> {
        Err(UnavailableKeychain::unavailable())
    }

    fn delete(&self, _: &str) -> Result<(), KeychainError> {
        Err(UnavailableKeychain::unavailable())
    }
}

/// The store a daemon serves with: a store in memory when the daemon runs under an overridden root
/// (`COLLIDE_DAEMON_ROOT`, as every test and trial does, so none of them reaches the OS keychain), and
/// [`system_keychain`] otherwise. `root_override` is the value of that variable.
pub fn keychain_for_root(root_override: Option<&str>) -> Arc<dyn Keychain> {
    match root_override {
        Some(_) => Arc::new(MemoryKeychain::new()),
        None => system_keychain(),
    }
}

/// The store a daemon uses when it runs for a person: the macOS keychain under [`SERVICE`]. Nothing else calls it,
/// and no test does ([`crate::daemon::DaemonOptions::new`] starts with a memory store).
pub fn system_keychain() -> Arc<dyn Keychain> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(MacKeychain::with_service(SERVICE))
    }
    #[cfg(not(target_os = "macos"))]
    {
        Arc::new(UnavailableKeychain)
    }
}

/// The macOS keychain, as generic passwords under one service name.
#[cfg(target_os = "macos")]
pub struct MacKeychain {
    service: String,
}

#[cfg(target_os = "macos")]
impl MacKeychain {
    pub fn with_service(service: &str) -> MacKeychain {
        MacKeychain {
            service: service.to_string(),
        }
    }
}

/// The code the keychain gives for an item that is not there (`errSecItemNotFound`).
#[cfg(target_os = "macos")]
const ITEM_NOT_FOUND: i32 = -25300;

#[cfg(target_os = "macos")]
impl MacKeychain {
    fn failed(
        item: &str,
        what: &'static str,
        error: security_framework::base::Error,
    ) -> KeychainError {
        KeychainError::Failed {
            item: item.to_string(),
            what,
            reason: error.to_string(),
        }
    }
}

#[cfg(target_os = "macos")]
impl Keychain for MacKeychain {
    fn set(&self, item: &str, secret: &Secret) -> Result<(), KeychainError> {
        security_framework::passwords::set_generic_password(
            &self.service,
            item,
            secret.expose().as_bytes(),
        )
        .map_err(|error| MacKeychain::failed(item, "store", error))
    }

    fn contains(&self, item: &str) -> Result<bool, KeychainError> {
        use security_framework::item::{ItemClass, ItemSearchOptions, Limit};
        // The attributes only: the secret is not read, so a look does not ask the person to allow anything.
        let found = ItemSearchOptions::new()
            .class(ItemClass::generic_password())
            .service(&self.service)
            .account(item)
            .load_attributes(true)
            .limit(Limit::Max(1))
            .search();
        match found {
            Ok(found) => Ok(!found.is_empty()),
            Err(error) if error.code() == ITEM_NOT_FOUND => Ok(false),
            Err(error) => Err(MacKeychain::failed(item, "look up", error)),
        }
    }

    fn get(&self, item: &str) -> Result<Secret, KeychainError> {
        match security_framework::passwords::get_generic_password(&self.service, item) {
            Ok(bytes) => {
                String::from_utf8(bytes)
                    .map(Secret::new)
                    .map_err(|_| KeychainError::Failed {
                        item: item.to_string(),
                        what: "read",
                        reason: "the stored value is not text".to_string(),
                    })
            }
            Err(error) if error.code() == ITEM_NOT_FOUND => Err(KeychainError::NotFound {
                item: item.to_string(),
            }),
            Err(error) => Err(MacKeychain::failed(item, "read", error)),
        }
    }

    fn delete(&self, item: &str) -> Result<(), KeychainError> {
        match security_framework::passwords::delete_generic_password(&self.service, item) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == ITEM_NOT_FOUND => Ok(()),
            Err(error) => Err(MacKeychain::failed(item, "delete", error)),
        }
    }
}
