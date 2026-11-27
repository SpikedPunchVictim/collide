//! The corpus cache in the store (ts-0116-wh; 0.10, Collections). The tables are migration 10 of the store
//! ([`crate::store::MIGRATIONS`]); this file is the writes and reads on them.
//!
//! - **What a row is.** What one source last listed, or was told, for one checkout of one board: a task without its
//!   body, or an invalid item. A source that is not ref-scoped has no checkout, kept as empty text.
//! - **Written as it changes.** Each list and each notification the daemon applies is one transaction here, so a
//!   daemon that is killed leaves the cache as its last answered request left it, the same as after a clean stop.
//! - **Read at start only.** The daemon compares a fresh `list` with what [`Store::corpus_cache`] returns and then
//!   replaces it. It is never the truth: nothing else reads it.
//! - **A source's declaration is kept too** (its `initialize` answer), so that a start can serve the cached tasks
//!   with their categories before any provider is up.
//! - **A row that does not read is dropped.** It is deleted and named in [`CorpusCache::dropped`], so one bad row is
//!   a log line and a task that shows up as new, and never a daemon that cannot start.

use crate::store::{Store, StoreError, sql_error};
use collide_core::provider::{InitializeResult, Task, TaskInvalid};
use rusqlite::{Connection, params};

/// One cached task and where it was listed.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedTask {
    pub source: String,
    pub checkout: Option<String>,
    pub task: Task,
}

/// One cached invalid item. Its checkout is `item.checkout`.
#[derive(Debug, Clone, PartialEq)]
pub struct CachedInvalid {
    pub source: String,
    pub item: TaskInvalid,
}

/// A board's cache. `dropped` names each row that did not read, as the daemon logs it.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CorpusCache {
    pub tasks: Vec<CachedTask>,
    pub invalid: Vec<CachedInvalid>,
    /// What each cached source declared at its last start, by source name. The first listing of a start is served
    /// from these before any provider has started.
    pub sources: Vec<(String, InitializeResult)>,
    pub dropped: Vec<String>,
}

fn checkout_text(checkout: Option<&str>) -> &str {
    checkout.unwrap_or("")
}

fn checkout_of(text: String) -> Option<String> {
    (!text.is_empty()).then_some(text)
}

/// The row of a task: what is kept never holds a body, since a list does not carry one.
fn task_json(task: &Task) -> Result<String, StoreError> {
    let bare = Task {
        body: None,
        ..task.clone()
    };
    serde_json::to_string(&bare).map_err(|error| StoreError::Io {
        context: format!("writing the cache of task {}", task.key),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, error),
    })
}

fn insert_task(
    conn: &Connection,
    board: &str,
    source: &str,
    checkout: Option<&str>,
    task: &Task,
) -> Result<(), StoreError> {
    let json = task_json(task)?;
    conn.execute(
        "INSERT INTO corpus_tasks (board, source, checkout, key, path, json) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (board, source, checkout, key) DO UPDATE SET path = excluded.path, json = excluded.json",
        params![board, source, checkout_text(checkout), task.key, task.path, json],
    )
    .map_err(|e| sql_error("writing the corpus cache", e))?;
    if let Some(path) = &task.path {
        conn.execute(
            "DELETE FROM corpus_invalid WHERE board = ?1 AND source = ?2 AND checkout = ?3 AND path = ?4",
            params![board, source, checkout_text(checkout), path],
        )
        .map_err(|e| sql_error("writing the corpus cache", e))?;
    }
    Ok(())
}

