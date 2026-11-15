//! Tests for the run, phase and workspace state machines (ts-0050-xc): a table test per machine over every ordered
//! pair of states, the terminal states, the two disk cases, `lost`, and `integrating` through the integrate answer.

use collide_core::non_empty::NonEmpty;
use collide_core::run_rules::{IntegrateAnswer, apply_integrate_answer};
use collide_core::state::{FailCause, PauseCause, PhaseState, RunState, WorkspaceState};
use collide_core::transition::{
    PhaseEnding, RunVia, WorkspaceVia, phase_ending, phase_transition, run_transition,
    workspace_transition,
};

fn text(value: &str) -> NonEmpty {
    NonEmpty::new(value).unwrap()
}

// --- Run ---

/// One state of each name, with `paused` and `ended` carrying data.
fn run_states() -> Vec<(&'static str, RunState)> {
    vec![
        ("running", RunState::Running {}),
        (
            "paused",
            RunState::Paused {
                cause: PauseCause::AskHuman,
            },
        ),
        ("integrating", RunState::Integrating {}),
        (
            "ended",
            RunState::Ended {
                outcome: text("done"),
            },
        ),
        ("failed", RunState::Failed {}),
        ("stopped", RunState::Stopped {}),
    ]
}

fn run(name: &str) -> RunState {
    run_states()
        .into_iter()
        .find(|(n, _)| *n == name)
        .unwrap()
        .1
}

/// Every move the engine may make, written out. Nothing else is allowed.
const RUN_ENGINE: [(&str, &str); 7] = [
    ("running", "paused"),
    ("running", "ended"),
    ("running", "failed"),
    ("running", "stopped"),
    ("paused", "running"),
    ("paused", "failed"),
    ("paused", "stopped"),
    // `integrating` is not here: only an integrate answer enters or leaves it.
];

/// Every move an integrate answer may make.
const RUN_INTEGRATE: [(&str, &str); 4] = [
    ("ended", "integrating"),
    ("ended", "ended"),
    ("integrating", "integrating"),
    ("integrating", "ended"),
];

#[test]
fn the_run_table_allows_exactly_the_moves_written_for_the_engine() {
    for (from_name, from) in run_states() {
        for (to_name, to) in run_states() {
            let allowed = RUN_ENGINE.contains(&(from_name, to_name));
            let got = run_transition(&from, &to, RunVia::Engine);
            assert_eq!(got.is_ok(), allowed, "{from_name} -> {to_name}: {got:?}");
        }
    }
}

#[test]
fn the_run_table_allows_exactly_the_moves_written_for_an_integrate_answer() {
    for (from_name, from) in run_states() {
        for (to_name, to) in run_states() {
            let allowed = RUN_INTEGRATE.contains(&(from_name, to_name));
            let got = run_transition(&from, &to, RunVia::IntegrateAnswer);
            assert_eq!(got.is_ok(), allowed, "{from_name} -> {to_name}: {got:?}");
        }
    }
}

#[test]
fn a_run_pauses_with_each_cause_and_resumes_to_running() {
    for cause in [
        PauseCause::AskHuman,
        PauseCause::UsageLimit,
        PauseCause::Disk,
    ] {
        let paused = RunState::Paused { cause };
        assert!(run_transition(&run("running"), &paused, RunVia::Engine).is_ok());
        assert!(run_transition(&paused, &run("running"), RunVia::Engine).is_ok());
    }
}

#[test]
fn a_paused_run_does_not_change_its_cause_without_running_first() {
    let ask = RunState::Paused {
        cause: PauseCause::AskHuman,
    };
    let disk = RunState::Paused {
        cause: PauseCause::Disk,
    };
    let error = run_transition(&ask, &disk, RunVia::Engine).unwrap_err();
    let shown = error.to_string();
    assert!(
        shown.contains("ask_human") && shown.contains("disk"),
        "{shown}"
    );
}

#[test]
fn a_run_that_ended_failed_or_stopped_accepts_no_engine_move() {
    for from_name in ["ended", "failed", "stopped"] {
        for (to_name, to) in run_states() {
            let got = run_transition(&run(from_name), &to, RunVia::Engine);
            assert!(got.is_err(), "{from_name} -> {to_name} was allowed");
        }
    }
}

