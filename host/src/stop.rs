//! Stopping a tree of processes (0.5, Stopping; Stage 3a). A stop is not a signal to one pid: hooks run in their own
//! process groups, and `iter` runs each step in its own again, so the stop finds everything first and escalates in a
//! fixed order. `collide stop` and the daemon's stop of a run both come here; each hook runs under a shim, so the same
//! rules cover it.
//!
//! **Finding the tree** combines the ppid walk, the marker scan (the processes that carry the run's marker in their
//! environment, read through libproc by `p_uniqueid`) and their descendants. A process that double-forked still carries
//! the marker.
//!
//! **The order** is [`ORDER`] (measured S3); an agent first gets `interrupt`, then its stdin closes (0.3):
//!
//! 1. SIGTERM to everything found.
//! 2. The grace period, [`DEFAULT_GRACE`] unless the hook sets one (`iter` escalates after 30 s).
//! 3. A rescan, then SIGKILL to everything found.
//! 4. The workspace implementation's `cleanup` hook.
//! 5. A final rescan and the survivor report.
//!
//! The shim checks that its child's process group still exists (`waitid` with `WNOWAIT`) before it signals the group,
//! so a reused group id is never hit.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `stop: find tree` | [`find_tree`] | the ppid walk, the marker scan and their descendants |
//! | `stop: escalation` | [`stop_tree`] | the steps of [`ORDER`] and the [`StopReport`] |

use std::io;
use std::time::Duration;

/// The time a stopped hook gets between SIGTERM and SIGKILL when its entry names none (0.5).
pub const DEFAULT_GRACE: Duration = Duration::from_secs(60);

/// One step of a stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopStep {
    /// SIGTERM to everything found.
    Terminate,
    /// Wait for the processes to go, up to the grace period.
    Grace,
    /// A rescan, then SIGKILL to everything found.
    Kill,
    /// The workspace implementation's `cleanup` hook.
    Cleanup,
    /// A final rescan and the survivor report.
    Report,
}

/// The steps in the order they run.
pub const ORDER: [StopStep; 5] = [
    StopStep::Terminate,
    StopStep::Grace,
    StopStep::Kill,
    StopStep::Cleanup,
    StopStep::Report,
];

/// What a stop is asked to stop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StopRequest {
    /// The root of the tree: a shim or the hook's own process.
    pub root_pid: u32,
    /// The run's marker, the value the processes of the run carry in their environment.
    pub marker: String,
    pub grace: Duration,
}

/// A process still alive after the final rescan. A double-forked orphan that its hook recorded is stopped by the
/// cleanup hook; one it did not record is listed here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Survivor {
    pub pid: u32,
    pub command: String,
}

/// What a stop did.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StopReport {
    /// The pids found by the first scan and signalled.
    pub signalled: Vec<u32>,
    pub survivors: Vec<Survivor>,
}

/// Every process of the tree below `root_pid` and every process carrying `marker`, with their descendants, as pids.
pub fn find_tree(root_pid: u32, marker: &str) -> io::Result<Vec<u32>> {
    let _ = (root_pid, marker);
    todo!("stop: find tree")
}

/// Stops the tree in the order of [`ORDER`]. An `Err` is a failure to scan or signal; processes that would not die are
/// not an error but the report's survivors.
pub fn stop_tree(request: &StopRequest) -> io::Result<StopReport> {
    let _ = request;
    todo!("stop: escalation")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_ends_the_hook_before_its_cleanup_and_reports_last() {
        assert_eq!(
            ORDER,
            [
                StopStep::Terminate,
                StopStep::Grace,
                StopStep::Kill,
                StopStep::Cleanup,
                StopStep::Report
            ]
        );
    }

    #[test]
    fn the_default_grace_is_sixty_seconds() {
        assert_eq!(DEFAULT_GRACE, Duration::from_secs(60));
    }

    #[test]
    #[should_panic(expected = "stop: find tree")]
    fn finding_the_tree_is_a_stub_tagged_for_stage_3a() {
        let _ = find_tree(1, "marker");
    }

    #[test]
    #[should_panic(expected = "stop: escalation")]
    fn stopping_the_tree_is_a_stub_tagged_for_stage_3a() {
        let _ = stop_tree(&StopRequest {
            root_pid: 1,
            marker: "marker".into(),
            grace: DEFAULT_GRACE,
        });
    }
}
