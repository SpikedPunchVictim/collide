//! The secret store (ts-0099-bw): the secret type that does not print, the item names, and the contract every store
//! meets (set, contains, get, replace, delete, a missing item), run over the memory double. The real store meets the
//! same contract in one test that runs only with `COLLIDE_REAL_KEYCHAIN=1` (on macOS), under a service name of its own
//! and with its items deleted at the end. No other test touches a real keychain.

use collide_host::secret_store::{
    MemorySecretStore, Secret, SecretStore, SecretStoreError, pack_item, secret_store_for_root,
};

/// What every store does. `item` is one the store does not hold when this starts.
fn meets_the_contract(store: &dyn SecretStore, item: &str) {
    assert_eq!(
        store.contains(item),
        Ok(false),
        "a missing item is not contained"
    );
    assert_eq!(
        store.get(item).unwrap_err(),
        SecretStoreError::NotFound {
            item: item.to_string()
        },
        "a missing item is not found, and the error names it"
    );
    assert_eq!(
        store.delete(item),
        Ok(()),
        "deleting a missing item is not an error"
    );

    store.set(item, &Secret::new("first-value")).unwrap();
    assert_eq!(store.contains(item), Ok(true));
    assert_eq!(store.get(item).unwrap().expose(), "first-value");

    store.set(item, &Secret::new("second-value")).unwrap();
    assert_eq!(
        store.get(item).unwrap().expose(),
        "second-value",
        "a second set replaces the first"
    );

    store.delete(item).unwrap();
    assert_eq!(store.contains(item), Ok(false));
    assert!(matches!(
        store.get(item),
        Err(SecretStoreError::NotFound { .. })
    ));
}

// ---- the secret type ----

#[test]
fn a_secret_prints_as_secret_in_debug() {
    assert_eq!(format!("{:?}", Secret::new("hunter2")), "<secret>");
}

#[test]
fn a_secret_prints_as_secret_in_display() {
    assert_eq!(Secret::new("hunter2").to_string(), "<secret>");
}

#[test]
fn a_secret_inside_another_value_prints_as_secret() {
    #[derive(Debug)]
    #[allow(dead_code)]
    struct Holder {
        name: &'static str,
        value: Secret,
    }
    let text = format!(
        "{:?}",
        Holder {
            name: "n",
            value: Secret::new("hunter2")
        }
    );
    assert!(text.contains("<secret>"), "{text}");
    assert!(!text.contains("hunter2"), "{text}");
}

#[test]
fn a_secret_gives_its_value_only_through_expose() {
    assert_eq!(Secret::new("hunter2").expose(), "hunter2");
}

// ---- the item names ----

#[test]
fn a_pack_secret_is_kept_under_pack_dot_pack_dot_key() {
    assert_eq!(pack_item("ferry", "notifyHook"), "pack.ferry.notifyHook");
}

// ---- the memory double ----

#[test]
fn the_memory_keychain_meets_the_contract() {
    meets_the_contract(&MemorySecretStore::new(), "pack.ferry.notifyHook");
}

#[test]
fn items_do_not_see_each_other() {
    let store = MemorySecretStore::new();
    store.set("a", &Secret::new("1")).unwrap();
    assert_eq!(store.contains("b"), Ok(false));
    store.set("b", &Secret::new("2")).unwrap();
    store.delete("a").unwrap();
    assert_eq!(store.get("b").unwrap().expose(), "2");
}

#[test]
fn an_error_names_the_item_and_never_holds_a_value() {
    let text = SecretStoreError::NotFound {
        item: "pack.ferry.notifyHook".into(),
    }
    .to_string();
    assert!(text.contains("pack.ferry.notifyHook"), "{text}");
}

#[test]
fn two_memory_keychains_are_two_stores() {
    let (one, two) = (MemorySecretStore::new(), MemorySecretStore::new());
    one.set("a", &Secret::new("1")).unwrap();
    assert_eq!(two.contains("a"), Ok(false));
}

#[test]
fn a_daemon_under_an_overridden_root_gets_a_store_in_memory() {
    // The system store is the real keychain, so only the overridden case is run here.
    let store = secret_store_for_root(Some("/tmp/any-root"));
    meets_the_contract(store.as_ref(), "pack.ferry.notifyHook");
    assert_eq!(
        secret_store_for_root(Some("/tmp/any-root")).contains("pack.ferry.notifyHook"),
        Ok(false)
    );
}

// ---- the real store, for the owner ----

/// The reason the real-keychain test does not run, or `None` when it does.
#[cfg(target_os = "macos")]
fn real_keychain_skip() -> Option<&'static str> {
    (std::env::var("COLLIDE_REAL_KEYCHAIN").as_deref() != Ok("1"))
        .then_some("COLLIDE_REAL_KEYCHAIN=1 is not set")
}

/// Deletes the item when the test ends, also when it panics.
#[cfg(target_os = "macos")]
struct DeleteAtEnd<'a>(&'a dyn SecretStore, String);

#[cfg(target_os = "macos")]
impl Drop for DeleteAtEnd<'_> {
    fn drop(&mut self) {
        let _ = self.0.delete(&self.1);
    }
}

#[cfg(target_os = "macos")]
#[test]
fn the_real_keychain_meets_the_contract() {
    if let Some(reason) = real_keychain_skip() {
        eprintln!("skipped: {reason}");
        return;
    }
    // A service name of this test's own, never `collide`, so no item of a real install is read or replaced.
    let service = format!("dev.collide.test.{}", std::process::id());
    let store = collide_host::secret_store::MacKeychain::with_service(&service);
    let item = "contract-item";
    let _cleanup = DeleteAtEnd(&store, item.to_string());
    meets_the_contract(&store, item);
}