#[test]
fn a_run_that_failed_or_stopped_accepts_no_integrate_answer() {
    for from_name in ["failed", "stopped"] {
        for (to_name, to) in run_states() {
            let got = run_transition(&run(from_name), &to, RunVia::IntegrateAnswer);
            assert!(got.is_err(), "{from_name} -> {to_name} was allowed");
        }
    }
}

#[test]
fn a_refused_run_move_names_both_states() {
    let error = run_transition(&run("stopped"), &run("running"), RunVia::Engine).unwrap_err();
    let shown = error.to_string();
    assert!(shown.contains("run"), "{shown}");
    assert!(
        shown.contains("stopped") && shown.contains("running"),
        "{shown}"
    );
    assert_eq!(error.from, "stopped");
    assert_eq!(error.to, "running");
}

#[test]
fn a_refused_move_into_ended_names_the_outcome() {
    let error = run_transition(&run("failed"), &run("ended"), RunVia::Engine).unwrap_err();
    assert_eq!(error.to, "ended (done)");
}

#[test]
fn the_engine_cannot_enter_or_leave_integrating() {
    let into = run_transition(&run("ended"), &run("integrating"), RunVia::Engine).unwrap_err();
    let out_of = run_transition(&run("integrating"), &run("ended"), RunVia::Engine).unwrap_err();
    for error in [into, out_of] {
        let shown = error.to_string();
        assert!(shown.contains("integrate"), "{shown}");
        assert!(shown.contains("integrating"), "{shown}");
    }
}

// --- integrating, through the integrate answer ---

fn merged() -> IntegrateAnswer {
    IntegrateAnswer::Merged {
        commit: text("abc123"),
    }
}

fn pending() -> IntegrateAnswer {
    IntegrateAnswer::Pending {
        url: text("https://example.test/pr/7"),
        head: text("b2c3d4"),
    }
}

#[test]
fn a_pending_answer_moves_an_ended_run_into_integrating() {
    let step = apply_integrate_answer(&run("ended"), &pending(), &text("done")).unwrap();
    assert_eq!(step.run, RunState::Integrating {});
}

#[test]
fn a_merged_answer_moves_an_integrating_run_back_to_ended_with_its_outcome() {
    let step = apply_integrate_answer(&run("integrating"), &merged(), &text("done")).unwrap();
    assert_eq!(
        step.run,
        RunState::Ended {
            outcome: text("done")
        }
    );
}

#[test]
fn a_pending_answer_again_keeps_the_run_integrating() {
    let step = apply_integrate_answer(&run("integrating"), &pending(), &text("done")).unwrap();
    assert_eq!(step.run, RunState::Integrating {});
}

#[test]
fn a_merged_answer_for_an_ended_run_leaves_it_ended() {
    let step = apply_integrate_answer(&run("ended"), &merged(), &text("done")).unwrap();
    assert_eq!(step.run, run("ended"));
}

#[test]
fn an_integrate_answer_for_a_run_the_machine_refuses_names_both_states() {
    for from_name in ["running", "paused", "failed", "stopped"] {
        for answer in [merged(), pending()] {
            let error = apply_integrate_answer(&run(from_name), &answer, &text("done"))
                .unwrap_err()
                .to_string();
            assert!(error.contains(from_name), "{error}");
            assert!(error.contains("integrat"), "{error}");
        }
    }
}

// --- Phase ---

fn phase_states() -> Vec<(&'static str, PhaseState)> {
    vec![
        ("pending", PhaseState::Pending {}),
        ("running", PhaseState::Running {}),
        (
            "paused",
            PhaseState::Paused {
                cause: PauseCause::AskHuman,
            },
        ),
        ("ended", PhaseState::Ended {}),
        ("failed", PhaseState::Failed { cause: None }),
        ("stopped", PhaseState::Stopped {}),
        ("lost", PhaseState::Lost {}),
    ]
}

fn phase(name: &str) -> PhaseState {
    phase_states()
        .into_iter()
        .find(|(n, _)| *n == name)
        .unwrap()
        .1
}

