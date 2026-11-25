//! The runs in the store (ts-0076-cw; 0.8, Run; Stage 3c). The tables are migration 6 of the store
//! ([`crate::store::MIGRATIONS`]); this file is the writes and reads on them.
//!
//! - **`runs` has one row for each run: its board, line and workflow, its latest state and its times.** The row is
//!   written by [`Store::append_event`] in the same transaction as the `run.transition` event ([`note_transition`]), so
//!   the two never disagree, and a run that has an event has a row. `run.list`, `run.get` and the run-state read
//!   ([`Store::run_state`]) read it, so none scans the event log.
//! - **The board and the tasks are not in the event.** `run.start` links them with [`Store::set_run_launch`] once the
//!   run has its first event. A run started by `run.start_file` has no board, no tasks, and no line or workflow, which
//!   read as empty text. It has its task file, which [`Store::set_run_task_file`] keeps (migration 11).
//! - **`run_tasks` holds a run's task keys in the order they were given**, each with its status when the run started,
//!   so a read by task is an index lookup.
//! - **Newest first** is `opened_seq` descending: the `seq` of the run's first event, which is also the cursor of a
//!   page.
//!
//! A run's state is final when it is `ended`, `failed` or `stopped`; it is live otherwise, which includes
//! `integrating`.

use crate::store::{Store, StoreError, sql_error};
use collide_core::owner::{RunScope, RunTaskRef};
use collide_core::state::RunState;
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params, params_from_iter};
use std::io;

/// What `run.start` knows about a run that its `run.transition` event does not say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunLaunch {
    /// The board's name, or `None` for a run that has no board.
    pub board: Option<String>,
    pub tasks: Vec<RunTaskRef>,
    pub line: String,
    /// `pack/workflow`.
    pub workflow: String,
}

/// The commits a phase started and ended at (migration 12), which a task's changes are read from. A phase of a run
/// from before the migration has no row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PhaseHeads {
    pub phase: String,
    pub start_head: String,
    /// `None` while the phase is active, and for a phase whose end the daemon did not see.
    pub end_head: Option<String>,
}

/// One run as the store holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunRow {
    pub run: String,
    pub board: Option<String>,
    pub line: String,
    pub workflow: String,
    pub state: RunState,
    /// The `seq` of the run's first event.
    pub opened_seq: u64,
    pub started_ms: u64,
    /// When the run reached a final state, or `None` while it has not.
    pub ended_ms: Option<u64>,
    pub tasks: Vec<RunTaskRef>,
    /// The task file of a run of a file, or `None`.
    pub task_file: Option<String>,
}

/// Which runs [`Store::runs`] reads: all that match every part that is given.
#[derive(Debug, Clone, Copy)]
pub struct RunFilter<'a> {
    pub board: Option<&'a str>,
    pub task: Option<&'a str>,
    pub scope: RunScope,
    /// Only the runs opened before this `seq`: the cursor of the page before.
    pub before: Option<u64>,
    pub limit: u32,
}

/// Whether the run is still going: not `ended`, `failed` or `stopped`.
pub fn is_live(state: &RunState) -> bool {
    !matches!(
        state,
        RunState::Ended { .. } | RunState::Failed {} | RunState::Stopped {}
    )
}

fn unreadable(run: &str, error: serde_json::Error) -> StoreError {
    StoreError::Io {
        context: format!("reading the state of run {run}"),
        source: io::Error::new(io::ErrorKind::InvalidData, error),
    }
}

fn to_i64(number: u64) -> i64 {
    i64::try_from(number).unwrap_or(i64::MAX)
}

/// Writes the row of the run an event moved to `to`: made at its first event, and moved to the new state at each one
/// after. The caller is inside the transaction that wrote the event.
pub(crate) fn note_transition(
    conn: &Connection,
    seq: u64,
    wall_ms: u64,
    run: &str,
    to: &RunState,
) -> Result<(), StoreError> {
    let state = serde_json::to_string(to).map_err(|error| unreadable(run, error))?;
    let live = is_live(to);
    let ended_ms = (!live).then(|| to_i64(wall_ms));
    conn.execute(
        "INSERT INTO runs (run, state, live, opened_seq, started_ms, ended_ms) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT (run) DO UPDATE SET state = excluded.state, live = excluded.live, ended_ms = excluded.ended_ms",
        params![run, state, live, to_i64(seq), to_i64(wall_ms), ended_ms],
    )
    .map_err(|e| sql_error("writing a run", e))?;
    Ok(())
}

