//! The store (0.2, Durability; Stage 3a): SQLite for state and the event log, in one directory under the daemon's.
//! This module is the shape of it. No SQLite crate is a dependency yet, so [`Store`] holds a path and the
//! connection arrives with Stage 3a.
//!
//! What 0.2 requires of the store, which the signatures carry:
//!
//! - **A store that cannot be read is moved aside and reported, never overwritten** ([`quarantine`],
//!   [`StoreError::Quarantined`]). The same rule holds for `exit.json` in the shim.
//! - **State is saved without holding a lock that other work waits on**, so [`Store`] methods take `&self`.
//! - **Logging never panics**, and a full disk is tested with a writer that fails with `ENOSPC` on demand: the event
//!   log is written through [`std::io::Write`], and a failed write is a [`StoreError::Io`], not a panic.
//! - **Migrations are numbered and run in order** ([`Migration`]), and a store newer than the build is refused
//!   rather than downgraded.
//!
//! The events written are `core`'s ([`collide_core::events::Event`]): one line of the log each.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `store: open and migrate` | [`Store::open`] | opens the database, runs the migrations that are due |
//! | `store: event log` | [`Store::append_event`], [`Store::events_since`] | the event log's append and read |
//! | `store: quarantine` | [`quarantine`] | moves an unreadable store or record aside, with a report |

use collide_core::events::Event;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

/// One schema change. `version` counts from 1 with no gap, and `sql` is the statements that take the store from the
/// version before to this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// The migrations, in order. Stage 3a writes the first; the skeleton fixes no table.
pub const MIGRATIONS: [Migration; 0] = [];

/// Why the store could not do what it was asked.
#[derive(Debug)]
pub enum StoreError {
    /// The file could not be read as a store. It was moved to `moved_to`, left whole, and the store is empty.
    Quarantined { moved_to: PathBuf, reason: String },
    /// The store was written by a newer build: its version is above the newest migration this build knows.
    TooNew { found: u32, known: u32 },
    /// A read or write failed, for example on a full disk. `context` says which.
    Io { context: String, source: io::Error },
}

impl fmt::Display for StoreError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StoreError::Quarantined { moved_to, reason } => write!(
                f,
                "the store could not be read ({reason}); it was moved to {}",
                moved_to.display()
            ),
            StoreError::TooNew { found, known } => write!(
                f,
                "the store is at version {found}, and this build knows up to version {known}"
            ),
            StoreError::Io { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// The store of one daemon.
#[derive(Debug)]
pub struct Store {
    /// The directory the database file and the event log are in.
    pub dir: PathBuf,
}

impl Store {
    /// Opens the store in `dir`, making it when it is not there and running the [`MIGRATIONS`] that are due, each in
    /// one transaction. A file that cannot be read is moved aside ([`quarantine`]) and reported, and the open fails
    /// with [`StoreError::Quarantined`] so the caller says so.
    pub fn open(dir: &Path) -> Result<Store, StoreError> {
        let _ = dir;
        todo!("store: open and migrate")
    }

    /// Appends one event to the event log. `event.seq` is the next number, and a gap or a repeat is refused.
    pub fn append_event(&self, event: &Event) -> Result<(), StoreError> {
        let _ = event;
        todo!("store: event log")
    }

    /// The events with `seq` above `after`, in order, for a client that reconnects.
    pub fn events_since(&self, after: u64) -> Result<Vec<Event>, StoreError> {
        let _ = after;
        todo!("store: event log")
    }
}

/// Moves `path` aside, to a name beside it that holds the time and the reason it was moved, and returns that name.
/// Nothing is deleted or overwritten, and the move is reported to the caller so that it reaches the log and the
/// Queue.
pub fn quarantine(path: &Path, reason: &str) -> Result<PathBuf, StoreError> {
    let _ = (path, reason);
    todo!("store: quarantine")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_numbered_from_one_without_a_gap() {
        for (index, migration) in MIGRATIONS.iter().enumerate() {
            assert_eq!(migration.version as usize, index + 1, "{}", migration.name);
        }
    }

    #[test]
    fn a_quarantine_error_names_where_the_store_went() {
        let error = StoreError::Quarantined {
            moved_to: PathBuf::from("/x/store.db.unreadable-1"),
            reason: "not a database".to_string(),
        }
        .to_string();
        assert!(error.contains("/x/store.db.unreadable-1") && error.contains("not a database"));
    }

    #[test]
    fn a_store_from_a_newer_build_is_described_by_both_versions() {
        let error = StoreError::TooNew { found: 4, known: 2 }.to_string();
        assert!(error.contains('4') && error.contains('2'), "{error}");
    }

    #[test]
    #[should_panic(expected = "store: open and migrate")]
    fn opening_is_a_stub_tagged_for_stage_3a() {
        let _ = Store::open(Path::new("/nonexistent"));
    }
}
