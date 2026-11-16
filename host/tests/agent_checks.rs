//! The load checks of the agent entries that need the machine (ts-0099-bw): a `read` folder that does not exist, and a
//! `secrets` entry whose keychain item is missing. The folders are made under a `TempRoot` and the keychain is the
//! memory double, so nothing here touches the real home or the real keychain.

mod common;

use collide_core::agent_layers::{AgentInputs, AgentView, agent_layers};
use collide_core::settings::ConfigFile;
use collide_core::yaml::load_yaml;
use collide_host::agent_checks::check_agent_environment;
use collide_host::keychain::{Keychain, KeychainError, MemoryKeychain, Secret};
use common::TempRoot;
use std::fs;
use std::path::Path;

fn agents(user: &str) -> Vec<AgentView> {
    let user: ConfigFile = load_yaml(user.as_bytes(), "config.yaml").unwrap();
    let none = ConfigFile::default();
    agent_layers(&AgentInputs {
        user: &user,
        user_board: &none,
        repo: &none,
    })
    .agents
}

fn check(user: &str, board_root: Option<&Path>, keychain: &dyn Keychain) -> Vec<String> {
    check_agent_environment(&agents(user), board_root, keychain)
        .into_iter()
        .map(|problem| problem.message)
        .collect()
}

/// A keychain that fails the test when a value is read: a load only looks.
struct NeverRead(MemoryKeychain);

impl Keychain for NeverRead {
    fn set(&self, item: &str, secret: &Secret) -> Result<(), KeychainError> {
        self.0.set(item, secret)
    }

    fn contains(&self, item: &str) -> Result<bool, KeychainError> {
        self.0.contains(item)
    }

    fn get(&self, item: &str) -> Result<Secret, KeychainError> {
        panic!("the load read the value of `{item}`")
    }

    fn delete(&self, item: &str) -> Result<(), KeychainError> {
        self.0.delete(item)
    }
}

/// A keychain that cannot be asked.
struct Broken;

impl Keychain for Broken {
    fn set(&self, _: &str, _: &Secret) -> Result<(), KeychainError> {
        unreachable!()
    }

    fn contains(&self, _: &str) -> Result<bool, KeychainError> {
        Err(KeychainError::Unavailable {
            reason: "locked".into(),
        })
    }

    fn get(&self, _: &str) -> Result<Secret, KeychainError> {
        unreachable!()
    }

    fn delete(&self, _: &str) -> Result<(), KeychainError> {
        unreachable!()
    }
}

// ---- read folders ----

fn reading(folders: &[&Path]) -> String {
    let list: Vec<String> = folders
        .iter()
        .map(|folder| format!("{:?}", folder.display().to_string()))
        .collect();
    format!(
        "agents:\n  mine:\n    kind: external\n    read: [{}]\n",
        list.join(", ")
    )
}

#[test]
fn a_read_folder_that_exists_passes() {
    let root = TempRoot::new();
    assert!(check(&reading(&[root.path()]), None, &MemoryKeychain::new()).is_empty());
}

#[test]
fn a_read_folder_that_does_not_exist_fails_naming_the_key_and_the_folder() {
    let root = TempRoot::new();
    let missing = root.join("nope");
    let problems = check(&reading(&[&missing]), None, &MemoryKeychain::new());
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("agents.mine.read"), "{}", problems[0]);
    assert!(
        problems[0].contains(missing.to_str().unwrap()),
        "{}",
        problems[0]
    );
}

#[test]
fn a_file_is_not_a_read_folder() {
    let root = TempRoot::new();
    fs::write(root.join("file"), "x").unwrap();
    let problems = check(
        &reading(&[&root.join("file")]),
        None,
        &MemoryKeychain::new(),
    );
    assert_eq!(problems.len(), 1, "{problems:?}");
}

#[test]
fn every_missing_folder_is_reported() {
    let root = TempRoot::new();
    let problems = check(
        &reading(&[&root.join("a"), root.path(), &root.join("b")]),
        None,
        &MemoryKeychain::new(),
    );
    assert_eq!(problems.len(), 2, "{problems:?}");
}

#[test]
fn the_built_in_entrys_board_root_is_the_boards_root() {
    let root = TempRoot::new();
    assert!(check("", Some(root.path()), &MemoryKeychain::new()).is_empty());
}

#[test]
fn a_board_root_that_is_gone_fails_naming_the_key_and_the_folder_it_expanded_to() {
    let root = TempRoot::new();
    let gone = root.join("gone");
    let problems = check("", Some(&gone), &MemoryKeychain::new());
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(
        problems[0].contains("agents.claude-haiku.read"),
        "{}",
        problems[0]
    );
    assert!(
        problems[0].contains(gone.to_str().unwrap()),
        "{}",
        problems[0]
    );
}

#[test]
fn a_folder_below_the_board_root_is_checked_below_it() {
    let root = TempRoot::new();
    fs::create_dir(root.join("docs")).unwrap();
    let text = "agents:\n  mine:\n    kind: external\n    read: [\"<board root>/docs\", \"<board root>/missing\"]\n";
    let problems = check(text, Some(root.path()), &MemoryKeychain::new());
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("missing"), "{}", problems[0]);
}

#[test]
fn with_no_board_the_board_root_is_not_checked_and_other_folders_still_are() {
    let root = TempRoot::new();
    let text = format!(
        "agents:\n  mine:\n    kind: external\n    read: [\"<board root>\", {:?}]\n",
        root.join("nope").display().to_string()
    );
    let problems = check(&text, None, &MemoryKeychain::new());
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("nope"), "{}", problems[0]);
}

// ---- keychain items ----

const SECRETS: &str =
    "agents:\n  mine:\n    kind: external\n    secrets: { SOME_API_KEY: my-item }\n";

#[test]
fn a_missing_keychain_item_fails_naming_the_item_and_the_key() {
    let problems = check(SECRETS, None, &MemoryKeychain::new());
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("my-item"), "{}", problems[0]);
    assert!(
        problems[0].contains("agents.mine.secrets.SOME_API_KEY"),
        "{}",
        problems[0]
    );
}

#[test]
fn a_present_item_passes_and_its_value_is_not_read() {
    let keychain = NeverRead(MemoryKeychain::new());
    keychain.set("my-item", &Secret::new("canary")).unwrap();
    assert!(check(SECRETS, None, &keychain).is_empty());
}

#[test]
fn a_keychain_that_cannot_be_asked_is_a_problem_naming_the_item_and_the_key() {
    let problems = check(SECRETS, None, &Broken);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(problems[0].contains("my-item"), "{}", problems[0]);
    assert!(
        problems[0].contains("agents.mine.secrets.SOME_API_KEY"),
        "{}",
        problems[0]
    );
}

#[test]
fn an_entry_with_no_secrets_never_asks_the_keychain() {
    assert!(check("", None, &Broken).is_empty());
}

#[test]
fn each_missing_item_is_reported() {
    let text = "agents:\n  mine:\n    kind: external\n    secrets: { A_KEY: one, B_TOKEN: two }\n";
    assert_eq!(check(text, None, &MemoryKeychain::new()).len(), 2);
}