const COLUMNS: &str =
    "run, board, line, workflow, state, opened_seq, started_ms, ended_ms, task_file";

/// A row of [`COLUMNS`], with no tasks yet.
fn row_of(row: &rusqlite::Row<'_>) -> rusqlite::Result<(RunRow, String)> {
    let run: String = row.get(0)?;
    let state: String = row.get(4)?;
    Ok((
        RunRow {
            run,
            board: row.get(1)?,
            line: row.get(2)?,
            workflow: row.get(3)?,
            state: RunState::Running {},
            opened_seq: row.get::<_, i64>(5)? as u64,
            started_ms: row.get::<_, i64>(6)? as u64,
            ended_ms: row.get::<_, Option<i64>>(7)?.map(|ms| ms as u64),
            tasks: Vec::new(),
            task_file: row.get(8)?,
        },
        state,
    ))
}

impl Store {
    /// The run's latest state, from its row. `None` for a run that has no `run.transition` event.
    pub fn run_state(&self, run: &str) -> Result<Option<RunState>, StoreError> {
        let state: Option<String> = self
            .connect()?
            .query_row("SELECT state FROM runs WHERE run = ?1", [run], |row| {
                row.get(0)
            })
            .optional()
            .map_err(|e| sql_error("reading a run's state", e))?;
        state
            .map(|text| serde_json::from_str(&text).map_err(|error| unreadable(run, error)))
            .transpose()
    }

    /// Links a run to its board, tasks, line and workflow, replacing what it had. `false` when the run has no row yet,
    /// which is a run with no `run.transition` event: nothing is written.
    pub fn set_run_launch(&self, run: &str, launch: &RunLaunch) -> Result<bool, StoreError> {
        let context = "linking a run";
        let mut conn = self.connect()?;
        let tx = conn
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|e| sql_error(context, e))?;
        let changed = tx
            .execute(
                "UPDATE runs SET board = ?2, line = ?3, workflow = ?4 WHERE run = ?1",
                params![run, launch.board, launch.line, launch.workflow],
            )
            .map_err(|e| sql_error(context, e))?;
        if changed == 0 {
            return Ok(false);
        }
        tx.execute("DELETE FROM run_tasks WHERE run = ?1", [run])
            .map_err(|e| sql_error(context, e))?;
        for (order, task) in launch.tasks.iter().enumerate() {
            tx.execute(
                "INSERT INTO run_tasks (run, ord, task, status) VALUES (?1, ?2, ?3, ?4)",
                params![run, to_i64(order as u64), task.key, task.status],
            )
            .map_err(|e| sql_error(context, e))?;
        }
        tx.commit().map_err(|e| sql_error(context, e))?;
        Ok(true)
    }

    /// Keeps the task file a run of a file was started from. `false` when the run has no row yet.
    pub fn set_run_task_file(&self, run: &str, task_file: &str) -> Result<bool, StoreError> {
        let changed = self
            .connect()?
            .execute(
                "UPDATE runs SET task_file = ?2 WHERE run = ?1",
                params![run, task_file],
            )
            .map_err(|e| sql_error("writing a run's task file", e))?;
        Ok(changed > 0)
    }

    /// Keeps the commit a phase started at. The first write stays: a phase that resumes (a usage limit, a disk pause)
    /// started once. `false` when the phase already has its start head.
    pub fn set_phase_start_head(
        &self,
        run: &str,
        phase: &str,
        head: &str,
    ) -> Result<bool, StoreError> {
        let changed = self
            .connect()?
            .execute(
                "INSERT OR IGNORE INTO phase_heads (run, phase, start_head) VALUES (?1, ?2, ?3)",
                params![run, phase, head],
            )
            .map_err(|e| sql_error("writing a phase's start head", e))?;
        Ok(changed > 0)
    }

    /// Keeps the commit a phase ended at. `false` when the phase has no start head, so there is nothing to end.
    pub fn set_phase_end_head(
        &self,
        run: &str,
        phase: &str,
        head: &str,
    ) -> Result<bool, StoreError> {
        let changed = self
            .connect()?
            .execute(
                "UPDATE phase_heads SET end_head = ?3 WHERE run = ?1 AND phase = ?2",
                params![run, phase, head],
            )
            .map_err(|e| sql_error("writing a phase's end head", e))?;
        Ok(changed > 0)
    }

    /// The heads of each phase of a run, in the order the phases started. Empty for a run from before they were kept.
    pub fn phase_heads(&self, run: &str) -> Result<Vec<PhaseHeads>, StoreError> {
        let conn = self.connect()?;
        let mut statement = conn
            .prepare(
                "SELECT phase, start_head, end_head FROM phase_heads WHERE run = ?1 ORDER BY rowid",
            )
            .map_err(|e| sql_error("reading a run's phase heads", e))?;
        let rows = statement
            .query_map([run], |row| {
                Ok(PhaseHeads {
                    phase: row.get(0)?,
                    start_head: row.get(1)?,
                    end_head: row.get(2)?,
                })
            })
            .map_err(|e| sql_error("reading a run's phase heads", e))?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|e| sql_error("reading a run's phase heads", e))
    }

    /// One run, or `None` for a run the store has no row of.
    pub fn run(&self, run: &str) -> Result<Option<RunRow>, StoreError> {
        let conn = self.connect()?;
        let found = conn
            .query_row(
                &format!("SELECT {COLUMNS} FROM runs WHERE run = ?1"),
                [run],
                row_of,
            )
            .optional()
            .map_err(|e| sql_error("reading a run", e))?;
        found.map(|row| finish(&conn, row)).transpose()
    }

    /// The runs that match `filter`, newest first.
    pub fn runs(&self, filter: &RunFilter<'_>) -> Result<Vec<RunRow>, StoreError> {
        let mut clauses: Vec<&str> = Vec::new();
        let mut values: Vec<Value> = Vec::new();
        if let Some(board) = filter.board {
            clauses.push("board = ?");
            values.push(Value::Text(board.to_string()));
        }
        if let Some(task) = filter.task {
            clauses.push("run IN (SELECT run FROM run_tasks WHERE task = ?)");
            values.push(Value::Text(task.to_string()));
        }
        match filter.scope {
            RunScope::Live => clauses.push("live = 1"),
            RunScope::Finished => clauses.push("live = 0"),
            RunScope::All => {}
        }
        if let Some(before) = filter.before {
            clauses.push("opened_seq < ?");
            values.push(Value::Integer(to_i64(before)));
        }
        values.push(Value::Integer(i64::from(filter.limit)));
        let condition = if clauses.is_empty() {
            String::new()
        } else {
            format!("WHERE {}", clauses.join(" AND "))
        };
        let context = "reading the runs";
        let conn = self.connect()?;
        let mut statement = conn
            .prepare(&format!(
                "SELECT {COLUMNS} FROM runs {condition} ORDER BY opened_seq DESC LIMIT ?"
            ))
            .map_err(|e| sql_error(context, e))?;
        let rows = statement
            .query_map(params_from_iter(values), row_of)
            .map_err(|e| sql_error(context, e))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| sql_error(context, e))?;
        rows.into_iter().map(|row| finish(&conn, row)).collect()
    }
}

