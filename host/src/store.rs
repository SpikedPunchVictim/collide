//! The store (0.2, Durability; Stage 3a): SQLite for state and the event log, in one directory under the daemon's.
//!
//! What 0.2 requires of the store, and where it is met:
//!
//! - **A store that cannot be read is moved aside and reported, never overwritten** ([`quarantine`],
//!   [`StoreError::Quarantined`]). The same rule holds for `exit.json` in the shim. The move is a hard link to a new
//!   name followed by removing the old one, so it fails rather than replace a file that is already there.
//! - **State is saved without holding a lock that other work waits on.** A [`Store`] holds a path and nothing else:
//!   each call opens its own connection, runs one short transaction and closes it, so no `Mutex` is held and no
//!   connection is shared between threads. The database is in WAL mode, where a reader never blocks the writer and a
//!   writer never blocks a reader, and two writers wait on SQLite's own lock for the length of one `INSERT`.
//! - **Logging never panics**, and a full disk is tested with a writer that fails with `ENOSPC` on demand: the log is
//!   written as JSON lines through [`std::io::Write`] ([`Store::write_events_since`]), and a failed write is a
//!   [`StoreError::Io`], not a panic. SQLite's own full disk (`SQLITE_FULL`) is the same error, with the kind
//!   [`io::ErrorKind::StorageFull`].
//! - **Migrations are numbered and run in order** ([`Migration`]), each in one transaction that also sets the
//!   store's `user_version`, and a store newer than the build is refused rather than downgraded.
//!
//! The events written are `core`'s ([`collide_core::events::Event`]): one row of the `events` table each, holding the
//! event's JSON line as written. The first event is number 1, so `events_since(0)` is the whole log, and each number
//! is the one after the last: a gap or a repeat is refused.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `store: open and migrate` | [`Store::open`] | opens the database, runs the migrations that are due |
//! | `store: event log` | [`Store::append_event`], [`Store::events_since`] | the event log's append and read |
//! | `store: quarantine` | [`quarantine`] | moves an unreadable store or record aside, with a report |

use crate::run_store::note_transition;
use collide_core::agent::Usage;
use collide_core::events::{Event, EventBody};
use collide_core::owner::RunWarning;
use rusqlite::{Connection, ErrorCode, OpenFlags, TransactionBehavior, params};
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// The database file in the store's folder. Its WAL files are this name with `-wal` and `-shm` added.
pub const DB_FILE: &str = "store.db";

/// How long a connection waits for another writer's transaction, which is one `INSERT` or one migration.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The WAL file that sits beside the database while it is open: the writes the database file does not hold yet.
const WAL: &str = "-wal";

/// The files that sit beside the database while it is open: the WAL, and the shared-memory index of it.
const SIDE_FILES: [&str; 2] = [WAL, "-shm"];

/// Added to the WAL file's name for the second name [`keep_side_files`] gives it.
const KEEP: &str = ".keep";

/// How many names [`quarantine`] tries before it gives up. A name is taken only by an earlier quarantine of the same
/// file in the same millisecond, so reaching this is a bug or a broken folder.
const QUARANTINE_NAMES: u32 = 1000;

/// One schema change. `version` counts from 1 with no gap, and `sql` is the statements that take the store from the
/// version before to this one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

/// The migrations, in order. `events` holds the event log: `seq` is the event's number, `wall_ms` is the time it was
/// written with, and `line` is its JSON. `run_warnings` holds what a run did that the person should look at
/// (ts-0104-gx): one JSON row for each warning, in the order they were written.
pub const MIGRATIONS: [Migration; 8] = [
    Migration {
        version: 1,
        name: "events",
        sql: "CREATE TABLE events (
              seq     INTEGER PRIMARY KEY,
              wall_ms INTEGER NOT NULL,
              line    TEXT NOT NULL
          )",
    },
    Migration {
        version: 2,
        name: "phase_records",
        sql: "CREATE TABLE phase_records (
              run     TEXT NOT NULL,
              phase   TEXT NOT NULL,
              usage   TEXT,
              failure TEXT,
              report  TEXT,
              PRIMARY KEY (run, phase)
          )",
    },
    Migration {
        version: 3,
        name: "run_contexts_and_queue_items",
        sql: "CREATE TABLE run_contexts (
              run  TEXT PRIMARY KEY,
              json TEXT NOT NULL
          );
          CREATE TABLE queue_items (
              id   TEXT PRIMARY KEY,
              run  TEXT,
              json TEXT NOT NULL
          )",
    },
    Migration {
        version: 4,
        name: "metrics",
        sql: METRICS_SQL,
    },
    Migration {
        version: 5,
        name: "boards_and_scratch_tasks",
        sql: BOARDS_SQL,
    },
    Migration {
        version: 6,
        name: "runs",
        sql: RUNS_SQL,
    },
    Migration {
        version: 7,
        name: "phase_record_failure_evidence",
        sql: "ALTER TABLE phase_records ADD COLUMN failure_evidence TEXT",
    },
    Migration {
        version: 8,
        name: "run_warnings",
        sql: "CREATE TABLE run_warnings (
              id   INTEGER PRIMARY KEY AUTOINCREMENT,
              run  TEXT NOT NULL,
              json TEXT NOT NULL
          );
          CREATE INDEX run_warnings_by_run ON run_warnings (run)",
    },
];

/// The index of runs (ts-0076-cw; 0.8, Run), see [`crate::run_store`]. `state` is the run's latest `RunState` as JSON,
/// and `live` is 1 for a state that is not final. `opened_seq` is the number of the run's first event, so it orders the
/// runs and is the cursor of a page. `board` is null for a run that has none. The insert at the end gives a run the log
/// already has a row, from its `run.transition` events: its first and its last.
const RUNS_SQL: &str = "
    CREATE TABLE runs (
        run        TEXT PRIMARY KEY,
        board      TEXT,
        line       TEXT NOT NULL DEFAULT '',
        workflow   TEXT NOT NULL DEFAULT '',
        state      TEXT NOT NULL,
        live       INTEGER NOT NULL,
        opened_seq INTEGER NOT NULL,
        started_ms INTEGER NOT NULL,
        ended_ms   INTEGER
    );
    CREATE INDEX runs_by_opened ON runs (opened_seq);
    CREATE INDEX runs_by_board ON runs (board, opened_seq);
    CREATE TABLE run_tasks (
        run    TEXT NOT NULL,
        ord    INTEGER NOT NULL,
        task   TEXT NOT NULL,
        status TEXT NOT NULL,
        PRIMARY KEY (run, ord)
    );
    CREATE INDEX run_tasks_by_task ON run_tasks (task, run);
    WITH transitions AS (
        SELECT seq, wall_ms,
               json_extract(line, '$.body.run') AS run,
               json_extract(line, '$.body.to') AS to_state
        FROM events
        WHERE json_extract(line, '$.body.event') = 'run.transition'
    ),
    bounds AS (
        SELECT run, MIN(seq) AS first_seq, MAX(seq) AS last_seq FROM transitions GROUP BY run
    )
    INSERT INTO runs (run, state, live, opened_seq, started_ms, ended_ms)
    SELECT bounds.run,
           latest.to_state,
           json_extract(latest.to_state, '$.state') NOT IN ('ended', 'failed', 'stopped'),
           bounds.first_seq,
           first.wall_ms,
           CASE WHEN json_extract(latest.to_state, '$.state') IN ('ended', 'failed', 'stopped')
                THEN latest.wall_ms END
    FROM bounds
    JOIN transitions AS first ON first.seq = bounds.first_seq
    JOIN transitions AS latest ON latest.seq = bounds.last_seq;
";

/// The boards the user opened and their scratch tasks (ts-0075-hj; 0.1, 0.10). `boards.root` is the checkout's top
/// level, so opening it again finds the same row. A scratch task is numbered inside its board: `num` is the short id
/// of the key `scratch:<num>`. `status` is one of the five categories, and `version` counts the writes to the task.
const BOARDS_SQL: &str = "
    CREATE TABLE boards (
        id   INTEGER PRIMARY KEY,
        root TEXT NOT NULL UNIQUE
    );
    CREATE TABLE scratch_tasks (
        board   INTEGER NOT NULL,
        num     INTEGER NOT NULL,
        title   TEXT NOT NULL,
        prompt  TEXT NOT NULL,
        status  TEXT NOT NULL,
        version INTEGER NOT NULL,
        PRIMARY KEY (board, num)
    );
";

