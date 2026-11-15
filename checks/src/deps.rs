//! The dependency-direction check (0.15; 0.1): `core` depends on no framework, storage or process crate.
//!
//! It reads `Cargo.lock`, so it needs no network and no cargo process, and it sees the whole closure of `core`,
//! including dev-dependencies and crates that a pure-looking dependency pulls in. The lock is a superset of what a
//! given build links, so it is the stricter reading. Two versions of one crate are read as one name.
//!
//! A crate can also spawn a process with the standard library alone, so `core/src` is checked for that too.

use crate::walk;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::io;
use std::path::Path;

const CORE: &str = "collide-core";

/// Why a crate may not be in `core`'s closure, or `None` when it may. The list is the plan's three kinds (0.1):
/// Tauri (the shell framework), SQLite (the store) and crates whose job is to spawn a process. It does not name
/// `tokio`: the plan keeps `core` sans-IO without forbidding the runtime crate, and the lock cannot show whether its
/// `process` feature is on. Add a name here when `core` must stay clear of it.
fn why_forbidden(name: &str) -> Option<&'static str> {
    const TAURI: [&str; 2] = ["wry", "tao"];
    const SQLITE: [&str; 6] = [
        "rusqlite",
        "libsqlite3-sys",
        "sqlite",
        "sqlite3-src",
        "sqlite3-sys",
        "diesel",
    ];
    const PROCESS: [&str; 8] = [
        "duct",
        "subprocess",
        "command-group",
        "async-process",
        "tokio-process",
        "shared_child",
        "portable-pty",
        "pty-process",
    ];
    if name == "tauri" || name.starts_with("tauri-") || TAURI.contains(&name) {
        Some("Tauri")
    } else if name.starts_with("sqlx") || SQLITE.contains(&name) {
        Some("SQLite")
    } else if PROCESS.contains(&name) {
        Some("process-spawning")
    } else {
        None
    }
}

/// package name to the names of its dependencies, from the text of a `Cargo.lock`.
fn parse_lock(lock: &str) -> BTreeMap<String, BTreeSet<String>> {
    let mut packages: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut in_dependencies = false;
    for line in lock.lines() {
        let line = line.trim();
        if line == "[[package]]" {
            current = None;
            in_dependencies = false;
        } else if let Some(name) = line.strip_prefix("name = ") {
            let name = name.trim_matches('"').to_string();
            packages.entry(name.clone()).or_default();
            current = Some(name);
        } else if line.starts_with("dependencies = [") {
            in_dependencies = !line.ends_with(']');
        } else if in_dependencies && line == "]" {
            in_dependencies = false;
        } else if in_dependencies && let Some(package) = &current {
            // An entry is `"name"`, `"name 1.2.3"` or `"name 1.2.3 (source)"`.
            let entry = line.trim_end_matches(',').trim_matches('"');
            if let Some(name) = entry.split(' ').next().filter(|n| !n.is_empty()) {
                packages
                    .get_mut(package)
                    .expect("inserted with the name")
                    .insert(name.to_string());
            }
        }
    }
    packages
}

/// One message per forbidden crate in `core`'s closure, each showing the chain that reaches it. An empty list means
/// `core` is clean. A lock with no `core` package is not clean: the caller sees an empty list only when `core` is
/// there, so use `check` to tell the two apart.
pub fn forbidden_in_core(lock: &str) -> Vec<String> {
    let packages = parse_lock(lock);
    let mut found = Vec::new();
    let mut seen = BTreeSet::from([CORE.to_string()]);
    let mut queue = VecDeque::from([vec![CORE.to_string()]]);
    while let Some(chain) = queue.pop_front() {
        let last = chain.last().expect("a chain is never empty");
        for dependency in packages.get(last).into_iter().flatten() {
            if !seen.insert(dependency.clone()) {
                continue;
            }
            let mut next = chain.clone();
            next.push(dependency.clone());
            if let Some(why) = why_forbidden(dependency) {
                found.push(format!("core reaches a {why} crate: {}", next.join(" -> ")));
            }
            queue.push_back(next);
        }
    }
    found
}

/// The forbidden-dependency messages for `root`'s `Cargo.lock`, then one `path:line:` message per use of
/// `std::process` in `core/src`. An error when the lock cannot be read or has no `core` package.
pub fn check(root: &Path) -> io::Result<Vec<String>> {
    let lock = std::fs::read_to_string(root.join("Cargo.lock"))?;
    if !parse_lock(&lock).contains_key(CORE) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Cargo.lock has no {CORE} package"),
        ));
    }
    let mut found = forbidden_in_core(&lock);
    let spawn = ["std::", "process"].concat();
    let spawn_use = ["process::", "Command"].concat();
    for entry in walk::files(&root.join("core/src"), &["rs"], &[])? {
        for (index, line) in std::fs::read_to_string(&entry.path)?.lines().enumerate() {
            if line.contains(&spawn) || line.contains(&spawn_use) {
                found.push(format!(
                    "core/src/{}:{}: core spawns a process through the standard library",
                    entry.rel,
                    index + 1
                ));
            }
        }
    }
    Ok(found)
}
