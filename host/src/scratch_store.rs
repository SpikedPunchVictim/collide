//! The boards and scratch tasks in the store (ts-0075-hj; 0.1, 0.10). The tables are migration 5 of the store
//! ([`crate::store::MIGRATIONS`]); this file is the writes and reads on them. What a scratch task is, and which
//! statuses it may have, is [`crate::scratch`]'s; this is the SQL.
//!
//! - A board is named `board-<n>`: the store gives each checkout root the next number the first time it is opened, and
//!   gives the same name every time after.
//! - A scratch task is numbered inside its board, from 1. The number and the insert are one transaction, so two writers
//!   never take the same number.

use crate::store::{Store, StoreError, sql_error};
use rusqlite::{OptionalExtension, TransactionBehavior, params};
use std::io;

/// The text a board's name starts with. The number after it is the row's.
const BOARD_PREFIX: &str = "board-";

/// One board as the store holds it: its name and the top level of its checkout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBoard {
    pub id: String,
    pub root: String,
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

/// The number in a board's name, or `None` for text that is not the name of any board.
fn board_number(name: &str) -> Option<i64> {
    name.strip_prefix(BOARD_PREFIX)?.parse().ok()
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
    /// The board of the checkout at `root`: the one the store has, or a new one with the next number. Opening the same
    /// root again returns the same name.
    pub fn open_board(&self, root: &str) -> Result<String, StoreError> {
        let context = "opening a board";
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| sql_error(context, e))?;
        let known: Option<i64> = tx
            .query_row("SELECT id FROM boards WHERE root = ?1", [root], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| sql_error(context, e))?;
        let number = match known {
            Some(number) => number,
            None => {
                tx.execute("INSERT INTO boards (root) VALUES (?1)", [root])
                    .map_err(|e| sql_error(context, e))?;
                tx.last_insert_rowid()
            }
        };
        tx.commit().map_err(|e| sql_error(context, e))?;
        Ok(board_name(number))
    }

    /// Every board, in the order they were first opened.
    pub fn boards(&self) -> Result<Vec<StoredBoard>, StoreError> {
        let context = "reading the boards";
        let conn = self.connect()?;
        let mut statement = conn
            .prepare("SELECT id, root FROM boards ORDER BY id")
            .map_err(|e| sql_error(context, e))?;
        let rows = statement
            .query_map([], |row| {
                Ok(StoredBoard {
                    id: board_name(row.get(0)?),
                    root: row.get(1)?,
                })
            })
            .map_err(|e| sql_error(context, e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| sql_error(context, e))
    }

    /// The root of the board named `board`, or `None` when the store has no such board.
    pub fn board_root(&self, board: &str) -> Result<Option<String>, StoreError> {
        let Some(number) = board_number(board) else {
            return Ok(None);
        };
        self.connect()?
            .query_row("SELECT root FROM boards WHERE id = ?1", [number], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| sql_error("reading a board", e))
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
        let number = board_number(board).ok_or_else(|| StoreError::Io {
            context: context.to_string(),
            source: io::Error::new(io::ErrorKind::InvalidInput, format!("no board {board:?}")),
        })?;
        let mut conn = self.connect()?;
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
        let (Some(board), Ok(num)) = (board_number(board), i64::try_from(num)) else {
            return Ok(None);
        };
        self.connect()?
            .query_row(
                &format!("SELECT {TASK_COLUMNS} FROM scratch_tasks WHERE board = ?1 AND num = ?2"),
                params![board, num],
                task_row,
            )
            .optional()
            .map_err(|e| sql_error("reading a scratch task", e))
    }

    /// Every scratch task of `board`, by number. A board the store does not have has none.
    pub fn scratch_tasks(&self, board: &str) -> Result<Vec<ScratchRow>, StoreError> {
        let context = "reading the scratch tasks";
        let Some(board) = board_number(board) else {
            return Ok(Vec::new());
        };
        let conn = self.connect()?;
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
        let (Some(board), Ok(num)) = (board_number(board), i64::try_from(num)) else {
            return Ok(None);
        };
        let mut conn = self.connect()?;
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
