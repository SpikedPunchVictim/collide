//! The boards and scratch tasks in the store (ts-0075-hj; 0.1, 0.10). The tables are migration 5 of the store
//! ([`crate::store::MIGRATIONS`]); this file is the writes and reads on them. What a scratch task is, and which
//! statuses it may have, is [`crate::scratch`]'s; this is the SQL.
//!
//! - A board keeps one name for its life, and the store gives each checkout root its name the first time it is
//!   opened. A board made before ts-0191-nv is `board-<n>`, the number of its row, and keeps that name. A new board is
//!   `board-` and six random letters, so a lost store never gives a name that a folder of an earlier store (its views,
//!   its pack data) already has: the daemon says which names are taken on disk, and the store checks its own.
//! - A board records the root commits of its repository the first time one is seen (`root_commits`): `board.locate`
//!   accepts a folder only when it shares one.
//! - A board keeps the time it was last opened or picked (`last_opened_ms`, migration 14; ts-0218-sc): a window opens
//!   on the board with the greatest. No two boards share a time, so the latest is never in doubt. A board never
//!   opened since the column was added has none.
//! - A board is removed by a time beside it (`removed_ms`, migration 15; ts-0222-gx), which hides it and keeps its
//!   row. [`Store::open_board`] on its root clears the time. Its name stays taken, so no new board gets it.
//! - A scratch task is numbered inside its board, from 1. The number and the insert are one transaction, so two writers
//!   never take the same number.

use crate::store::{Store, StoreError, sql_error};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::collections::hash_map::RandomState;
use std::hash::{BuildHasher, Hasher};
use std::io;

/// The text a board's name starts with. After it comes the row's number (a board made before ts-0191-nv) or the
/// board's random letters.
const BOARD_PREFIX: &str = "board-";

/// How many random letters a new board's name ends with. They are letters only, so a new name is never a number and
/// never the name of an older board's row.
const SUFFIX_LEN: usize = 6;

/// How many names a new board tries before the store gives up. Each is taken with a chance of about 1 in 300 million.
const NAME_TRIES: usize = 64;

/// One board as the store holds it: its name, the top level of its checkout, the root commits of its repository when
/// they were recorded (`None` when they were not), and when it was last opened or picked in milliseconds of the wall
/// clock (`None` for a board that has not been since ts-0218-sc). No two boards have the same time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBoard {
    pub id: String,
    pub root: String,
    pub root_commits: Option<Vec<String>>,
    pub last_opened_ms: Option<u64>,
}

/// What removing a board found (ts-0222-gx).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    /// The board was listed and is now removed.
    Removed,
    /// The board was removed before.
    AlreadyRemoved,
    /// The store has no board of that name.
    Unknown,
}

/// One scratch task as the store holds it. `status` is the text of its category.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScratchRow {
    pub num: u64,
    pub title: String,
    pub prompt: String,
    pub status: String,
    pub version: u64,
}

fn board_name(number: i64) -> String {
    format!("{BOARD_PREFIX}{number}")
}

/// The number in a board's name, or `None` for text that is not the name of an older board.
fn board_number(name: &str) -> Option<i64> {
    name.strip_prefix(BOARD_PREFIX)?.parse().ok()
}

/// A board's name from its row: its own, or `board-<n>` for a board made before names were stored.
fn name_of(number: i64, own: Option<String>) -> String {
    own.unwrap_or_else(|| board_name(number))
}

/// The row of the board named `name`: the row whose own name it is, or row `n` for `board-<n>` when that row has no
/// name of its own. `None` when the store has no such board.
fn row_of(conn: &Connection, name: &str) -> rusqlite::Result<Option<i64>> {
    conn.query_row(
        "SELECT id FROM boards WHERE name = ?1 OR (name IS NULL AND id = ?2)",
        params![name, board_number(name)],
        |row| row.get(0),
    )
    .optional()
}

/// Six random letters, from a hasher the operating system seeds: `RandomState` is the one source of randomness in
/// `std`, and a name only has to differ from the others, not be secret.
fn random_suffix() -> String {
    let mut bits = RandomState::new().build_hasher().finish();
    (0..SUFFIX_LEN)
        .map(|_| {
            let letter = char::from(b'a' + (bits % 26) as u8);
            bits /= 26;
            letter
        })
        .collect()
}

/// The root commits as a text, one per line, and back.
fn commits_text(commits: &[String]) -> String {
    commits.join("\n")
}

fn commits_of(text: Option<String>) -> Option<Vec<String>> {
    text.map(|text| text.lines().map(str::to_string).collect())
}