const PHASE_MOVES: [(&str, &str); 12] = [
    ("pending", "running"),
    ("pending", "failed"),
    ("pending", "stopped"),
    ("running", "paused"),
    ("running", "ended"),
    ("running", "failed"),
    ("running", "stopped"),
    ("running", "lost"),
    ("paused", "running"),
    ("paused", "failed"),
    ("paused", "stopped"),
    ("paused", "lost"),
];

#[test]
fn the_phase_table_allows_exactly_the_moves_written() {
    for (from_name, from) in phase_states() {
        for (to_name, to) in phase_states() {
            let allowed = PHASE_MOVES.contains(&(from_name, to_name));
            let got = phase_transition(&from, &to);
            assert_eq!(got.is_ok(), allowed, "{from_name} -> {to_name}: {got:?}");
        }
    }
}

#[test]
fn a_phase_that_ended_failed_stopped_or_was_lost_accepts_nothing() {
    for from_name in ["ended", "failed", "stopped", "lost"] {
        for (to_name, to) in phase_states() {
            let got = phase_transition(&phase(from_name), &to);
            assert!(got.is_err(), "{from_name} -> {to_name} was allowed");
        }
    }
}

#[test]
fn a_failed_phase_with_a_cause_is_as_final_as_one_without() {
    let failed = PhaseState::Failed {
        cause: Some(FailCause::Disk),
    };
    assert!(phase_transition(&failed, &phase("running")).is_err());
    assert!(phase_transition(&phase("running"), &failed).is_ok());
    assert!(phase_transition(&phase("pending"), &failed).is_ok());
    assert!(phase_transition(&phase("paused"), &failed).is_ok());
}

#[test]
fn a_refused_phase_move_names_both_states() {
    let error = phase_transition(&phase("lost"), &phase("running")).unwrap_err();
    let shown = error.to_string();
    assert!(shown.contains("phase"), "{shown}");
    assert!(
        shown.contains("lost") && shown.contains("running"),
        "{shown}"
    );
}

#[test]
fn a_refused_move_into_a_failed_phase_names_its_cause() {
    let failed = PhaseState::Failed {
        cause: Some(FailCause::Disk),
    };
    let error = phase_transition(&phase("ended"), &failed).unwrap_err();
    assert_eq!(error.to, "failed (disk)");
}

#[test]
fn a_phase_whose_shim_died_is_lost_from_running_or_paused_and_from_nowhere_else() {
    for from_name in ["running", "paused"] {
        assert!(phase_transition(&phase(from_name), &phase("lost")).is_ok());
    }
    for from_name in ["pending", "ended", "failed", "stopped", "lost"] {
        assert!(phase_transition(&phase(from_name), &phase("lost")).is_err());
    }
}

// --- Disk ---

#[test]
fn a_phase_pauses_for_disk_from_running_and_returns_to_running() {
    let paused = PhaseState::Paused {
        cause: PauseCause::Disk,
    };
    assert!(phase_transition(&phase("running"), &paused).is_ok());
    assert!(phase_transition(&paused, &phase("running")).is_ok());
}

fn ending(exit_recorded: bool, result_seen: bool, volume_full: bool) -> PhaseState {
    phase_ending(&PhaseEnding {
        exit_recorded,
        result_seen,
        volume_full,
    })
}

#[test]
fn a_phase_that_ends_with_no_result_on_a_full_volume_fails_with_cause_disk() {
    let disk = PhaseState::Failed {
        cause: Some(FailCause::Disk),
    };
    assert_eq!(ending(true, false, true), disk);
    assert_eq!(
        ending(false, false, true),
        disk,
        "not lost, though no exit.json could be written"
    );
}

#[test]
fn a_shim_that_died_without_an_exit_record_is_lost_whatever_the_stream_reported() {
    assert_eq!(ending(false, false, false), phase("lost"));
    assert_eq!(ending(false, true, false), phase("lost"));
}

#[test]
fn a_full_volume_does_not_hide_a_lost_phase_that_did_report_a_result() {
    assert_eq!(ending(false, true, true), phase("lost"));
}

#[test]
fn a_phase_with_an_exit_record_and_a_result_ended() {
    assert_eq!(ending(true, true, false), phase("ended"));
    assert_eq!(ending(true, true, true), phase("ended"));
}

#[test]
fn a_phase_with_an_exit_record_and_no_result_on_a_free_volume_failed_with_no_known_cause() {
    assert_eq!(ending(true, false, false), phase("failed"));
}