/// The tables of the always-recorded metrics (ts-0067-kw; 0.13). `spans` holds runs, phases, turns and waits, each
/// with the continuous clock's nanoseconds (`started_ns`, `ended_ns`: what lengths are measured on) and the wall
/// clock's milliseconds (for display). A span that has no end is open. Every figure an agent may not report is
/// nullable: a missing value is NULL, never 0. Nothing here holds a message, a tool's input, a tool's result, a hook's
/// output or an `Other` event's body.
const METRICS_SQL: &str = "
    CREATE TABLE spans (
        id              INTEGER PRIMARY KEY,
        run             TEXT NOT NULL,
        phase           TEXT,
        parent          INTEGER,
        kind            TEXT NOT NULL,
        wait            TEXT,
        item            TEXT,
        started_wall_ms INTEGER NOT NULL,
        started_ns      INTEGER NOT NULL,
        ended_wall_ms   INTEGER,
        ended_ns        INTEGER,
        outcome         TEXT
    );
    CREATE INDEX spans_by_run ON spans (run, kind);
    CREATE TABLE usage_rows (
        id             INTEGER PRIMARY KEY,
        run            TEXT NOT NULL,
        phase          TEXT NOT NULL,
        turn           INTEGER,
        wall_ms        INTEGER NOT NULL,
        scope          TEXT NOT NULL,
        quality        TEXT NOT NULL,
        input          INTEGER,
        output         INTEGER,
        cache_read     INTEGER,
        cache_write    INTEGER,
        cache_write_1h INTEGER,
        cache_write_5m INTEGER,
        thought        INTEGER,
        context_used   INTEGER,
        context_size   INTEGER,
        cost_amount    REAL,
        cost_currency  TEXT,
        model_time_ms  INTEGER
    );
    CREATE INDEX usage_rows_by_phase ON usage_rows (run, phase);
    CREATE TABLE tool_calls (
        run     TEXT NOT NULL,
        phase   TEXT NOT NULL,
        call_id TEXT NOT NULL,
        tool    TEXT,
        kind    TEXT,
        bytes   INTEGER,
        refused TEXT,
        PRIMARY KEY (run, phase, call_id)
    );
    CREATE TABLE hook_calls (
        id        INTEGER PRIMARY KEY,
        run       TEXT NOT NULL,
        phase     TEXT NOT NULL,
        turn      INTEGER,
        name      TEXT NOT NULL,
        event     TEXT NOT NULL,
        outcome   TEXT NOT NULL,
        exit_code INTEGER,
        decision  TEXT
    );
    CREATE TABLE findings (
        id            INTEGER PRIMARY KEY,
        run           TEXT NOT NULL,
        phase         TEXT NOT NULL,
        turn          INTEGER,
        detector      TEXT NOT NULL,
        name          TEXT,
        event         TEXT,
        tool_call_id  TEXT,
        cause         TEXT NOT NULL,
        cost_amount   REAL,
        cost_currency TEXT,
        cost_basis    TEXT NOT NULL
    );
    CREATE TABLE compactions (
        id            INTEGER PRIMARY KEY,
        run           TEXT NOT NULL,
        phase         TEXT NOT NULL,
        turn          INTEGER,
        wall_ms       INTEGER NOT NULL,
        trigger_kind  TEXT,
        tokens_before INTEGER,
        tokens_after  INTEGER,
        duration_ms   INTEGER,
        mid_task      INTEGER NOT NULL
    );
    CREATE TABLE other_counts (
        run      TEXT NOT NULL,
        raw_type TEXT NOT NULL,
        count    INTEGER NOT NULL,
        PRIMARY KEY (run, raw_type)
    );
    CREATE TABLE phase_ends (
        id              INTEGER PRIMARY KEY,
        run             TEXT NOT NULL,
        phase           TEXT NOT NULL,
        wall_ms         INTEGER NOT NULL,
        state           TEXT NOT NULL,
        exit_code       INTEGER,
        signal          INTEGER,
        cpu_user_us     INTEGER,
        cpu_system_us   INTEGER,
        max_rss_bytes   INTEGER,
        workspace_bytes INTEGER
    );
    CREATE TABLE cold_returns (
        id      INTEGER PRIMARY KEY,
        run     TEXT NOT NULL,
        phase   TEXT NOT NULL,
        cause   TEXT NOT NULL,
        idle_ms INTEGER NOT NULL,
        wall_ms INTEGER NOT NULL
    );
    CREATE TABLE fingerprints (
        run  TEXT NOT NULL,
        part TEXT NOT NULL,
        json TEXT NOT NULL,
        PRIMARY KEY (run, part)
    );
";

/// What the daemon keeps of a phase beyond its state events (ts-0065-kx): the last usage its session reported, why it
/// failed when the session could tell, and its report. Nothing of the agent's stream is stored here: the events of
/// the stream are ts-0067's.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct PhaseRecord {
    /// The last usage event of the phase's session, as it reported it.
    pub usage: Option<Usage>,
    /// `no_result`, `refusal`, `max_tokens`, `max_turns`, `budget`, `blocked`, `api` (`api_<status>` when the result
    /// had a status), `empty_turn`, `settings_ignored` (the settings canary never appeared) or `model_unavailable`
    /// (the agent does not offer the model the role names): what the session said the failure was. Core's
    /// `PhaseState::Failed` carries only `disk`, so the cause is kept here.
    pub failure: Option<String>,
    /// What backs `failure` when the session had some: the end of the agent's stderr for an `empty_turn`, the error
    /// code for an error response. At most [`collide_core::adapter::EVIDENCE_BYTES`], from the end, on a character
    /// boundary. It is untrusted text and may hold what the person did not expect to be kept.
    pub failure_evidence: Option<String>,
    /// The text of the session's final report.
    pub report: Option<String>,
}

/// Why the store could not do what it was asked.
#[derive(Debug)]
pub enum StoreError {
    /// The file could not be read as a store. It was moved to `moved_to`, left whole, and the next [`Store::open`]
    /// makes a fresh store in its place.
    Quarantined { moved_to: PathBuf, reason: String },
    /// The store was written by a newer build: its version is above the newest migration this build knows.
    TooNew { found: u32, known: u32 },
    /// An event's number is not the one after the last: `expected` is the number it had to carry.
    OutOfOrder { expected: u64, found: u64 },
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
            StoreError::OutOfOrder { expected, found } => write!(
                f,
                "event {found} was refused: the next event is number {expected}"
            ),
            StoreError::Io { context, source } => write!(f, "{context}: {source}"),
        }
    }
}

impl std::error::Error for StoreError {}

/// The store of one daemon. It holds the folder and nothing else: see the module doc for why no connection is kept.
#[derive(Debug)]
pub struct Store {
    /// The directory the database file is in.
    pub dir: PathBuf,
}

impl Store {
    /// Opens the store in `dir`, making the folder and the database when they are not there and running the
    /// [`MIGRATIONS`] that are due, each in one transaction. A database that cannot be read, because it is not one or
    /// because it fails SQLite's quick check, is moved aside ([`quarantine`], with its WAL files) and reported, and the
    /// open fails with [`StoreError::Quarantined`] so the caller says so. Opening again makes the fresh store. The
    /// quick check reads the whole file, so an open costs time in proportion to the store's size.
    pub fn open(dir: &Path) -> Result<Store, StoreError> {
        fs::create_dir_all(dir).map_err(|source| StoreError::Io {
            context: format!("making the store folder {}", dir.display()),
            source,
        })?;
        let store = Store {
            dir: dir.to_path_buf(),
        };
        let path = store.db_path();
        keep_side_files(&path)?;
        match prepare(&path) {
            Ok(()) => {
                release_side_files(&path)?;
                Ok(store)
            }
            Err(Unusable::Unreadable(reason)) => {
                let moved_to = quarantine_database(&path, &reason)?;
                Err(StoreError::Quarantined { moved_to, reason })
            }
            Err(Unusable::Failed(error)) => {
                release_side_files(&path)?;
                Err(error)
            }
        }
    }

    /// The database file.
    pub fn db_path(&self) -> PathBuf {
        self.dir.join(DB_FILE)
    }