fn task_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<ScratchRow> {
    Ok(ScratchRow {
        num: row.get::<_, i64>(0)? as u64,
        title: row.get(1)?,
        prompt: row.get(2)?,
        status: row.get(3)?,
        version: row.get::<_, i64>(4)? as u64,
    })
}

const TASK_COLUMNS: &str = "num, title, prompt, status, version";

impl Store {
    /// The board of the checkout at `root`: the one the store has, or a new one with a random name. Opening the same
    /// root again returns the same name. `taken` says whether a name is already used outside the store (a folder of an
    /// earlier store), and a new board never takes one.
    pub fn open_board(
        &self,
        root: &str,
        taken: &dyn Fn(&str) -> bool,
    ) -> Result<String, StoreError> {
        self.open_board_with(root, taken, &mut random_suffix)
    }

    /// [`Store::open_board`] with the letters a new name gets, so a test can name them.
    pub fn open_board_with(
        &self,
        root: &str,
        taken: &dyn Fn(&str) -> bool,
        suffix: &mut dyn FnMut() -> String,
    ) -> Result<String, StoreError> {
        let context = "opening a board";
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| sql_error(context, e))?;
        let known: Option<(i64, Option<String>)> = tx
            .query_row(
                "SELECT id, name FROM boards WHERE root = ?1",
                [root],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .map_err(|e| sql_error(context, e))?;
        if let Some((number, own)) = known {
            // A removed board comes back by its root, as the same board (ts-0222-gx).
            tx.execute(
                "UPDATE boards SET removed_ms = NULL WHERE id = ?1",
                [number],
            )
            .map_err(|e| sql_error(context, e))?;
            tx.commit().map_err(|e| sql_error(context, e))?;
            return Ok(name_of(number, own));
        }
        for _ in 0..NAME_TRIES {
            let name = format!("{BOARD_PREFIX}{}", suffix());
            let in_store = row_of(&tx, &name)
                .map_err(|e| sql_error(context, e))?
                .is_some();
            if in_store || taken(&name) {
                continue;
            }
            tx.execute(
                "INSERT INTO boards (root, name) VALUES (?1, ?2)",
                params![root, name],
            )
            .map_err(|e| sql_error(context, e))?;
            tx.commit().map_err(|e| sql_error(context, e))?;
            return Ok(name);
        }
        Err(StoreError::Io {
            context: context.to_string(),
            source: io::Error::other(format!(
                "no free board name after {NAME_TRIES} tries; every one was taken"
            )),
        })
    }

    /// Every listed board, in the order they were first opened. A removed board (ts-0222-gx) is not one.
    pub fn boards(&self) -> Result<Vec<StoredBoard>, StoreError> {
        let context = "reading the boards";
        let conn = self.connect()?;
        let mut statement = conn
            .prepare(
                "SELECT id, name, root, root_commits, last_opened_ms FROM boards WHERE removed_ms IS NULL ORDER BY id",
            )
            .map_err(|e| sql_error(context, e))?;
        let rows = statement
            .query_map([], |row| {
                Ok(StoredBoard {
                    id: name_of(row.get(0)?, row.get(1)?),
                    root: row.get(2)?,
                    root_commits: commits_of(row.get(3)?),
                    last_opened_ms: row.get::<_, Option<i64>>(4)?.map(|ms| ms as u64),
                })
            })
            .map_err(|e| sql_error(context, e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| sql_error(context, e))
    }

    /// Removes `board` from the list (ts-0222-gx): the row stays, with the time it was removed, and so do its scratch
    /// tasks and everything keyed by its name. Opening its root again brings it back.
    pub fn remove_board(&self, board: &str, now_ms: u64) -> Result<Removal, StoreError> {
        let context = "removing a board";
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| sql_error(context, e))?;
        let Some(row) = row_of(&tx, board).map_err(|e| sql_error(context, e))? else {
            return Ok(Removal::Unknown);
        };
        let now = i64::try_from(now_ms).unwrap_or(i64::MAX);
        let changed = tx
            .execute(
                "UPDATE boards SET removed_ms = ?2 WHERE id = ?1 AND removed_ms IS NULL",
                params![row, now],
            )
            .map_err(|e| sql_error(context, e))?;
        tx.commit().map_err(|e| sql_error(context, e))?;
        Ok(if changed == 0 {
            Removal::AlreadyRemoved
        } else {
            Removal::Removed
        })
    }