/// The row with its state read and its tasks.
fn finish(conn: &Connection, (mut row, state): (RunRow, String)) -> Result<RunRow, StoreError> {
    row.state = serde_json::from_str(&state).map_err(|error| unreadable(&row.run, error))?;
    let context = "reading a run's tasks";
    let mut statement = conn
        .prepare("SELECT task, status FROM run_tasks WHERE run = ?1 ORDER BY ord")
        .map_err(|e| sql_error(context, e))?;
    row.tasks = statement
        .query_map([&row.run], |task| {
            Ok(RunTaskRef {
                key: task.get(0)?,
                status: task.get(1)?,
            })
        })
        .map_err(|e| sql_error(context, e))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| sql_error(context, e))?;
    Ok(row)
}

#[cfg(test)]
mod tests {
    use super::*;
    use collide_core::change::{Actor, ActorKind};
    use collide_core::events::{Event, EventBody};
    use collide_core::non_empty::NonEmpty;
    use collide_core::state::PauseCause;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    static COUNTER: AtomicU32 = AtomicU32::new(0);

    /// A folder made for one test; `Drop` removes the files in it by their own paths and then the folder.
    struct TempDir(PathBuf);

    impl TempDir {
        fn new() -> TempDir {
            let n = COUNTER.fetch_add(1, Ordering::SeqCst);
            let dir =
                std::env::temp_dir().join(format!("collide-run-store-{}-{n}", std::process::id()));
            fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
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

    #[test]
    fn a_phase_keeps_its_first_start_head_and_gains_an_end_head() {
        let dir = TempDir::new();
        let store = Store::open(&dir.0).unwrap();

        assert!(store.set_phase_start_head("r", "build", "aaa").unwrap());
        assert!(!store.set_phase_start_head("r", "build", "bbb").unwrap());
        assert!(store.set_phase_end_head("r", "build", "ccc").unwrap());

        assert_eq!(
            store.phase_heads("r").unwrap(),
            [PhaseHeads {
                phase: "build".into(),
                start_head: "aaa".into(),
                end_head: Some("ccc".into())
            }]
        );
    }

    #[test]
    fn an_end_head_for_a_phase_that_has_no_start_head_is_not_kept() {
        let dir = TempDir::new();
        let store = Store::open(&dir.0).unwrap();

        assert!(!store.set_phase_end_head("r", "build", "ccc").unwrap());

        assert_eq!(store.phase_heads("r").unwrap(), []);
    }

    #[test]
    fn phase_heads_come_in_the_order_the_phases_started_and_by_run() {
        let dir = TempDir::new();
        let store = Store::open(&dir.0).unwrap();
        store.set_phase_start_head("r", "second", "b").unwrap();
        store.set_phase_start_head("r", "first", "a").unwrap();
        store.set_phase_start_head("other", "x", "z").unwrap();

        let phases: Vec<String> = store
            .phase_heads("r")
            .unwrap()
            .into_iter()
            .map(|heads| heads.phase)
            .collect();

        assert_eq!(phases, ["second", "first"]);
    }

    fn ended() -> RunState {
        RunState::Ended {
            outcome: NonEmpty::new("done").unwrap(),
        }
    }

    fn transition(store: &Store, run: &str, to: RunState) {
        let seq = store.last_seq().unwrap() + 1;
        store
            .append_event(&Event {
                seq,
                wall_ms: 100 * seq,
                actor: Actor {
                    kind: ActorKind::Workflow,
                    run: None,
                    phase: None,
                },
                experiment: None,
                body: EventBody::RunTransition {
                    run: run.to_string(),
                    from: None,
                    to,
                },
            })
            .unwrap();
    }

    fn launch(board: Option<&str>, tasks: &[&str]) -> RunLaunch {
        RunLaunch {
            board: board.map(str::to_string),
            tasks: tasks
                .iter()
                .map(|key| RunTaskRef {
                    key: key.to_string(),
                    status: "status-a".to_string(),
                })
                .collect(),
            line: "main".to_string(),
            workflow: "collide/quick".to_string(),
        }
    }

    fn filter(scope: RunScope) -> RunFilter<'static> {
        RunFilter {
            board: None,
            task: None,
            scope,
            before: None,
            limit: 100,
        }
    }