#[test]
fn every_ending_is_reachable_from_a_running_phase() {
    for exit_recorded in [true, false] {
        for result_seen in [true, false] {
            for volume_full in [true, false] {
                let to = ending(exit_recorded, result_seen, volume_full);
                let got = phase_transition(&phase("running"), &to);
                assert!(
                    got.is_ok(),
                    "{exit_recorded} {result_seen} {volume_full}: {got:?}"
                );
            }
        }
    }
}

// --- Workspace ---

const WORKSPACE_STATES: [WorkspaceState; 7] = [
    WorkspaceState::Provisioning,
    WorkspaceState::Ready,
    WorkspaceState::InUse,
    WorkspaceState::Idle,
    WorkspaceState::Removable,
    WorkspaceState::Removed,
    WorkspaceState::Failed,
];

use WorkspaceState::{Failed, Idle, InUse, Provisioning, Ready, Removable, Removed};

const WORKSPACE_ENGINE: [(WorkspaceState, WorkspaceState); 12] = [
    (Provisioning, Ready),
    (Provisioning, Failed),
    (Ready, InUse),
    (Ready, Removable),
    (Ready, Failed),
    (InUse, Idle),
    (InUse, Failed),
    (Idle, InUse),
    (Idle, Removable),
    (Idle, Failed),
    (Removable, Removed),
    (Removable, Failed),
];

const WORKSPACE_RESUME: [(WorkspaceState, WorkspaceState); 1] = [(Failed, Ready)];

#[test]
fn the_workspace_table_allows_exactly_the_moves_written_for_the_engine() {
    for from in WORKSPACE_STATES {
        for to in WORKSPACE_STATES {
            let allowed = WORKSPACE_ENGINE.contains(&(from, to));
            let got = workspace_transition(from, to, WorkspaceVia::Engine);
            assert_eq!(got.is_ok(), allowed, "{from:?} -> {to:?}: {got:?}");
        }
    }
}

#[test]
fn the_workspace_table_allows_exactly_the_moves_written_for_the_resume_hook() {
    for from in WORKSPACE_STATES {
        for to in WORKSPACE_STATES {
            let allowed = WORKSPACE_RESUME.contains(&(from, to));
            let got = workspace_transition(from, to, WorkspaceVia::ResumeHook);
            assert_eq!(got.is_ok(), allowed, "{from:?} -> {to:?}: {got:?}");
        }
    }
}

#[test]
fn a_failed_workspace_leaves_only_through_the_resume_hook() {
    for to in WORKSPACE_STATES {
        assert!(
            workspace_transition(Failed, to, WorkspaceVia::Engine).is_err(),
            "{to:?}"
        );
    }
    assert!(workspace_transition(Failed, Ready, WorkspaceVia::ResumeHook).is_ok());
}

#[test]
fn a_removed_workspace_accepts_nothing() {
    for via in [WorkspaceVia::Engine, WorkspaceVia::ResumeHook] {
        for to in WORKSPACE_STATES {
            assert!(workspace_transition(Removed, to, via).is_err(), "{to:?}");
        }
    }
}

#[test]
fn a_workspace_goes_down_its_lifecycle_one_state_at_a_time() {
    let chain = [Provisioning, Ready, InUse, Idle, Removable, Removed];
    for pair in chain.windows(2) {
        assert!(workspace_transition(pair[0], pair[1], WorkspaceVia::Engine).is_ok());
    }
    assert!(workspace_transition(Provisioning, InUse, WorkspaceVia::Engine).is_err());
    assert!(workspace_transition(Ready, Removed, WorkspaceVia::Engine).is_err());
}

#[test]
fn a_workspace_that_was_made_and_never_used_can_be_marked_removable() {
    assert!(workspace_transition(Ready, Removable, WorkspaceVia::Engine).is_ok());
}

#[test]
fn a_refused_workspace_move_names_both_states() {
    let error = workspace_transition(Removed, InUse, WorkspaceVia::Engine).unwrap_err();
    let shown = error.to_string();
    assert!(shown.contains("workspace"), "{shown}");
    assert!(
        shown.contains("removed") && shown.contains("in_use"),
        "{shown}"
    );
}