    /// Appends one event to the event log. `event.seq` is the next number, and a gap or a repeat is refused with
    /// [`StoreError::OutOfOrder`]. The check and the insert are one statement, so two writers cannot both take a number.
    ///
    /// A `run.transition` event also moves the run's row in the index of runs ([`crate::run_store`]), in the same
    /// transaction, so the log and the index never disagree.
    pub fn append_event(&self, event: &Event) -> Result<(), StoreError> {
        let context = "writing an event";
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| sql_error(context, e))?;
        insert_event(&tx, event)?;
        if let EventBody::RunTransition { run, to, .. } = &event.body {
            note_transition(&tx, event.seq, event.wall_ms, run, to)?;
        }
        tx.commit().map_err(|e| sql_error(context, e))
    }

    /// The number of the oldest event the log holds. An empty log has none, and its answer is the number the first
    /// event will have, so a client that asks for `since` below it finds nothing missing.
    pub fn oldest_seq(&self) -> Result<u64, StoreError> {
        let conn = self.connect()?;
        let oldest: Option<i64> = conn
            .query_row("SELECT MIN(seq) FROM events", [], |row| row.get(0))
            .map_err(|e| sql_error("reading the first event number", e))?;
        match oldest {
            Some(seq) => Ok(seq as u64),
            None => Ok(last_seq(&conn)? + 1),
        }
    }

    /// The events with `seq` above `after`, in order, for a client that reconnects. `0` is the whole log. A line this
    /// build cannot read as an [`Event`] (one a newer build wrote) is an error that names its number.
    pub fn events_since(&self, after: u64) -> Result<Vec<Event>, StoreError> {
        lines_since(&self.connect()?, after)?
            .into_iter()
            .map(|(seq, line)| {
                serde_json::from_str(&line).map_err(|error| StoreError::Io {
                    context: format!("reading event {seq} of the event log"),
                    source: io::Error::new(io::ErrorKind::InvalidData, error),
                })
            })
            .collect()
    }

    /// Writes the phase's record, replacing the one it had: the daemon writes it each time the session reports usage,
    /// and once more when the phase ends.
    pub fn set_phase_record(
        &self,
        run: &str,
        phase: &str,
        record: &PhaseRecord,
    ) -> Result<(), StoreError> {
        let usage = record
            .usage
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| StoreError::Io {
                context: "writing a phase record".to_string(),
                source: io::Error::new(io::ErrorKind::InvalidData, error),
            })?;
        self.connect()?
            .execute(
                "INSERT INTO phase_records (run, phase, usage, failure, failure_evidence, report)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                 ON CONFLICT (run, phase) DO UPDATE SET
                     usage = excluded.usage, failure = excluded.failure,
                     failure_evidence = excluded.failure_evidence, report = excluded.report",
                params![
                    run,
                    phase,
                    usage,
                    record.failure,
                    record.failure_evidence,
                    record.report
                ],
            )
            .map_err(|e| sql_error("writing a phase record", e))?;
        Ok(())
    }

    /// Keeps what a file-task run needs to go on after a restart (its params, its session, its workspace): `json` is
    /// the daemon's own text. One row for each run, replaced by the next write.
    pub fn set_run_context(&self, run: &str, json: &str) -> Result<(), StoreError> {
        self.connect()?
            .execute(
                "INSERT INTO run_contexts (run, json) VALUES (?1, ?2)
                 ON CONFLICT (run) DO UPDATE SET json = excluded.json",
                params![run, json],
            )
            .map_err(|e| sql_error("writing a run's context", e))?;
        Ok(())
    }

    /// Takes a run's context out. A run that has none is not an error.
    pub fn delete_run_context(&self, run: &str) -> Result<(), StoreError> {
        self.connect()?
            .execute("DELETE FROM run_contexts WHERE run = ?1", params![run])
            .map_err(|e| sql_error("deleting a run's context", e))?;
        Ok(())
    }

    /// Every run's context as `(run, json)`, in the order the runs were first written.
    pub fn run_contexts(&self) -> Result<Vec<(String, String)>, StoreError> {
        self.pairs(
            "SELECT run, json FROM run_contexts ORDER BY rowid",
            "reading the runs' contexts",
        )
    }

    /// Keeps a Queue item so a restart does not lose it. `json` is the item. Writing the same `id` again replaces it.
    pub fn set_queue_item(
        &self,
        id: &str,
        run: Option<&str>,
        json: &str,
    ) -> Result<(), StoreError> {
        self.connect()?
            .execute(
                "INSERT INTO queue_items (id, run, json) VALUES (?1, ?2, ?3)
                 ON CONFLICT (id) DO UPDATE SET run = excluded.run, json = excluded.json",
                params![id, run, json],
            )
            .map_err(|e| sql_error("writing a queue item", e))?;
        Ok(())
    }

    /// Takes one Queue item out. An `id` that is not there is not an error.
    pub fn delete_queue_item(&self, id: &str) -> Result<(), StoreError> {
        self.connect()?
            .execute("DELETE FROM queue_items WHERE id = ?1", params![id])
            .map_err(|e| sql_error("deleting a queue item", e))?;
        Ok(())
    }

    /// Every stored Queue item as `(id, json)`, in the order they were first written.
    pub fn queue_items(&self) -> Result<Vec<(String, String)>, StoreError> {
        self.pairs(
            "SELECT id, json FROM queue_items ORDER BY rowid",
            "reading the queue items",
        )
    }

    fn pairs(&self, sql: &str, context: &str) -> Result<Vec<(String, String)>, StoreError> {
        let conn = self.connect()?;
        let mut statement = conn.prepare(sql).map_err(|e| sql_error(context, e))?;
        let rows = statement
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| sql_error(context, e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| sql_error(context, e))
    }

    /// Keeps a warning of a run (ts-0104-gx), after the ones it already has. The warning is its own JSON, so a new kind
    /// or field needs no migration.
    pub fn add_run_warning(&self, run: &str, warning: &RunWarning) -> Result<(), StoreError> {
        let json = serde_json::to_string(warning).map_err(|error| StoreError::Io {
            context: format!("writing a warning of run {run}"),
            source: io::Error::new(io::ErrorKind::InvalidData, error),
        })?;
        self.connect()?
            .execute(
                "INSERT INTO run_warnings (run, json) VALUES (?1, ?2)",
                params![run, json],
            )
            .map_err(|e| sql_error("writing a run's warning", e))?;
        Ok(())
    }

    /// The warnings of `run`, oldest first. A run with none (every run from before the table) has an empty list.
    pub fn run_warnings(&self, run: &str) -> Result<Vec<RunWarning>, StoreError> {
        let conn = self.connect()?;
        let mut statement = conn
            .prepare("SELECT json FROM run_warnings WHERE run = ?1 ORDER BY id")
            .map_err(|e| sql_error("reading a run's warnings", e))?;
        let rows = statement
            .query_map(params![run], |row| row.get::<_, String>(0))
            .map_err(|e| sql_error("reading a run's warnings", e))?;
        let texts = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| sql_error("reading a run's warnings", e))?;
        texts
            .iter()
            .map(|text| serde_json::from_str::<RunWarning>(text))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| StoreError::Io {
                context: format!("reading the warnings of run {run}"),
                source: io::Error::new(io::ErrorKind::InvalidData, error),
            })
    }

    /// The phase's record, or `None` for a phase that has none.
    pub fn phase_record(&self, run: &str, phase: &str) -> Result<Option<PhaseRecord>, StoreError> {
        let conn = self.connect()?;
        let row = conn.query_row(
            "SELECT usage, failure, failure_evidence, report FROM phase_records WHERE run = ?1 AND phase = ?2",
            params![run, phase],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        );
        let (usage, failure, failure_evidence, report) = match row {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(error) => return Err(sql_error("reading a phase record", error)),
        };
        let usage = usage
            .map(|text| serde_json::from_str::<Usage>(&text))
            .transpose()
            .map_err(|error| StoreError::Io {
                context: format!("reading the record of phase {run}/{phase}"),
                source: io::Error::new(io::ErrorKind::InvalidData, error),
            })?;
        Ok(Some(PhaseRecord {
            usage,
            failure,
            failure_evidence,
            report,
        }))
    }

    /// The number of the last event, or 0 when the log is empty.
    pub fn last_seq(&self) -> Result<u64, StoreError> {
        last_seq(&self.connect()?)
    }

    /// Writes the events with `seq` above `after` to `out`, one JSON line each, and returns how many. A write that
    /// fails (a full disk, a closed pipe) is a [`StoreError::Io`] that carries the writer's own error, and nothing more
    /// is written after it.
    pub fn write_events_since(
        &self,
        after: u64,
        out: &mut dyn io::Write,
    ) -> Result<usize, StoreError> {
        let lines = lines_since(&self.connect()?, after)?;
        let fail = |source| StoreError::Io {
            context: "writing the event log".to_string(),
            source,
        };
        for (_, line) in &lines {
            out.write_all(format!("{line}\n").as_bytes())
                .map_err(fail)?;
        }
        out.flush().map_err(fail)?;
        Ok(lines.len())
    }

    /// A connection for one call. Opened without `CREATE`: the file was made by [`Store::open`], and a call that finds
    /// it gone is an error and not a second, empty store.
    pub(crate) fn connect(&self) -> Result<Connection, StoreError> {
        let conn = Connection::open_with_flags(
            self.db_path(),
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )
        .map_err(|e| sql_error("opening the store", e))?;
        conn.busy_timeout(BUSY_TIMEOUT)
            .map_err(|e| sql_error("opening the store", e))?;
        Ok(conn)
    }
}

/// Why [`prepare`] did not give a usable database.
enum Unusable {
    /// SQLite says the file is not a database or is damaged.
    Unreadable(String),
    Failed(StoreError),
}