    /// Whether `board` is a board the store has and has removed.
    pub fn is_board_removed(&self, board: &str) -> Result<bool, StoreError> {
        let conn = self.connect()?;
        let context = "reading a board";
        let Some(row) = row_of(&conn, board).map_err(|e| sql_error(context, e))? else {
            return Ok(false);
        };
        conn.query_row(
            "SELECT removed_ms IS NOT NULL FROM boards WHERE id = ?1",
            [row],
            |row| row.get(0),
        )
        .map_err(|e| sql_error(context, e))
    }

    /// The name of the removed board whose checkout is at `root`, or `None` when no removed board has that root.
    pub fn removed_board_at(&self, root: &str) -> Result<Option<String>, StoreError> {
        let conn = self.connect()?;
        conn.query_row(
            "SELECT id, name FROM boards WHERE root = ?1 AND removed_ms IS NOT NULL",
            [root],
            |row| Ok(name_of(row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|e| sql_error("reading the removed boards", e))
    }

    /// Marks `board` as the one opened or picked last, at `now_ms`. The mark is later than every other board's, even
    /// when the clock stands still or went back, so the board marked last is always the one with the latest time.
    /// Returns the time it was marked with, or `None` when the store has no such listed board.
    pub fn mark_board_opened(&self, board: &str, now_ms: u64) -> Result<Option<u64>, StoreError> {
        let context = "marking a board as opened";
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| sql_error(context, e))?;
        let Some(row) = row_of(&tx, board).map_err(|e| sql_error(context, e))? else {
            return Ok(None);
        };
        let removed: bool = tx
            .query_row(
                "SELECT removed_ms IS NOT NULL FROM boards WHERE id = ?1",
                [row],
                |row| row.get(0),
            )
            .map_err(|e| sql_error(context, e))?;
        if removed {
            return Ok(None);
        }
        let others: Option<i64> = tx
            .query_row(
                "SELECT MAX(last_opened_ms) FROM boards WHERE id != ?1",
                [row],
                |row| row.get(0),
            )
            .map_err(|e| sql_error(context, e))?;
        let now = i64::try_from(now_ms).unwrap_or(i64::MAX);
        let at = now.max(others.map_or(0, |latest| latest.saturating_add(1)));
        tx.execute(
            "UPDATE boards SET last_opened_ms = ?2 WHERE id = ?1",
            params![row, at],
        )
        .map_err(|e| sql_error(context, e))?;
        tx.commit().map_err(|e| sql_error(context, e))?;
        Ok(Some(u64::try_from(at).unwrap_or(0)))
    }

    /// The root of the board named `board`, or `None` when the store has no such listed board: a removed board is
    /// unknown here, so every method that names a board refuses it.
    pub fn board_root(&self, board: &str) -> Result<Option<String>, StoreError> {
        let conn = self.connect()?;
        let context = "reading a board";
        let Some(row) = row_of(&conn, board).map_err(|e| sql_error(context, e))? else {
            return Ok(None);
        };
        conn.query_row(
            "SELECT root FROM boards WHERE id = ?1 AND removed_ms IS NULL",
            [row],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| sql_error(context, e))
    }

    /// The root commits recorded for `board`, or `None` when none are (or the board is not in the store).
    pub fn root_commits(&self, board: &str) -> Result<Option<Vec<String>>, StoreError> {
        let context = "reading a board's root commits";
        let conn = self.connect()?;
        let Some(row) = row_of(&conn, board).map_err(|e| sql_error(context, e))? else {
            return Ok(None);
        };
        let text: Option<String> = conn
            .query_row(
                "SELECT root_commits FROM boards WHERE id = ?1",
                [row],
                |row| row.get(0),
            )
            .map_err(|e| sql_error(context, e))?;
        Ok(commits_of(text))
    }

    /// Records the root commits of the repository of `board`, when none are recorded yet. A record is never replaced,
    /// and an empty list is never recorded: a repository with no commit has no record to keep. Returns whether it
    /// wrote.
    pub fn record_root_commits(&self, board: &str, commits: &[String]) -> Result<bool, StoreError> {
        let context = "recording a board's root commits";
        if commits.is_empty() {
            return Ok(false);
        }
        let conn = self.connect()?;
        let Some(row) = row_of(&conn, board).map_err(|e| sql_error(context, e))? else {
            return Ok(false);
        };
        let changed = conn
            .execute(
                "UPDATE boards SET root_commits = ?2 WHERE id = ?1 AND root_commits IS NULL",
                params![row, commits_text(commits)],
            )
            .map_err(|e| sql_error(context, e))?;
        Ok(changed > 0)
    }

    /// Moves `board` to the checkout at `root`. Its name, its scratch tasks and everything keyed by its name stay.
    /// `false` when the store has no such board. A root another board has is an error of the store (the column is
    /// unique), so the caller looks first.
    pub fn move_board(&self, board: &str, root: &str) -> Result<bool, StoreError> {
        let context = "moving a board";
        let conn = self.connect()?;
        let Some(row) = row_of(&conn, board).map_err(|e| sql_error(context, e))? else {
            return Ok(false);
        };
        conn.execute(
            "UPDATE boards SET root = ?2 WHERE id = ?1",
            params![row, root],
        )
        .map_err(|e| sql_error(context, e))?;
        Ok(true)
    }

    /// Adds a scratch task to `board` with the next number there, at version 1.
    pub fn add_scratch_task(
        &self,
        board: &str,
        title: &str,
        prompt: &str,
        status: &str,
    ) -> Result<ScratchRow, StoreError> {
        let context = "writing a scratch task";
        let mut conn = self.connect()?;
        let number = row_of(&conn, board)
            .map_err(|e| sql_error(context, e))?
            .ok_or_else(|| StoreError::Io {
                context: context.to_string(),
                source: io::Error::new(io::ErrorKind::InvalidInput, format!("no board {board:?}")),
            })?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| sql_error(context, e))?;
        let num: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(num), 0) + 1 FROM scratch_tasks WHERE board = ?1",
                [number],
                |row| row.get(0),
            )
            .map_err(|e| sql_error(context, e))?;
        tx.execute(
            "INSERT INTO scratch_tasks (board, num, title, prompt, status, version)
             VALUES (?1, ?2, ?3, ?4, ?5, 1)",
            params![number, num, title, prompt, status],
        )
        .map_err(|e| sql_error(context, e))?;
        tx.commit().map_err(|e| sql_error(context, e))?;
        Ok(ScratchRow {
            num: num as u64,
            title: title.to_string(),
            prompt: prompt.to_string(),
            status: status.to_string(),
            version: 1,
        })
    }

    /// One scratch task of `board`, or `None` when the board has no task with that number.
    pub fn scratch_task(&self, board: &str, num: u64) -> Result<Option<ScratchRow>, StoreError> {
        let context = "reading a scratch task";
        let Ok(num) = i64::try_from(num) else {
            return Ok(None);
        };
        let conn = self.connect()?;
        let Some(board) = row_of(&conn, board).map_err(|e| sql_error(context, e))? else {
            return Ok(None);
        };
        conn.query_row(
            &format!("SELECT {TASK_COLUMNS} FROM scratch_tasks WHERE board = ?1 AND num = ?2"),
            params![board, num],
            task_row,
        )
        .optional()
        .map_err(|e| sql_error(context, e))
    }

    /// Every scratch task of `board`, by number. A board the store does not have has none.
    pub fn scratch_tasks(&self, board: &str) -> Result<Vec<ScratchRow>, StoreError> {
        let context = "reading the scratch tasks";
        let conn = self.connect()?;
        let Some(board) = row_of(&conn, board).map_err(|e| sql_error(context, e))? else {
            return Ok(Vec::new());
        };
        let mut statement = conn
            .prepare(&format!(
                "SELECT {TASK_COLUMNS} FROM scratch_tasks WHERE board = ?1 ORDER BY num"
            ))
            .map_err(|e| sql_error(context, e))?;
        let rows = statement
            .query_map([board], task_row)
            .map_err(|e| sql_error(context, e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| sql_error(context, e))
    }

    /// Sets a scratch task's status and raises its version by one. Returns the task as it is now, or `None` when the
    /// board has no task with that number.
    pub fn set_scratch_status(
        &self,
        board: &str,
        num: u64,
        status: &str,
    ) -> Result<Option<ScratchRow>, StoreError> {
        let context = "writing a scratch task's status";
        let Ok(num) = i64::try_from(num) else {
            return Ok(None);
        };
        let mut conn = self.connect()?;
        let Some(board) = row_of(&conn, board).map_err(|e| sql_error(context, e))? else {
            return Ok(None);
        };
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| sql_error(context, e))?;
        let changed = tx
            .execute(
                "UPDATE scratch_tasks SET status = ?3, version = version + 1 WHERE board = ?1 AND num = ?2",
                params![board, num, status],
            )
            .map_err(|e| sql_error(context, e))?;
        if changed == 0 {
            return Ok(None);
        }
        let row = tx
            .query_row(
                &format!("SELECT {TASK_COLUMNS} FROM scratch_tasks WHERE board = ?1 AND num = ?2"),
                params![board, num],
                task_row,
            )
            .map_err(|e| sql_error(context, e))?;
        tx.commit().map_err(|e| sql_error(context, e))?;
        Ok(Some(row))
    }
}