fn insert_invalid(
    conn: &Connection,
    board: &str,
    source: &str,
    item: &TaskInvalid,
) -> Result<(), StoreError> {
    let checkout = checkout_text(item.checkout.as_deref());
    conn.execute(
        "DELETE FROM corpus_tasks WHERE board = ?1 AND source = ?2 AND checkout = ?3 AND path = ?4",
        params![board, source, checkout, item.path],
    )
    .map_err(|e| sql_error("writing the corpus cache", e))?;
    conn.execute(
        "INSERT INTO corpus_invalid (board, source, checkout, path, reason) VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT (board, source, checkout, path) DO UPDATE SET reason = excluded.reason",
        params![board, source, checkout, item.path, item.reason],
    )
    .map_err(|e| sql_error("writing the corpus cache", e))?;
    Ok(())
}

impl Store {
    /// Everything cached for `board`. A row that does not read as a task is deleted and named in `dropped`.
    pub fn corpus_cache(&self, board: &str) -> Result<CorpusCache, StoreError> {
        let context = "reading the corpus cache";
        let conn = self.connect()?;
        let mut cache = CorpusCache::default();
        let mut bad: Vec<(String, String, String, String)> = Vec::new();
        {
            let mut statement = conn
                .prepare(
                    "SELECT source, checkout, key, json FROM corpus_tasks WHERE board = ?1
                     ORDER BY source, checkout, key",
                )
                .map_err(|e| sql_error(context, e))?;
            let rows = statement
                .query_map([board], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                    ))
                })
                .map_err(|e| sql_error(context, e))?
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| sql_error(context, e))?;
            for (source, checkout, key, json) in rows {
                match serde_json::from_str::<Task>(&json) {
                    Ok(task) => cache.tasks.push(CachedTask {
                        source,
                        checkout: checkout_of(checkout),
                        task,
                    }),
                    Err(error) => {
                        cache.dropped.push(format!(
                            "the cached task {key} of source {source} did not read, so it was dropped: {error}"
                        ));
                        bad.push((source, checkout, key, json));
                    }
                }
            }
        }
        for (source, checkout, key, _) in bad {
            conn.execute(
                "DELETE FROM corpus_tasks WHERE board = ?1 AND source = ?2 AND checkout = ?3 AND key = ?4",
                params![board, source, checkout, key],
            )
            .map_err(|e| sql_error(context, e))?;
        }
        let mut statement = conn
            .prepare(
                "SELECT source, checkout, path, reason FROM corpus_invalid WHERE board = ?1
                 ORDER BY source, checkout, path",
            )
            .map_err(|e| sql_error(context, e))?;
        let rows = statement
            .query_map([board], |row| {
                Ok(CachedInvalid {
                    source: row.get(0)?,
                    item: TaskInvalid {
                        checkout: checkout_of(row.get(1)?),
                        path: row.get(2)?,
                        reason: row.get(3)?,
                    },
                })
            })
            .map_err(|e| sql_error(context, e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| sql_error(context, e))?;
        cache.invalid = rows;
        let mut statement = conn
            .prepare("SELECT source, json FROM corpus_sources WHERE board = ?1 ORDER BY source")
            .map_err(|e| sql_error(context, e))?;
        let rows = statement
            .query_map([board], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| sql_error(context, e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| sql_error(context, e))?;
        drop(statement);
        for (source, json) in rows {
            match serde_json::from_str::<InitializeResult>(&json) {
                Ok(init) => cache.sources.push((source, init)),
                Err(error) => {
                    cache.dropped.push(format!(
                        "the cached declaration of source {source} did not read, so it was dropped: {error}"
                    ));
                    conn.execute(
                        "DELETE FROM corpus_sources WHERE board = ?1 AND source = ?2",
                        params![board, source],
                    )
                    .map_err(|e| sql_error(context, e))?;
                }
            }
        }
        Ok(cache)
    }

    /// Keeps what `source` declared at `initialize`, replacing its last declaration.
    pub fn corpus_put_source(
        &self,
        board: &str,
        source: &str,
        init: &InitializeResult,
    ) -> Result<(), StoreError> {
        let json = serde_json::to_string(init).map_err(|error| StoreError::Io {
            context: format!("writing the declaration of source {source}"),
            source: std::io::Error::new(std::io::ErrorKind::InvalidData, error),
        })?;
        self.connect()?
            .execute(
                "INSERT INTO corpus_sources (board, source, json) VALUES (?1, ?2, ?3)
                 ON CONFLICT (board, source) DO UPDATE SET json = excluded.json",
                params![board, source, json],
            )
            .map_err(|e| sql_error("writing the corpus cache", e))?;
        Ok(())
    }

    /// Makes what is cached for one source in one checkout exactly this list, in one transaction.
    pub fn corpus_replace(
        &self,
        board: &str,
        source: &str,
        checkout: Option<&str>,
        tasks: &[Task],
        invalid: &[TaskInvalid],
    ) -> Result<(), StoreError> {
        let mut conn = self.connect()?;
        let tx = conn
            .transaction()
            .map_err(|e| sql_error("replacing the corpus cache", e))?;
        for table in ["corpus_tasks", "corpus_invalid"] {
            tx.execute(
                &format!("DELETE FROM {table} WHERE board = ?1 AND source = ?2 AND checkout = ?3"),
                params![board, source, checkout_text(checkout)],
            )
            .map_err(|e| sql_error("replacing the corpus cache", e))?;
        }
        for task in tasks {
            insert_task(&tx, board, source, checkout, task)?;
        }
        for item in invalid {
            insert_invalid(&tx, board, source, item)?;
        }
        tx.commit()
            .map_err(|e| sql_error("replacing the corpus cache", e))
    }

    /// Keeps `task` as the copy of its key in `checkout`, replacing the one there, and clears the invalid item its
    /// path had.
    pub fn corpus_put_task(
        &self,
        board: &str,
        source: &str,
        checkout: Option<&str>,
        task: &Task,
    ) -> Result<(), StoreError> {
        let mut conn = self.connect()?;
        let tx = conn
            .transaction()
            .map_err(|e| sql_error("writing the corpus cache", e))?;
        insert_task(&tx, board, source, checkout, task)?;
        tx.commit()
            .map_err(|e| sql_error("writing the corpus cache", e))
    }

    /// Forgets the copy of `key` in `checkout`. A copy that is not there is not an error.
    pub fn corpus_drop_task(
        &self,
        board: &str,
        source: &str,
        checkout: Option<&str>,
        key: &str,
    ) -> Result<(), StoreError> {
        self.connect()?
            .execute(
                "DELETE FROM corpus_tasks WHERE board = ?1 AND source = ?2 AND checkout = ?3 AND key = ?4",
                params![board, source, checkout_text(checkout), key],
            )
            .map_err(|e| sql_error("writing the corpus cache", e))?;
        Ok(())
    }

    /// Forgets everything the cache holds for `board`: its tasks, its invalid items and what its sources declared.
    /// The cache is a copy, so nothing is lost (ts-0222-gx).
    pub fn corpus_drop_board(&self, board: &str) -> Result<(), StoreError> {
        let mut conn = self.connect()?;
        let tx = conn
            .transaction()
            .map_err(|e| sql_error("dropping the corpus cache", e))?;
        for table in ["corpus_tasks", "corpus_invalid", "corpus_sources"] {
            tx.execute(&format!("DELETE FROM {table} WHERE board = ?1"), [board])
                .map_err(|e| sql_error("dropping the corpus cache", e))?;
        }
        tx.commit()
            .map_err(|e| sql_error("dropping the corpus cache", e))
    }

    /// Keeps an invalid item, and forgets the task that was at its path.
    pub fn corpus_put_invalid(
        &self,
        board: &str,
        source: &str,
        item: &TaskInvalid,
    ) -> Result<(), StoreError> {
        let mut conn = self.connect()?;
        let tx = conn
            .transaction()
            .map_err(|e| sql_error("writing the corpus cache", e))?;
        insert_invalid(&tx, board, source, item)?;
        tx.commit()
            .map_err(|e| sql_error("writing the corpus cache", e))
    }
}