/// Opens `path` (making it when it is not there), checks that it can be read, puts it in WAL mode and runs the
/// migrations that are due. The connection is closed on return.
fn prepare(path: &Path) -> Result<(), Unusable> {
    let judge = |context: &str, error: rusqlite::Error| match error {
        rusqlite::Error::SqliteFailure(ref failure, _)
            if matches!(
                failure.code,
                ErrorCode::NotADatabase | ErrorCode::DatabaseCorrupt
            ) =>
        {
            Unusable::Unreadable(error.to_string())
        }
        other => Unusable::Failed(sql_error(context, other)),
    };
    let mut conn = Connection::open(path).map_err(|e| judge("opening the store", e))?;
    conn.busy_timeout(BUSY_TIMEOUT)
        .map_err(|e| judge("opening the store", e))?;
    let check: String = conn
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(|e| judge("checking the store", e))?;
    if check != "ok" {
        return Err(Unusable::Unreadable(check));
    }
    let mode: String = conn
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(|e| judge("switching the store to WAL", e))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(Unusable::Failed(StoreError::Io {
            context: "switching the store to WAL".to_string(),
            source: io::Error::other(format!("the journal mode is {mode}")),
        }));
    }
    migrate(&mut conn, &MIGRATIONS).map_err(Unusable::Failed)?;
    Ok(())
}

/// Runs the `migrations` above the store's `user_version`, in order, each in one transaction that also sets the
/// version, and returns how many ran. A store above the newest migration is [`StoreError::TooNew`]. The version is read
/// again inside the transaction, so a second process that opens at the same moment does not run a migration twice.
fn migrate(conn: &mut Connection, migrations: &[Migration]) -> Result<usize, StoreError> {
    let version = |conn: &Connection| -> Result<u32, StoreError> {
        conn.query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|e| sql_error("reading the store's version", e))
    };
    let found = version(conn)?;
    let known = migrations.last().map_or(0, |m| m.version);
    if found > known {
        return Err(StoreError::TooNew { found, known });
    }
    let mut ran = 0;
    for migration in migrations.iter().filter(|m| m.version > found) {
        let context = format!(
            "running migration {} ({})",
            migration.version, migration.name
        );
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| sql_error(&context, e))?;
        if version(&tx)? >= migration.version {
            continue;
        }
        tx.execute_batch(migration.sql)
            .map_err(|e| sql_error(&context, e))?;
        tx.pragma_update(None, "user_version", migration.version)
            .map_err(|e| sql_error(&context, e))?;
        tx.commit().map_err(|e| sql_error(&context, e))?;
        ran += 1;
    }
    Ok(ran)
}

/// The insert of [`Store::append_event`]: one statement that takes the number only if it is the next.
fn insert_event(conn: &Connection, event: &Event) -> Result<(), StoreError> {
    let line = serde_json::to_string(event).map_err(|error| StoreError::Io {
        context: "writing an event".to_string(),
        source: io::Error::new(io::ErrorKind::InvalidData, error),
    })?;
    // A number SQLite cannot hold is above any number the log has.
    let (Ok(seq), Ok(wall_ms)) = (i64::try_from(event.seq), i64::try_from(event.wall_ms)) else {
        return Err(StoreError::OutOfOrder {
            expected: last_seq(conn)? + 1,
            found: event.seq,
        });
    };
    let inserted = conn
        .execute(
            "INSERT INTO events (seq, wall_ms, line)
             SELECT ?1, ?2, ?3 WHERE ?1 = (SELECT COALESCE(MAX(seq), 0) + 1 FROM events)",
            params![seq, wall_ms, line],
        )
        .map_err(|e| sql_error("writing an event", e))?;
    if inserted == 0 {
        return Err(StoreError::OutOfOrder {
            expected: last_seq(conn)? + 1,
            found: event.seq,
        });
    }
    Ok(())
}

fn last_seq(conn: &Connection) -> Result<u64, StoreError> {
    conn.query_row("SELECT COALESCE(MAX(seq), 0) FROM events", [], |row| {
        row.get::<_, i64>(0)
    })
    .map(|seq| seq as u64)
    .map_err(|e| sql_error("reading the last event number", e))
}

/// The stored `(seq, line)` pairs above `after`, in order.
fn lines_since(conn: &Connection, after: u64) -> Result<Vec<(u64, String)>, StoreError> {
    let Ok(after) = i64::try_from(after) else {
        return Ok(Vec::new());
    };
    let context = "reading the event log";
    let mut statement = conn
        .prepare("SELECT seq, line FROM events WHERE seq > ?1 ORDER BY seq")
        .map_err(|e| sql_error(context, e))?;
    let rows = statement
        .query_map([after], |row| {
            Ok((row.get::<_, i64>(0)? as u64, row.get::<_, String>(1)?))
        })
        .map_err(|e| sql_error(context, e))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(|e| sql_error(context, e))
}

/// A SQLite error as a [`StoreError::Io`]. A full database (`SQLITE_FULL`) is [`io::ErrorKind::StorageFull`], so a
/// caller handles a full disk the same whether it came from a writer or from SQLite.
pub(crate) fn sql_error(context: &str, error: rusqlite::Error) -> StoreError {
    let full = matches!(
        error,
        rusqlite::Error::SqliteFailure(ref failure, _) if failure.code == ErrorCode::DiskFull
    );
    let source = if full {
        io::Error::new(io::ErrorKind::StorageFull, error)
    } else {
        io::Error::other(error)
    };
    StoreError::Io {
        context: context.to_string(),
        source,
    }
}

/// Moves `path` aside, to a name beside it that holds the time and the reason it was moved, and returns that name.
/// Nothing is deleted or overwritten, and the move is reported to the caller so that it reaches the log and the
/// Queue. The name is `<file>.unreadable-<wall ms>-<reason>`, with `-2`, `-3` and so on added when an earlier move
/// already took it.
pub fn quarantine(path: &Path, reason: &str) -> Result<PathBuf, StoreError> {
    let wall_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64);
    quarantine_at(path, reason, wall_ms)
}

/// [`quarantine`] at a chosen time, so a test can make two moves collide.
fn quarantine_at(path: &Path, reason: &str, wall_ms: u64) -> Result<PathBuf, StoreError> {
    let io_error = |source| StoreError::Io {
        context: format!("moving {} aside", path.display()),
        source,
    };
    let Some(name) = path.file_name() else {
        return Err(io_error(io::Error::new(
            io::ErrorKind::InvalidInput,
            "the path has no file name",
        )));
    };
    let mut base = name.to_os_string();
    base.push(format!(".unreadable-{wall_ms}-{}", reason_slug(reason)));
    for attempt in 1..=QUARANTINE_NAMES {
        let mut candidate = base.clone();
        if attempt > 1 {
            candidate.push(format!("-{attempt}"));
        }
        let target = path.with_file_name(candidate);
        match move_without_replacing(path, &target) {
            Ok(()) => return Ok(target),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(io_error(error)),
        }
    }
    Err(io_error(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "every name tried was taken",
    )))
}

/// A hard link to `to`, then the removal of `from`: the link fails when `to` exists, where a rename would replace it,
/// and the file's bytes are never without a name.
fn move_without_replacing(from: &Path, to: &Path) -> io::Result<()> {
    fs::hard_link(from, to)?;
    fs::remove_file(from)
}

/// The reason as it goes into a file name: lower-case letters and digits, one `-` between runs of anything else,
/// at most 32 characters.
fn reason_slug(reason: &str) -> String {
    let mut slug = String::new();
    for c in reason.chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    slug.truncate(32);
    let slug = slug.trim_end_matches('-');
    if slug.is_empty() {
        "unknown".to_string()
    } else {
        slug.to_string()
    }
}

/// [`quarantine`] for the database, which takes its WAL files with it: a fresh database beside an old WAL could replay
/// it. They get the moved name with their own suffix, which is the name SQLite looks for if the file is opened later.
/// A WAL file SQLite removed while it failed to open the database is moved from its [`keep_side_files`] name.
fn quarantine_database(path: &Path, reason: &str) -> Result<PathBuf, StoreError> {
    let moved_to = quarantine(path, reason)?;
    for suffix in SIDE_FILES {
        let original = with_suffix(path, suffix);
        let keep = with_suffix(path, &format!("{suffix}{KEEP}"));
        let source = if original.exists() { &original } else { &keep };
        if source.exists() {
            move_without_replacing(source, &with_suffix(&moved_to, suffix))
                .map_err(|source_error| io_context(source, source_error))?;
        }
        // The file just moved had two names. Removing this one deletes no bytes.
        if keep.exists() {
            fs::remove_file(&keep).map_err(|source_error| io_context(&keep, source_error))?;
        }
    }
    Ok(moved_to)
}

/// SQLite deletes the WAL file of a database it cannot read, and a WAL holds writes the database file does not yet.
/// So before the open the WAL file gets a second name (a hard link, no copy), and [`quarantine_database`] moves from
/// it when the open fails. A name left by an open that died is kept as it is. The shared-memory file is only an index
/// that SQLite rewrites in place, so it is moved when it is still there and not kept.
///
/// The link is tried, not checked first: another connection to the database (the daemon's, while a test reads the
/// store) can close between a check and the link, and SQLite then deletes the WAL. A WAL that is not there has
/// nothing to keep.
fn keep_side_files(db: &Path) -> Result<(), StoreError> {
    let original = with_suffix(db, WAL);
    let keep = with_suffix(db, &format!("{WAL}{KEEP}"));
    match fs::hard_link(&original, &keep) {
        Ok(()) => Ok(()),
        Err(error)
            if matches!(
                error.kind(),
                io::ErrorKind::NotFound | io::ErrorKind::AlreadyExists
            ) =>
        {
            Ok(())
        }
        Err(source) => Err(io_context(&keep, source)),
    }
}

