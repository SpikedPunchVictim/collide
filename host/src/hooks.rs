//! The hook executor (0.5, Hook protocol; Stage 7): the real [`HookRunner`] of `core`. One hook is one command per call:
//!
//! - **argv only, never a shell string.** No templating into argv. Task text reaches a hook only as JSON on stdin,
//!   with the pack's settings as `config` (secrets included): settings never travel in the environment, because
//!   `ps -E` reads a same-user process's environment.
//! - **The environment** is `COLLIDE_*` plus the role, phase and experiment `env:` templates, applied in that order,
//!   after the variables the agent adapter strips ([`crate::adapters`]).
//! - **The result** is the file named by [`RESULT_VAR`], and the status in it is the verdict. A non-zero exit, or a
//!   missing or invalid file, is `error`. [`read_result`] reads the file and gives it to [`effective_result`], which
//!   applies the exit code and the rules for `pending` and `unverified`, so the engine never sees a raw exit.
//! - **cwd** is the line root for `workspace.*`, `record`, `integrate` and provider writes, and the workspace for
//!   phase hooks and gates.
//! - **Logs:** stdout and stderr go to files, streamed to the UI.
//! - **Stopping** is [`crate::stop`]; each hook runs under a shim so the stop and survivor rules cover it.
//!
//! The line lock, hook state and the report files are later work in the same stage and are not outlined here.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `hooks: invocation` | [`hook_command`], [`HookExecutor::run`] | stdin JSON, env, cwd and timeout; spawn under a shim; wait |
//! | `hooks: result file` | [`read_result`] | the capped read of the result file and the effective verdict |

use crate::stop::DEFAULT_GRACE;
use collide_core::hook::{HookKind, HookResult, effective_result};
use collide_core::ports::{HookRequest, HookRunError, HookRunner};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// The variable that names the result file. Its name is the plan's own.
pub const RESULT_VAR: &str = "COLLIDE_RESULT";

/// The variable that names a pack's data directory (0.7).
pub const PACK_DATA_VAR: &str = "COLLIDE_PACK_DATA";

/// The default `timeout:` of a point binding for a task point (0.5).
pub const TASK_POINT_TIMEOUT: Duration = Duration::from_secs(30);

/// The default `timeout:` of a point binding for a move point, which may scan files (0.5).
pub const MOVE_POINT_TIMEOUT: Duration = Duration::from_secs(120);

/// What the executor starts: everything a hook sees, decided before anything runs, so it can be checked without a
/// process.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookCommand {
    /// The script and its arguments, never a shell string.
    pub argv: Vec<String>,
    pub cwd: PathBuf,
    /// The environment to add to the base environment, after the adapter's strip.
    pub env: BTreeMap<String, String>,
    /// What the hook reads on stdin: the request's input as JSON.
    pub stdin_json: String,
    pub timeout: Duration,
    /// The time between SIGTERM and SIGKILL if the hook is stopped ([`DEFAULT_GRACE`] when the entry names none).
    pub grace: Duration,
    /// Where the hook's stdout and stderr go.
    pub stdout_log: PathBuf,
    pub stderr_log: PathBuf,
}

/// The executor. `run_dir` is the invocation's own directory: the result file and the logs are made in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HookExecutor {
    pub run_dir: PathBuf,
}

impl HookExecutor {
    pub fn new(run_dir: impl Into<PathBuf>) -> Self {
        Self {
            run_dir: run_dir.into(),
        }
    }

    /// The grace a stopped hook gets between SIGTERM and SIGKILL when its entry names none.
    pub fn default_grace(&self) -> Duration {
        DEFAULT_GRACE
    }
}

/// The command for one request: argv from the entry's script, the JSON the hook reads, the environment and the two
/// logs in `run_dir`.
pub fn hook_command(request: &HookRequest, run_dir: &Path) -> HookCommand {
    let _ = (request, run_dir);
    todo!("hooks: invocation")
}

/// Reads the result file at `result_path` (never past [`collide_core::hook::OUTPUTS_MAX_BYTES`] of outputs) and gives
/// it to [`effective_result`] with the hook's kind and whether it exited zero. The status in the file is the verdict;
/// a child tool's exit code is never forwarded as one.
pub fn read_result(kind: HookKind, exit_zero: bool, result_path: &Path) -> HookResult {
    let _ = (kind, exit_zero, result_path, effective_result);
    todo!("hooks: result file")
}

impl HookRunner for HookExecutor {
    fn run(&self, request: &HookRequest) -> Result<HookResult, HookRunError> {
        let _ = (request, &self.run_dir);
        todo!("hooks: invocation")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hook_gets_thirty_seconds_at_a_task_point_and_two_minutes_at_a_move_point() {
        assert_eq!(TASK_POINT_TIMEOUT, Duration::from_secs(30));
        assert_eq!(MOVE_POINT_TIMEOUT, Duration::from_secs(120));
    }

    #[test]
    fn a_stopped_hook_gets_sixty_seconds_unless_its_entry_says_otherwise() {
        assert_eq!(
            HookExecutor::new("/x").default_grace(),
            Duration::from_secs(60)
        );
    }

    #[test]
    #[should_panic(expected = "hooks: result file")]
    fn reading_the_result_file_is_a_stub_tagged_for_stage_7() {
        let _ = read_result(HookKind::Gate, true, Path::new("/x/result.json"));
    }
}