    fn names(rows: &[RunRow]) -> Vec<&str> {
        rows.iter().map(|row| row.run.as_str()).collect()
    }

    #[test]
    fn a_run_is_live_unless_it_ended_failed_or_was_stopped() {
        assert!(is_live(&RunState::Running {}));
        assert!(is_live(&RunState::Integrating {}));
        assert!(is_live(&RunState::Paused {
            cause: PauseCause::UsageLimit
        }));
        assert!(!is_live(&ended()));
        assert!(!is_live(&RunState::Failed {}));
        assert!(!is_live(&RunState::Stopped {}));
    }

    #[test]
    fn linking_a_run_that_has_no_row_writes_nothing_and_says_so() {
        let dir = TempDir::new();
        let store = Store::open(&dir.0).unwrap();
        assert!(
            !store
                .set_run_launch("ghost", &launch(None, &["t:1"]))
                .unwrap()
        );
        assert_eq!(store.run("ghost").unwrap(), None);
        assert!(store.runs(&filter(RunScope::All)).unwrap().is_empty());
    }

    #[test]
    fn linking_again_replaces_the_board_and_the_tasks() {
        let dir = TempDir::new();
        let store = Store::open(&dir.0).unwrap();
        transition(&store, "a", RunState::Running {});
        assert!(
            store
                .set_run_launch("a", &launch(Some("board-1"), &["t:1", "t:2"]))
                .unwrap()
        );
        assert!(
            store
                .set_run_launch("a", &launch(Some("board-2"), &["t:3"]))
                .unwrap()
        );
        let row = store.run("a").unwrap().unwrap();
        assert_eq!(row.board.as_deref(), Some("board-2"));
        let keys: Vec<&str> = row.tasks.iter().map(|t| t.key.as_str()).collect();
        assert_eq!(keys, ["t:3"]);
    }