/// Removes the second names of [`keep_side_files`] once the open is over and the database is usable. A name that is
/// already gone was released by another open of the same store.
fn release_side_files(db: &Path) -> Result<(), StoreError> {
    let keep = with_suffix(db, &format!("{WAL}{KEEP}"));
    match fs::remove_file(&keep) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(source) => Err(io_context(&keep, source)),
    }
}

fn io_context(path: &Path, source: io::Error) -> StoreError {
    StoreError::Io {
        context: format!("moving {} aside", path.display()),
        source,
    }
}

fn with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut name: OsString = path.as_os_str().to_os_string();
    name.push(suffix);
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use collide_core::change::{Actor, ActorKind};
    use collide_core::events::{BudgetScope, EventBody, StepKind};
    use collide_core::hook::HookExperiment;
    use collide_core::non_empty::NonEmpty;
    use collide_core::state::{PauseCause, RunState};
    use rusqlite::{Connection, OpenFlags};
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::sync::mpsc;
    use std::time::Duration;

    /// ENOSPC is 28 on macOS and on Linux.
    const ENOSPC: i32 = 28;

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A folder made for one test, in the system's temp folder. `Drop` removes the files in it by their own paths and
    /// then the folder (never a recursive delete).
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> TempDir {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir =
                std::env::temp_dir().join(format!("collide-store-{}-{n}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn path(&self) -> &Path {
            &self.0
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            if let Ok(entries) = fs::read_dir(&self.0) {
                for entry in entries.flatten() {
                    let _ = fs::remove_file(entry.path());
                }
            }
            let _ = fs::remove_dir(&self.0);
        }
    }

    fn actor(kind: ActorKind) -> Actor {
        Actor {
            kind,
            run: None,
            phase: None,
        }
    }

    fn event(seq: u64, wall_ms: u64) -> Event {
        Event {
            seq,
            wall_ms,
            actor: actor(ActorKind::Workflow),
            experiment: None,
            body: EventBody::BudgetExceeded {
                run: format!("r-{seq}"),
                scope: BudgetScope::Run,
            },
        }
    }

    fn raw(store: &Store) -> Connection {
        Connection::open_with_flags(store.db_path(), OpenFlags::SQLITE_OPEN_READ_WRITE).unwrap()
    }

    fn user_version(conn: &Connection) -> u32 {
        conn.query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap()
    }

    fn table_exists(conn: &Connection, name: &str) -> bool {
        conn.query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name = ?1",
            [name],
            |row| row.get::<_, u32>(0),
        )
        .unwrap()
            == 1
    }

    /// A database file at `version`: the first `version` migrations applied by hand, as an older build would have left it.
    fn store_at_version(dir: &Path, version: usize) {
        let conn = Connection::open(dir.join(DB_FILE)).unwrap();
        for migration in &MIGRATIONS[..version] {
            conn.execute_batch(migration.sql).unwrap();
            conn.pragma_update(None, "user_version", migration.version)
                .unwrap();
        }
    }

    fn known_version() -> u32 {
        MIGRATIONS.last().map_or(0, |m| m.version)
    }

    fn transition(seq: u64, run: &str, to: RunState) -> Event {
        Event {
            seq,
            wall_ms: 1000 + seq,
            actor: actor(ActorKind::Workflow),
            experiment: None,
            body: EventBody::RunTransition {
                run: run.to_string(),
                from: None,
                to,
            },
        }
    }

    // ---- shape ----------------------------------------------------------------------------------------------

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
    fn an_out_of_order_event_is_described_by_both_numbers() {
        let error = StoreError::OutOfOrder {
            expected: 3,
            found: 7,
        }
        .to_string();
        assert!(error.contains('3') && error.contains('7'), "{error}");
    }

    // ---- open and migrate -----------------------------------------------------------------------------------

    #[test]
    fn a_new_store_migrates_to_the_current_version() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(user_version(&raw(&store)), known_version());
    }

    #[test]
    fn opening_makes_the_folder_when_it_is_not_there() {
        let dir = TempDir::new();
        let inner = dir.path().join("daemon");
        let store = Store::open(&inner).unwrap();
        assert!(store.db_path().exists());
        let _ = fs::remove_file(store.db_path());
        for suffix in ["-wal", "-shm"] {
            let mut side = store.db_path().into_os_string();
            side.push(suffix);
            let _ = fs::remove_file(PathBuf::from(side));
        }
        let _ = fs::remove_dir(&inner);
    }

    #[test]
    fn the_store_is_in_wal_mode() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        let mode: String = raw(&store)
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "wal");
    }

    #[test]
    fn opening_again_runs_no_migration() {
        let dir = TempDir::new();
        let first = Store::open(dir.path()).unwrap();
        let mut conn = raw(&first);
        assert_eq!(migrate(&mut conn, &MIGRATIONS).unwrap(), 0);
        drop(conn);
        let second = Store::open(dir.path()).unwrap();
        assert_eq!(user_version(&raw(&second)), known_version());
    }

    #[test]
    fn opening_again_keeps_what_was_written() {
        let dir = TempDir::new();
        Store::open(dir.path())
            .unwrap()
            .append_event(&event(1, 10))
            .unwrap();
        let again = Store::open(dir.path()).unwrap();
        assert_eq!(again.events_since(0).unwrap(), vec![event(1, 10)]);
    }

    #[test]
    fn a_store_from_each_older_version_migrates_forward() {
        for older in 0..MIGRATIONS.len() {
            let dir = TempDir::new();
            store_at_version(dir.path(), older);
            let store = Store::open(dir.path()).unwrap();
            let conn = raw(&store);
            assert_eq!(user_version(&conn), known_version(), "from version {older}");
            assert!(table_exists(&conn, "events"), "from version {older}");
        }
    }

    fn usage(output: u64) -> Usage {
        Usage {
            scope: collide_core::agent::UsageScope::Turn,
            input: Some(10),
            output: Some(output),
            cache_read: None,
            cache_write: None,
            cache_write_1h: None,
            cache_write_5m: None,
            thought: None,
            context_used: None,
            context_size: None,
            cost: None,
            model_time_ms: None,
            quality: collide_core::agent::UsageQuality::Exact,
        }
    }

    #[test]
    fn a_phase_record_is_written_replaced_and_read_back() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(store.phase_record("r", "p").unwrap(), None);
        let first = PhaseRecord {
            usage: Some(usage(5)),
            failure: None,
            failure_evidence: None,
            report: None,
        };
        store.set_phase_record("r", "p", &first).unwrap();
        assert_eq!(store.phase_record("r", "p").unwrap(), Some(first));
        let last = PhaseRecord {
            usage: Some(usage(9)),
            failure: Some("no_result".to_string()),
            failure_evidence: Some("the end of stderr".to_string()),
            report: Some("done".to_string()),
        };
        store.set_phase_record("r", "p", &last).unwrap();
        assert_eq!(store.phase_record("r", "p").unwrap(), Some(last));
        assert_eq!(store.phase_record("r", "other").unwrap(), None);
    }

    #[test]
    fn a_phase_record_without_evidence_reads_back_without_it() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        let record = PhaseRecord {
            failure: Some("no_result".to_string()),
            ..PhaseRecord::default()
        };
        store.set_phase_record("r", "p", &record).unwrap();
        assert_eq!(store.phase_record("r", "p").unwrap(), Some(record));
    }

    #[test]
    fn a_store_at_version_six_gains_the_failure_evidence_column_and_keeps_its_records() {
        let dir = TempDir::new();
        store_at_version(dir.path(), 6);
        let conn = Connection::open(dir.path().join(DB_FILE)).unwrap();
        conn.execute(
            "INSERT INTO phase_records (run, phase, usage, failure, report) VALUES ('r', 'p', NULL, 'empty_turn', 'x')",
            [],
        )
        .unwrap();
        drop(conn);
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(user_version(&raw(&store)), known_version());
        assert_eq!(
            MIGRATIONS.get(6).map(|migration| migration.name),
            Some("phase_record_failure_evidence")
        );
        let old = store.phase_record("r", "p").unwrap().unwrap();
        assert_eq!(old.failure.as_deref(), Some("empty_turn"));
        assert_eq!(old.failure_evidence, None, "a record from before has none");
        let record = PhaseRecord {
            failure: Some("empty_turn".to_string()),
            failure_evidence: Some("model not found".to_string()),
            ..PhaseRecord::default()
        };
        store.set_phase_record("r", "p", &record).unwrap();
        assert_eq!(store.phase_record("r", "p").unwrap(), Some(record));
    }

    fn warning(phase: &str, files: &[&str]) -> RunWarning {
        RunWarning {
            kind: collide_core::owner::RunWarningKind::InstructionFilesChanged,
            phase: phase.to_string(),
            files: files.iter().map(|file| file.to_string()).collect(),
        }
    }

    #[test]
    fn a_store_at_version_seven_gains_the_run_warnings_table_and_every_run_has_an_empty_list() {
        let dir = TempDir::new();
        store_at_version(dir.path(), 7);
        let conn = Connection::open(dir.path().join(DB_FILE)).unwrap();
        conn.execute(
            "INSERT INTO phase_records (run, phase, usage, failure, report) VALUES ('r', 'p', NULL, NULL, 'x')",
            [],
        )
        .unwrap();
        drop(conn);
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(user_version(&raw(&store)), known_version());
        assert_eq!(
            MIGRATIONS.get(7).map(|migration| migration.name),
            Some("run_warnings")
        );
        assert!(table_exists(&raw(&store), "run_warnings"));
        assert_eq!(store.run_warnings("r").unwrap(), Vec::new());
        assert_eq!(
            store
                .phase_record("r", "p")
                .unwrap()
                .unwrap()
                .report
                .as_deref(),
            Some("x"),
            "what the old store held is kept"
        );
    }

    #[test]
    fn a_runs_warnings_are_read_back_oldest_first_and_only_that_runs() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        let first = warning("build", &["CLAUDE.md"]);
        let second = warning("review", &["sub/CLAUDE.md", ".claude/skills/a/SKILL.md"]);
        store.add_run_warning("r", &first).unwrap();
        store
            .add_run_warning("other", &warning("build", &["x/CLAUDE.md"]))
            .unwrap();
        store.add_run_warning("r", &second).unwrap();
        assert_eq!(store.run_warnings("r").unwrap(), vec![first, second]);
        assert_eq!(store.run_warnings("none").unwrap(), Vec::new());
    }

    #[test]
    fn a_runs_warnings_are_there_when_the_store_is_opened_again() {
        let dir = TempDir::new();
        let kept = warning("build", &["CLAUDE.md"]);
        Store::open(dir.path())
            .unwrap()
            .add_run_warning("r", &kept)
            .unwrap();
        assert_eq!(
            Store::open(dir.path()).unwrap().run_warnings("r").unwrap(),
            vec![kept]
        );
    }

    #[test]
    fn a_store_at_version_one_gains_the_phase_record_table() {
        let dir = TempDir::new();
        store_at_version(dir.path(), 1);
        let store = Store::open(dir.path()).unwrap();
        assert!(table_exists(&raw(&store), "phase_records"));
        assert_eq!(user_version(&raw(&store)), known_version());
    }

    #[test]
    fn a_store_at_version_four_gains_the_board_and_scratch_task_tables() {
        let dir = TempDir::new();
        store_at_version(dir.path(), 4);
        let store = Store::open(dir.path()).unwrap();
        assert!(table_exists(&raw(&store), "boards"));
        assert!(table_exists(&raw(&store), "scratch_tasks"));
        assert_eq!(user_version(&raw(&store)), known_version());
    }

    #[test]
    fn the_first_migration_makes_the_event_table() {
        let dir = TempDir::new();
        store_at_version(dir.path(), 1);
        let conn = Connection::open(dir.path().join(DB_FILE)).unwrap();
        assert!(table_exists(&conn, "events"));
        assert_eq!(user_version(&conn), 1);
    }

    #[test]
    fn migrations_run_in_order_from_the_version_the_store_is_at() {
        let list = [
            Migration {
                version: 1,
                name: "a",
                sql: "CREATE TABLE a (x)",
            },
            Migration {
                version: 2,
                name: "b",
                sql: "CREATE TABLE b (x)",
            },
            Migration {
                version: 3,
                name: "c",
                sql: "CREATE TABLE c (x)",
            },
        ];
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch("CREATE TABLE a (x)").unwrap();
        conn.pragma_update(None, "user_version", 1).unwrap();
        assert_eq!(migrate(&mut conn, &list).unwrap(), 2);
        assert_eq!(user_version(&conn), 3);
        assert!(table_exists(&conn, "b") && table_exists(&conn, "c"));
    }

    #[test]
    fn a_migration_that_fails_leaves_the_store_at_the_version_before() {
        let list = [
            Migration {
                version: 1,
                name: "a",
                sql: "CREATE TABLE a (x)",
            },
            Migration {
                version: 2,
                name: "broken",
                sql: "CREATE TABLE b (x); NOT SQL",
            },
        ];
        let mut conn = Connection::open_in_memory().unwrap();
        assert!(matches!(
            migrate(&mut conn, &list),
            Err(StoreError::Io { .. })
        ));
        assert_eq!(user_version(&conn), 1);
        assert!(!table_exists(&conn, "b"));
    }

    #[test]
    fn a_store_newer_than_the_build_is_refused_and_left_where_it_is() {
        let dir = TempDir::new();
        drop(Store::open(dir.path()).unwrap());
        let path = dir.path().join(DB_FILE);
        let newer = known_version() + 1;
        Connection::open(&path)
            .unwrap()
            .pragma_update(None, "user_version", newer)
            .unwrap();
        match Store::open(dir.path()) {
            Err(StoreError::TooNew { found, known }) => {
                assert_eq!((found, known), (newer, known_version()));
            }
            other => panic!("expected TooNew, got {other:?}"),
        }
        assert!(path.exists(), "a newer store is not moved aside");
    }

    // ---- the event log --------------------------------------------------------------------------------------

    #[test]
    fn events_come_back_in_the_order_they_were_written() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        for seq in 1..=20 {
            store.append_event(&event(seq, 1000 + seq)).unwrap();
        }
        let seqs: Vec<u64> = store
            .events_since(0)
            .unwrap()
            .iter()
            .map(|e| e.seq)
            .collect();
        assert_eq!(seqs, (1..=20).collect::<Vec<u64>>());
    }

    #[test]
    fn an_event_comes_back_with_its_actor_experiment_and_time() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        let first = Event {
            seq: 1,
            wall_ms: 1_700_000_000_500,
            actor: Actor {
                kind: ActorKind::Agent,
                run: Some("r-1".to_string()),
                phase: Some("build".to_string()),
            },
            experiment: Some(HookExperiment {
                id: "e-1".to_string(),
                arm: "a".to_string(),
                dir: "/arms/a".to_string(),
            }),
            body: EventBody::StepIntent {
                step: "s-1".to_string(),
                kind: StepKind::Integrate,
                action: "merge".to_string(),
            },
        };
        // The wall clock can jump back, so a later event may carry an earlier time. It is stored as it was written.
        let second = Event {
            actor: actor(ActorKind::Person),
            ..event(2, 1_700_000_000_100)
        };
        store.append_event(&first).unwrap();
        store.append_event(&second).unwrap();
        assert_eq!(store.events_since(0).unwrap(), vec![first, second]);
    }

    #[test]
    fn events_since_returns_only_the_events_after_the_number() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        for seq in 1..=5 {
            store.append_event(&event(seq, seq)).unwrap();
        }
        let seqs: Vec<u64> = store
            .events_since(3)
            .unwrap()
            .iter()
            .map(|e| e.seq)
            .collect();
        assert_eq!(seqs, vec![4, 5]);
        assert!(store.events_since(5).unwrap().is_empty());
        assert!(store.events_since(u64::MAX).unwrap().is_empty());
    }

    #[test]
    fn an_empty_log_has_last_number_zero() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(store.last_seq().unwrap(), 0);
        store.append_event(&event(1, 1)).unwrap();
        assert_eq!(store.last_seq().unwrap(), 1);
    }

    #[test]
    fn a_repeated_event_number_is_refused() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        store.append_event(&event(1, 1)).unwrap();
        match store.append_event(&event(1, 2)) {
            Err(StoreError::OutOfOrder { expected, found }) => {
                assert_eq!((expected, found), (2, 1))
            }
            other => panic!("expected OutOfOrder, got {other:?}"),
        }
        assert_eq!(store.events_since(0).unwrap().len(), 1);
    }

    #[test]
    fn a_gap_in_the_event_numbers_is_refused() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        store.append_event(&event(1, 1)).unwrap();
        match store.append_event(&event(3, 2)) {
            Err(StoreError::OutOfOrder { expected, found }) => {
                assert_eq!((expected, found), (2, 3))
            }
            other => panic!("expected OutOfOrder, got {other:?}"),
        }
    }

    #[test]
    fn the_first_event_is_number_one() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        assert!(matches!(
            store.append_event(&event(0, 1)),
            Err(StoreError::OutOfOrder {
                expected: 1,
                found: 0
            })
        ));
        assert!(matches!(
            store.append_event(&event(u64::MAX, 1)),
            Err(StoreError::OutOfOrder { .. })
        ));
    }

    #[test]
    fn a_line_this_build_cannot_read_is_an_error_naming_its_number() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        raw(&store)
            .execute(
                "INSERT INTO events (seq, wall_ms, line) VALUES (1, 1, '{\"event\":\"from.the.future\"}')",
                [],
            )
            .unwrap();
        match store.events_since(0) {
            Err(StoreError::Io { context, source }) => {
                assert!(context.contains('1'), "{context}");
                assert_eq!(source.kind(), io::ErrorKind::InvalidData);
            }
            other => panic!("expected Io, got {other:?}"),
        }
    }

    // ---- the log through a writer, and a full disk ----------------------------------------------------------

    /// Fails with `ENOSPC` on the write numbered `fail_on` (from 0), and keeps what the writes before it took.
    struct FullDisk {
        fail_on: usize,
        writes: usize,
        taken: Vec<u8>,
    }

    impl FullDisk {
        fn on_write(fail_on: usize) -> FullDisk {
            FullDisk {
                fail_on,
                writes: 0,
                taken: Vec::new(),
            }
        }
    }

    impl io::Write for FullDisk {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.writes == self.fail_on {
                return Err(io::Error::from_raw_os_error(ENOSPC));
            }
            self.writes += 1;
            self.taken.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    fn store_with_events(dir: &TempDir, count: u64) -> Store {
        let store = Store::open(dir.path()).unwrap();
        for seq in 1..=count {
            store.append_event(&event(seq, seq)).unwrap();
        }
        store
    }

    #[test]
    fn the_log_is_written_as_one_json_line_per_event() {
        let dir = TempDir::new();
        let store = store_with_events(&dir, 3);
        let mut out = Vec::new();
        assert_eq!(store.write_events_since(1, &mut out).unwrap(), 2);
        let text = String::from_utf8(out).unwrap();
        let events: Vec<Event> = text
            .lines()
            .map(|line| serde_json::from_str(line).unwrap())
            .collect();
        assert_eq!(events, vec![event(2, 2), event(3, 3)]);
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn a_write_that_fails_with_enospc_is_a_store_error_whichever_write_it_is() {
        let dir = TempDir::new();
        let store = store_with_events(&dir, 3);
        for fail_on in 0..3 {
            let mut disk = FullDisk::on_write(fail_on);
            match store.write_events_since(0, &mut disk) {
                Err(StoreError::Io { source, .. }) => {
                    assert_eq!(source.raw_os_error(), Some(ENOSPC), "write {fail_on}");
                    assert_eq!(source.kind(), io::ErrorKind::StorageFull);
                }
                other => panic!("write {fail_on}: expected Io, got {other:?}"),
            }
            assert_eq!(disk.writes, fail_on, "nothing is written after the failure");
        }
    }

    #[test]
    fn a_flush_that_fails_is_a_store_error() {
        struct FlushFails;
        impl io::Write for FlushFails {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                Ok(buf.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::from_raw_os_error(ENOSPC))
            }
        }
        let dir = TempDir::new();
        let store = store_with_events(&dir, 1);
        assert!(matches!(
            store.write_events_since(0, &mut FlushFails),
            Err(StoreError::Io { .. })
        ));
    }

    #[test]
    fn a_database_that_is_full_is_a_store_error_and_keeps_its_events() {
        let dir = TempDir::new();
        let store = store_with_events(&dir, 1);
        let conn = raw(&store);
        // SQLite's own full disk, on demand: the file may not grow past the pages it has.
        let pages: i64 = conn
            .query_row("PRAGMA page_count", [], |row| row.get(0))
            .unwrap();
        conn.pragma_update(None, "max_page_count", pages).unwrap();
        let big = Event {
            body: EventBody::StepIntent {
                step: "s".to_string(),
                kind: StepKind::SourceOp,
                action: "x".repeat(64 * 1024),
            },
            ..event(2, 2)
        };
        match insert_event(&conn, &big) {
            Err(StoreError::Io { source, .. }) => {
                assert_eq!(source.kind(), io::ErrorKind::StorageFull);
            }
            other => panic!("expected Io, got {other:?}"),
        }
        assert_eq!(store.events_since(0).unwrap(), vec![event(1, 1)]);
    }

    // ---- quarantine -----------------------------------------------------------------------------------------

    fn write_file(path: &Path, bytes: &[u8]) {
        fs::write(path, bytes).unwrap();
    }

    fn quarantined(result: Result<Store, StoreError>) -> PathBuf {
        match result {
            Err(StoreError::Quarantined { moved_to, .. }) => moved_to,
            other => panic!("expected Quarantined, got {other:?}"),
        }
    }

    #[test]
    fn a_store_of_garbage_bytes_is_moved_aside_whole_and_a_fresh_store_opens() {
        let dir = TempDir::new();
        let garbage: Vec<u8> = (0..20_000u32).map(|n| (n * 31 % 251) as u8).collect();
        write_file(&dir.path().join(DB_FILE), &garbage);
        let moved_to = quarantined(Store::open(dir.path()));
        assert_eq!(fs::read(&moved_to).unwrap(), garbage);
        assert_eq!(moved_to.parent(), Some(dir.path()));
        let fresh = Store::open(dir.path()).unwrap();
        assert!(fresh.events_since(0).unwrap().is_empty());
        assert_eq!(fs::read(&moved_to).unwrap(), garbage, "left whole");
    }

    #[test]
    fn a_store_of_a_few_bytes_is_moved_aside() {
        let dir = TempDir::new();
        write_file(&dir.path().join(DB_FILE), b"not sqlite");
        let moved_to = quarantined(Store::open(dir.path()));
        assert_eq!(fs::read(&moved_to).unwrap(), b"not sqlite");
        assert!(Store::open(dir.path()).is_ok());
    }

    #[test]
    fn a_truncated_store_is_moved_aside_whole_and_a_fresh_store_opens() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        for seq in 1..=60 {
            let big = Event {
                body: EventBody::StepIntent {
                    step: format!("s-{seq}"),
                    kind: StepKind::RemoteWrite,
                    action: "y".repeat(2000),
                },
                ..event(seq, seq)
            };
            store.append_event(&big).unwrap();
        }
        let path = store.db_path();
        raw(&store)
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
            .unwrap();
        let mut bytes = fs::read(&path).unwrap();
        assert!(bytes.len() > 40_000, "the file holds many pages");
        bytes.truncate(bytes.len() / 2);
        drop(store);
        for suffix in ["-wal", "-shm"] {
            let mut side = path.clone().into_os_string();
            side.push(suffix);
            let _ = fs::remove_file(PathBuf::from(side));
        }
        write_file(&path, &bytes);
        let moved_to = quarantined(Store::open(dir.path()));
        assert_eq!(fs::read(&moved_to).unwrap(), bytes);
        let fresh = Store::open(dir.path()).unwrap();
        assert!(fresh.events_since(0).unwrap().is_empty());
    }

    #[test]
    fn a_database_file_cut_to_one_page_is_moved_aside() {
        let dir = TempDir::new();
        let store = store_with_events(&dir, 1);
        let path = store.db_path();
        raw(&store)
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
            .unwrap();
        drop(store);
        let file = fs::OpenOptions::new().write(true).open(&path).unwrap();
        file.set_len(512).unwrap();
        drop(file);
        quarantined(Store::open(dir.path()));
    }

    #[test]
    fn the_wal_files_of_an_unreadable_store_move_with_it() {
        let dir = TempDir::new();
        let path = dir.path().join(DB_FILE);
        write_file(&path, &vec![7u8; 5000]);
        write_file(&dir.path().join("store.db-wal"), b"wal bytes");
        write_file(&dir.path().join("store.db-shm"), b"shm bytes");
        let moved_to = quarantined(Store::open(dir.path()));
        // SQLite removes a WAL it cannot use while it opens the file, so the WAL is checked here and the index is not.
        assert_eq!(
            fs::read(with_suffix(&moved_to, "-wal")).unwrap(),
            b"wal bytes"
        );
        // The fresh store starts without the old WAL, so it cannot replay it.
        let fresh = Store::open(dir.path()).unwrap();
        assert!(fresh.events_since(0).unwrap().is_empty());
    }

    #[test]
    fn opening_a_store_with_a_wal_leaves_no_second_names_behind() {
        let dir = TempDir::new();
        let store = store_with_events(&dir, 3);
        // While this connection is open the WAL file stays, as it does after a crash.
        let held = raw(&store);
        held.execute_batch("BEGIN").unwrap();
        held.query_row("SELECT COUNT(*) FROM events", [], |row| {
            row.get::<_, u32>(0)
        })
        .unwrap();
        assert!(with_suffix(&store.db_path(), "-wal").exists());
        let again = Store::open(dir.path()).unwrap();
        assert_eq!(again.events_since(0).unwrap().len(), 3);
        assert!(!with_suffix(&store.db_path(), &format!("{WAL}{KEEP}")).exists());
        held.execute_batch("COMMIT").unwrap();
    }

    #[test]
    fn keeping_the_side_files_of_a_database_with_no_wal_makes_no_second_name() {
        let dir = TempDir::new();
        let path = dir.path().join(DB_FILE);
        keep_side_files(&path).unwrap();
        assert!(!with_suffix(&path, &format!("{WAL}{KEEP}")).exists());
    }

    #[test]
    fn a_second_name_left_by_an_open_that_died_is_kept_as_it_is() {
        let dir = TempDir::new();
        let path = dir.path().join(DB_FILE);
        write_file(&with_suffix(&path, WAL), b"new wal");
        write_file(&with_suffix(&path, &format!("{WAL}{KEEP}")), b"old wal");
        keep_side_files(&path).unwrap();
        assert_eq!(
            fs::read(with_suffix(&path, &format!("{WAL}{KEEP}"))).unwrap(),
            b"old wal"
        );
    }

    #[test]
    fn releasing_a_second_name_that_is_already_gone_is_not_an_error() {
        let dir = TempDir::new();
        release_side_files(&dir.path().join(DB_FILE)).unwrap();
    }

    #[test]
    fn a_second_quarantine_of_the_same_name_gets_a_different_path() {
        let dir = TempDir::new();
        let path = dir.path().join("exit.json");
        write_file(&path, b"first");
        let first = quarantine_at(&path, "not json", 1234).unwrap();
        write_file(&path, b"second");
        let second = quarantine_at(&path, "not json", 1234).unwrap();
        write_file(&path, b"third");
        let third = quarantine_at(&path, "not json", 1234).unwrap();
        assert!(first != second && second != third && first != third);
        assert_eq!(fs::read(&first).unwrap(), b"first");
        assert_eq!(fs::read(&second).unwrap(), b"second");
        assert_eq!(fs::read(&third).unwrap(), b"third");
        assert!(!path.exists());
    }

    #[test]
    fn two_unreadable_stores_in_a_row_are_both_kept() {
        let dir = TempDir::new();
        let path = dir.path().join(DB_FILE);
        write_file(&path, b"first garbage");
        let first = quarantined(Store::open(dir.path()));
        // The fresh store is made only on the next open, so garbage can be put back in its place.
        let _ = fs::remove_file(&path);
        write_file(&path, b"second garbage");
        let second = quarantined(Store::open(dir.path()));
        assert_ne!(first, second);
        assert_eq!(fs::read(&first).unwrap(), b"first garbage");
        assert_eq!(fs::read(&second).unwrap(), b"second garbage");
    }

    #[test]
    fn the_new_name_holds_the_time_and_the_reason() {
        let dir = TempDir::new();
        let path = dir.path().join("exit.json");
        write_file(&path, b"{");
        let moved = quarantine_at(&path, "Not JSON: EOF at line 1!", 1_700_000_000_123).unwrap();
        let name = moved.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(
            name,
            "exit.json.unreadable-1700000000123-not-json-eof-at-line-1"
        );
    }

    #[test]
    fn quarantining_a_file_that_is_not_there_is_an_io_error() {
        let dir = TempDir::new();
        assert!(matches!(
            quarantine(&dir.path().join("missing.json"), "gone"),
            Err(StoreError::Io { .. })
        ));
    }

    #[test]
    fn a_reason_with_nothing_to_keep_still_gives_a_name() {
        let dir = TempDir::new();
        let path = dir.path().join("a");
        write_file(&path, b"x");
        let moved = quarantine_at(&path, "???", 5).unwrap();
        assert_eq!(moved.file_name().unwrap(), "a.unreadable-5-unknown");
    }

    // ---- no lock other work waits on ------------------------------------------------------------------------

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn the_store_can_be_shared_between_threads_without_a_lock() {
        assert_send_sync::<Store>();
    }

    #[test]
    fn nothing_is_held_between_calls() {
        let dir = TempDir::new();
        let store = store_with_events(&dir, 2);
        // An exclusive transaction from another connection would wait on any lock the store kept.
        let mut conn = raw(&store);
        conn.busy_timeout(Duration::from_millis(0)).unwrap();
        let tx = conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Exclusive)
            .unwrap();
        tx.rollback().unwrap();
    }

    #[test]
    fn an_append_does_not_wait_for_a_reader_that_stays_open() {
        let dir = TempDir::new();
        let store = store_with_events(&dir, 1);
        // A reader inside a transaction, which it never ends while the append runs.
        let reader = raw(&store);
        reader.execute_batch("BEGIN").unwrap();
        let before: u32 = reader
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        let (done, finished) = mpsc::channel();
        std::thread::scope(|scope| {
            scope.spawn(|| {
                done.send(store.append_event(&event(2, 2))).unwrap();
            });
            let result = finished
                .recv_timeout(Duration::from_secs(3))
                .expect("the append waited for the reader");
            result.unwrap();
        });
        let during: u32 = reader
            .query_row("SELECT COUNT(*) FROM events", [], |row| row.get(0))
            .unwrap();
        assert_eq!((before, during), (1, 1), "the reader keeps its snapshot");
        reader.execute_batch("COMMIT").unwrap();
        assert_eq!(store.last_seq().unwrap(), 2);
    }

    #[test]
    fn reads_go_on_while_events_are_appended_from_another_thread() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        let total = 150u64;
        std::thread::scope(|scope| {
            scope.spawn(|| {
                for seq in 1..=total {
                    store.append_event(&event(seq, seq)).unwrap();
                }
            });
            scope.spawn(|| {
                let mut seen = 0u64;
                while seen < total {
                    for e in store.events_since(seen).unwrap() {
                        assert_eq!(e.seq, seen + 1, "events come back in order, with no gap");
                        seen = e.seq;
                    }
                }
            });
        });
        assert_eq!(store.last_seq().unwrap(), total);
    }
    // ---- the index of runs ------------------------------------------------------------------------------------

    #[test]
    fn migrating_a_store_that_has_events_gives_each_run_its_row_from_its_first_and_last_transition()
    {
        let dir = TempDir::new();
        store_at_version(dir.path(), 5);
        let conn = Connection::open(dir.path().join(DB_FILE)).unwrap();
        let ended = RunState::Ended {
            outcome: NonEmpty::new("done").unwrap(),
        };
        let paused = RunState::Paused {
            cause: PauseCause::Disk,
        };
        let events = [
            transition(1, "a", RunState::Running {}),
            transition(2, "b", RunState::Running {}),
            event(3, 3),
            transition(4, "a", ended.clone()),
            transition(5, "b", paused.clone()),
        ];
        for event in &events {
            insert_event(&conn, event).unwrap();
        }
        drop(conn);

        let store = Store::open(dir.path()).unwrap();
        assert_eq!(store.run_state("a").unwrap(), Some(ended));
        assert_eq!(store.run_state("b").unwrap(), Some(paused));
        let a = store.run("a").unwrap().unwrap();
        assert_eq!(
            (a.opened_seq, a.started_ms, a.ended_ms),
            (1, 1001, Some(1004))
        );
        let b = store.run("b").unwrap().unwrap();
        assert_eq!((b.opened_seq, b.started_ms, b.ended_ms), (2, 1002, None));
        assert_eq!(
            (b.board, b.line, b.workflow),
            (None, String::new(), String::new())
        );
    }

    #[test]
    fn migrating_a_store_with_no_runs_leaves_the_index_empty() {
        let dir = TempDir::new();
        store_at_version(dir.path(), 5);
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(store.run_state("a").unwrap(), None);
        assert_eq!(user_version(&raw(&store)), known_version());
    }

    #[test]
    fn a_run_transition_moves_the_row_with_the_event_and_an_event_that_is_refused_moves_nothing() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        store
            .append_event(&transition(1, "a", RunState::Running {}))
            .unwrap();
        let refused = store.append_event(&transition(5, "a", RunState::Failed {}));
        assert!(matches!(refused, Err(StoreError::OutOfOrder { .. })));
        assert_eq!(store.run_state("a").unwrap(), Some(RunState::Running {}));
        store
            .append_event(&transition(2, "a", RunState::Failed {}))
            .unwrap();
        assert_eq!(store.run_state("a").unwrap(), Some(RunState::Failed {}));
    }

    #[test]
    fn an_event_that_is_not_a_run_transition_makes_no_row() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        store.append_event(&event(1, 1)).unwrap();
        assert_eq!(store.run_state("r-1").unwrap(), None);
    }

    #[test]
    fn the_oldest_seq_is_the_first_events_and_for_an_empty_log_the_one_after_the_head() {
        let dir = TempDir::new();
        let store = Store::open(dir.path()).unwrap();
        assert_eq!(store.oldest_seq().unwrap(), 1);
        store.append_event(&event(1, 1)).unwrap();
        store.append_event(&event(2, 2)).unwrap();
        assert_eq!(store.oldest_seq().unwrap(), 1);
        raw(&store)
            .execute("DELETE FROM events WHERE seq = 1", [])
            .unwrap();
        assert_eq!(store.oldest_seq().unwrap(), 2);
    }
}