    #[test]
    fn a_later_transition_keeps_the_link_and_the_first_time_and_sets_the_end() {
        let dir = TempDir::new();
        let store = Store::open(&dir.0).unwrap();
        transition(&store, "a", RunState::Running {});
        store
            .set_run_launch("a", &launch(Some("board-1"), &["t:1"]))
            .unwrap();
        transition(&store, "a", ended());
        let row = store.run("a").unwrap().unwrap();
        assert_eq!((row.started_ms, row.ended_ms), (100, Some(200)));
        assert_eq!(
            (row.board.as_deref(), row.tasks.len()),
            (Some("board-1"), 1)
        );
        assert_eq!(row.state, ended());
    }

    #[test]
    fn a_runs_task_file_is_kept_through_its_end_and_a_run_without_one_has_none() {
        let dir = TempDir::new();
        let store = Store::open(&dir.0).unwrap();
        transition(&store, "a", RunState::Running {});
        transition(&store, "b", RunState::Running {});
        assert!(
            store
                .set_run_task_file("a", "/work/notes/probe.md")
                .unwrap()
        );
        transition(&store, "a", ended());
        let a = store.run("a").unwrap().unwrap();
        assert_eq!(a.task_file.as_deref(), Some("/work/notes/probe.md"));
        assert_eq!(store.run("b").unwrap().unwrap().task_file, None);
    }

    #[test]
    fn a_task_file_for_a_run_that_has_no_row_writes_nothing_and_says_so() {
        let dir = TempDir::new();
        let store = Store::open(&dir.0).unwrap();
        assert!(!store.set_run_task_file("ghost", "/work/probe.md").unwrap());
        assert_eq!(store.run("ghost").unwrap(), None);
    }

    #[test]
    fn a_run_that_resumes_after_it_ended_is_live_again_with_no_end() {
        let dir = TempDir::new();
        let store = Store::open(&dir.0).unwrap();
        transition(&store, "a", RunState::Running {});
        transition(&store, "a", RunState::Stopped {});
        transition(&store, "a", RunState::Running {});
        let row = store.run("a").unwrap().unwrap();
        assert_eq!(row.ended_ms, None);
        assert_eq!(names(&store.runs(&filter(RunScope::Live)).unwrap()), ["a"]);
    }

    #[test]
    fn runs_come_newest_first_and_before_and_limit_cut_a_page() {
        let dir = TempDir::new();
        let store = Store::open(&dir.0).unwrap();
        for run in ["a", "b", "c", "d"] {
            transition(&store, run, RunState::Running {});
        }
        let all = store.runs(&filter(RunScope::All)).unwrap();
        assert_eq!(names(&all), ["d", "c", "b", "a"]);
        let page = store
            .runs(&RunFilter {
                before: Some(all[1].opened_seq),
                limit: 1,
                ..filter(RunScope::All)
            })
            .unwrap();
        assert_eq!(names(&page), ["b"]);
    }

    #[test]
    fn board_task_and_scope_each_narrow_the_runs() {
        let dir = TempDir::new();
        let store = Store::open(&dir.0).unwrap();
        transition(&store, "a", RunState::Running {});
        store
            .set_run_launch("a", &launch(Some("board-1"), &["t:1"]))
            .unwrap();
        transition(&store, "b", RunState::Running {});
        store
            .set_run_launch("b", &launch(Some("board-2"), &["t:1", "t:2"]))
            .unwrap();
        transition(&store, "b", RunState::Failed {});
        let by = |board, task, scope| {
            let rows = store
                .runs(&RunFilter {
                    board,
                    task,
                    ..filter(scope)
                })
                .unwrap();
            rows.into_iter().map(|row| row.run).collect::<Vec<_>>()
        };
        assert_eq!(by(Some("board-1"), None, RunScope::All), ["a"]);
        assert_eq!(by(None, Some("t:1"), RunScope::All), ["b", "a"]);
        assert_eq!(by(None, Some("t:2"), RunScope::All), ["b"]);
        assert_eq!(
            by(Some("board-1"), Some("t:2"), RunScope::All),
            Vec::<String>::new()
        );
        assert_eq!(by(None, None, RunScope::Live), ["a"]);
        assert_eq!(by(None, None, RunScope::Finished), ["b"]);
    }
}
