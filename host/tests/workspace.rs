//! Tests for the built-in git worktree workspace and the disk floor (ts-0055-cr; 0.2, Durability; 0.4; 0.8, Workspace;
//! 0.13). Every repository here is made in a scratch folder with `git init`, with the global and system git
//! configuration switched off, so no test reads or changes the user's own.

use collide_core::hook::WorkspaceKind;
use collide_core::owner::RemoveRefusal;
use collide_core::state::{FailCause, PauseCause, PhaseState, RunState, WorkspaceState};
use collide_core::transition::{PhaseEnding, RunVia, phase_ending, run_transition};
use collide_host::disk::{
    Before, DEFAULT_FLOOR_BYTES, DiskError, DiskGuard, Floor, FreeSpace, SystemFreeSpace,
};
use collide_host::workspace::{BuiltinWorkspace, WorkspaceError, WorkspaceSpec, disk_use};
use std::cell::Cell;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

mod common;
use common::scratch;

const FLOOR: Floor = Floor {
    min_free_bytes: 1_000,
};

/// A free-space reader a test sets. `None` makes the read fail.
struct FakeFree(Cell<Option<u64>>);

impl FakeFree {
    fn new(bytes: u64) -> FakeFree {
        FakeFree(Cell::new(Some(bytes)))
    }
}

impl FreeSpace for FakeFree {
    fn free_bytes(&self, _path: &Path) -> io::Result<u64> {
        self.0
            .get()
            .ok_or_else(|| io::Error::other("the volume cannot be read"))
    }
}

fn isolated(cmd: &mut Command) -> &mut Command {
    cmd.env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("LC_ALL", "C")
}

fn git(dir: &Path, args: &[&str]) -> String {
    let mut cmd = Command::new("git");
    cmd.args([
        "-c",
        "user.name=test",
        "-c",
        "user.email=test@example.invalid",
        "-c",
        "commit.gpgsign=false",
    ])
    .args(args)
    .current_dir(dir);
    let output = isolated(&mut cmd).output().expect("git should run");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

struct Fixture {
    root: PathBuf,
    repo: PathBuf,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

impl Fixture {
    /// A repository on `main` with one commit that holds `README` and a `.gitignore` for `*.log`.
    fn new(name: &str) -> Fixture {
        let root = scratch(&format!("workspace-{name}"));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        // The temp folder may sit behind a symbolic link (macOS /var); git reports the real path.
        let root = fs::canonicalize(root).unwrap();
        let repo = root.join("repo");
        fs::create_dir_all(&repo).unwrap();
        git(&repo, &["init", "-q", "-b", "main"]);
        fs::write(repo.join("README"), "hello\n").unwrap();
        fs::write(repo.join(".gitignore"), "*.log\n").unwrap();
        git(&repo, &["add", "."]);
        git(&repo, &["commit", "-q", "-m", "start"]);
        Fixture { root, repo }
    }

    fn spec(&self) -> WorkspaceSpec {
        WorkspaceSpec {
            kind: WorkspaceKind::Task,
            repo: self.repo.clone(),
            path: self.root.join("trees/repo/ts-0001"),
            branch: "collide/ts-0001".to_string(),
            base: "main".to_string(),
        }
    }

    fn workspace(&self) -> BuiltinWorkspace {
        BuiltinWorkspace::new(self.spec()).with_git_env(vec![
            ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
            ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
            ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
        ])
    }

    /// A workspace made and idle, as a run leaves it.
    fn idle(&self) -> BuiltinWorkspace {
        let free = FakeFree::new(u64::MAX);
        let guard = DiskGuard::new(&free, FLOOR);
        let mut ws = self.workspace();
        ws.provision(&guard).unwrap();
        ws.start_phase(&guard).unwrap();
        ws.end_phase().unwrap();
        ws
    }

    fn tree(&self) -> PathBuf {
        self.root.join("trees/repo/ts-0001")
    }

    fn worktrees(&self) -> String {
        git(&self.repo, &["worktree", "list", "--porcelain"])
    }
}

fn expect_refused(result: Result<(), WorkspaceError>, wanted: RemoveRefusal) -> String {
    match result {
        Err(WorkspaceError::Refused { reason, message }) => {
            assert_eq!(reason, wanted, "{message}");
            message
        }
        other => panic!("expected a refusal, got {other:?}"),
    }
}

// --- Create and remove ---

#[test]
fn a_workspace_is_made_on_a_new_branch_and_returns_its_path_and_git_dir() {
    let fx = Fixture::new("create");
    let free = FakeFree::new(u64::MAX);
    let guard = DiskGuard::new(&free, FLOOR);
    let mut ws = fx.workspace();
    let made = ws.provision(&guard).unwrap();

    assert_eq!(made.kind, WorkspaceKind::Task);
    assert_eq!(PathBuf::from(&made.path), fx.tree());
    let git_dir = PathBuf::from(&made.git_dir);
    assert!(git_dir.is_dir(), "{git_dir:?} is not a folder");
    assert!(
        git_dir.starts_with(fx.repo.join(".git/worktrees")),
        "{git_dir:?}"
    );
    assert_eq!(
        git(&fx.tree(), &["rev-parse", "--abbrev-ref", "HEAD"]),
        "collide/ts-0001"
    );
    assert_eq!(ws.location(), Some(made));
}

#[test]
fn a_workspace_moves_through_the_states_of_core() {
    let fx = Fixture::new("states");
    let free = FakeFree::new(u64::MAX);
    let guard = DiskGuard::new(&free, FLOOR);
    let mut ws = fx.workspace();
    assert_eq!(ws.state(), WorkspaceState::Provisioning);
    ws.provision(&guard).unwrap();
    assert_eq!(ws.state(), WorkspaceState::Ready);
    ws.start_phase(&guard).unwrap();
    assert_eq!(ws.state(), WorkspaceState::InUse);
    ws.end_phase().unwrap();
    assert_eq!(ws.state(), WorkspaceState::Idle);
    ws.start_phase(&guard).unwrap();
    assert_eq!(
        ws.state(),
        WorkspaceState::InUse,
        "an idle workspace is used again"
    );
    ws.end_phase().unwrap();
    ws.remove().unwrap();
    assert_eq!(ws.state(), WorkspaceState::Removed);
}

#[test]
fn a_move_core_does_not_allow_is_refused_and_names_both_states() {
    let fx = Fixture::new("badmove");
    let free = FakeFree::new(u64::MAX);
    let guard = DiskGuard::new(&free, FLOOR);
    let mut ws = fx.workspace();
    // provisioning -> in_use is not a move.
    match ws.start_phase(&guard) {
        Err(WorkspaceError::Transition(error)) => {
            assert_eq!(error.from, "provisioning");
            assert_eq!(error.to, "in_use");
        }
        other => panic!("expected a transition error, got {other:?}"),
    }
    assert_eq!(ws.state(), WorkspaceState::Provisioning);
    // ready cannot be provisioned again, and the second `git worktree add` does not run.
    ws.provision(&guard).unwrap();
    assert!(matches!(
        ws.provision(&guard),
        Err(WorkspaceError::Transition(_))
    ));
    assert_eq!(ws.state(), WorkspaceState::Ready);
}

#[test]
fn a_clean_merged_workspace_is_removed_with_its_folder_and_branch() {
    let fx = Fixture::new("remove");
    let mut ws = fx.idle();
    ws.remove().unwrap();
    assert!(!fx.tree().exists());
    assert!(!fx.worktrees().contains("ts-0001"));
    assert_eq!(git(&fx.repo, &["branch", "--list", "collide/ts-0001"]), "");
}

#[test]
fn removal_refuses_a_change_that_is_not_committed_and_says_so() {
    let fx = Fixture::new("dirty");
    let mut ws = fx.idle();
    fs::write(fx.tree().join("README"), "changed\n").unwrap();
    let message = expect_refused(ws.remove(), RemoveRefusal::Dirty);
    assert!(message.contains("uncommitted"), "{message}");
    assert!(fx.tree().join("README").exists());
    assert_eq!(ws.state(), WorkspaceState::Idle);
}

#[test]
fn removal_refuses_an_untracked_file_and_says_so() {
    let fx = Fixture::new("untracked");
    let mut ws = fx.idle();
    fs::write(fx.tree().join("notes.txt"), "mine\n").unwrap();
    let message = expect_refused(ws.remove(), RemoveRefusal::Dirty);
    assert!(message.contains("untracked"), "{message}");
    assert!(fx.tree().join("notes.txt").exists());
}

#[test]
fn removal_does_not_count_an_ignored_file() {
    let fx = Fixture::new("ignored");
    let mut ws = fx.idle();
    fs::write(fx.tree().join("build.log"), "output\n").unwrap();
    ws.remove().unwrap();
    assert!(!fx.tree().exists());
}

#[test]
fn removal_refuses_a_commit_not_merged_into_the_base_and_says_so() {
    let fx = Fixture::new("unmerged");
    let mut ws = fx.idle();
    fs::write(fx.tree().join("work.txt"), "done\n").unwrap();
    git(&fx.tree(), &["add", "work.txt"]);
    git(&fx.tree(), &["commit", "-q", "-m", "work"]);
    let message = expect_refused(ws.remove(), RemoveRefusal::Unmerged);
    assert!(message.contains("not merged into main"), "{message}");
    assert!(fx.tree().exists());
    assert_eq!(ws.state(), WorkspaceState::Idle);
}

#[test]
fn removal_goes_ahead_once_the_commit_is_merged_into_the_base() {
    let fx = Fixture::new("merged");
    let mut ws = fx.idle();
    fs::write(fx.tree().join("work.txt"), "done\n").unwrap();
    git(&fx.tree(), &["add", "work.txt"]);
    git(&fx.tree(), &["commit", "-q", "-m", "work"]);
    git(&fx.repo, &["merge", "-q", "--ff-only", "collide/ts-0001"]);
    ws.remove().unwrap();
    assert!(!fx.tree().exists());
}

#[test]
fn removal_says_dirty_before_unmerged_when_both_are_true() {
    let fx = Fixture::new("both");
    let mut ws = fx.idle();
    fs::write(fx.tree().join("work.txt"), "done\n").unwrap();
    git(&fx.tree(), &["add", "work.txt"]);
    git(&fx.tree(), &["commit", "-q", "-m", "work"]);
    fs::write(fx.tree().join("README"), "changed\n").unwrap();
    expect_refused(ws.remove(), RemoveRefusal::Dirty);
}

#[test]
fn a_workspace_a_run_is_using_is_not_removed() {
    let fx = Fixture::new("in-use");
    let free = FakeFree::new(u64::MAX);
    let guard = DiskGuard::new(&free, FLOOR);
    let mut ws = fx.workspace();
    ws.provision(&guard).unwrap();
    ws.start_phase(&guard).unwrap();
    expect_refused(ws.remove(), RemoveRefusal::InUse);
    assert!(fx.tree().exists());
    assert_eq!(ws.state(), WorkspaceState::InUse);
}

#[test]
fn a_workspace_that_was_never_used_is_not_removable_by_core_s_table() {
    let fx = Fixture::new("ready-remove");
    let free = FakeFree::new(u64::MAX);
    let guard = DiskGuard::new(&free, FLOOR);
    let mut ws = fx.workspace();
    ws.provision(&guard).unwrap();
    assert!(matches!(ws.remove(), Err(WorkspaceError::Transition(_))));
    assert!(fx.tree().exists());
}

#[test]
fn a_branch_or_base_that_could_pass_for_an_option_is_refused_before_git_runs() {
    let fx = Fixture::new("option");
    let free = FakeFree::new(u64::MAX);
    let guard = DiskGuard::new(&free, FLOOR);
    let mut spec = fx.spec();
    spec.base = "--detach".to_string();
    let mut ws = BuiltinWorkspace::new(spec);
    assert!(matches!(
        ws.provision(&guard),
        Err(WorkspaceError::BadSpec { .. })
    ));
    assert_eq!(ws.state(), WorkspaceState::Provisioning);
    assert!(!fx.tree().exists());
}

#[test]
fn a_base_that_does_not_exist_fails_the_workspace_with_git_s_words() {
    let fx = Fixture::new("no-base");
    let free = FakeFree::new(u64::MAX);
    let guard = DiskGuard::new(&free, FLOOR);
    let mut spec = fx.spec();
    spec.base = "nowhere".to_string();
    let mut ws = BuiltinWorkspace::new(spec).with_git_env(vec![
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
    ]);
    match ws.provision(&guard) {
        Err(WorkspaceError::Git { args, stderr }) => {
            assert!(args.contains("worktree add"), "{args}");
            assert!(stderr.contains("nowhere"), "{stderr}");
        }
        other => panic!("expected a git error, got {other:?}"),
    }
    assert_eq!(ws.state(), WorkspaceState::Failed);
}

// --- The disk floor ---

#[test]
fn under_the_floor_no_workspace_is_made_and_the_run_pauses_for_disk() {
    let fx = Fixture::new("floor-create");
    let free = FakeFree::new(FLOOR.min_free_bytes - 1);
    let guard = DiskGuard::new(&free, FLOOR);
    let mut ws = fx.workspace();
    let pause = match ws.provision(&guard) {
        Err(WorkspaceError::Disk(DiskError::Paused(pause))) => pause,
        other => panic!("expected a disk pause, got {other:?}"),
    };
    assert!(!fx.tree().exists(), "the folder was made");
    assert!(!fx.worktrees().contains("ts-0001"), "git made a worktree");
    assert_eq!(ws.state(), WorkspaceState::Provisioning);
    assert_eq!(pause.before, Before::Workspace);
    assert_eq!(pause.free_bytes, FLOOR.min_free_bytes - 1);
    assert_eq!(pause.floor_bytes, FLOOR.min_free_bytes);

    let paused = pause.run_state();
    assert_eq!(
        paused,
        RunState::Paused {
            cause: PauseCause::Disk
        }
    );
    assert_eq!(pause.cause(), PauseCause::Disk);
    run_transition(&RunState::Running {}, &paused, RunVia::Engine).unwrap();

    // Space comes back: the same workspace is made.
    free.0.set(Some(FLOOR.min_free_bytes));
    ws.provision(&guard).unwrap();
    assert_eq!(ws.state(), WorkspaceState::Ready);
}

#[test]
fn under_the_floor_a_phase_does_not_start_and_the_workspace_stays_as_it_was() {
    let fx = Fixture::new("floor-phase");
    let free = FakeFree::new(u64::MAX);
    let guard = DiskGuard::new(&free, FLOOR);
    let mut ws = fx.workspace();
    ws.provision(&guard).unwrap();
    free.0.set(Some(0));
    let pause = match ws.start_phase(&guard) {
        Err(WorkspaceError::Disk(DiskError::Paused(pause))) => pause,
        other => panic!("expected a disk pause, got {other:?}"),
    };
    assert_eq!(pause.before, Before::Phase);
    assert_eq!(ws.state(), WorkspaceState::Ready);
    // A phase that has not started is still `pending`: only the run pauses.
    assert_eq!(
        pause.run_state(),
        RunState::Paused {
            cause: PauseCause::Disk
        }
    );
}

#[test]
fn the_floor_is_a_strict_minimum() {
    let free = FakeFree::new(FLOOR.min_free_bytes);
    let guard = DiskGuard::new(&free, FLOOR);
    let path = Path::new("/anywhere");
    assert!(guard.check(path, Before::Phase).is_ok());
    free.0.set(Some(FLOOR.min_free_bytes - 1));
    assert!(matches!(
        guard.check(path, Before::Phase),
        Err(DiskError::Paused(_))
    ));
}

#[test]
fn a_volume_that_cannot_be_read_is_an_error_and_not_a_pause() {
    let free = FakeFree(Cell::new(None));
    let guard = DiskGuard::new(&free, FLOOR);
    match guard.check(Path::new("/anywhere"), Before::Workspace) {
        Err(DiskError::Unreadable { path, source }) => {
            assert_eq!(path, Path::new("/anywhere"));
            assert!(source.to_string().contains("cannot be read"));
        }
        other => panic!("expected an unreadable volume, got {other:?}"),
    }
}

#[test]
fn the_queue_item_says_what_was_wanted_how_much_is_free_and_what_to_do() {
    let free = FakeFree::new(3 * 1024 * 1024);
    let floor = Floor {
        min_free_bytes: DEFAULT_FLOOR_BYTES,
    };
    let guard = DiskGuard::new(&free, floor);
    let Err(DiskError::Paused(pause)) = guard.check(Path::new("/volume/tree"), Before::Workspace)
    else {
        panic!("expected a pause");
    };
    let item = pause.queue_item();
    assert!(item.title.contains("workspace"), "{}", item.title);
    assert!(item.title.contains("disk"), "{}", item.title);
    assert!(item.detail.contains("3.0 MiB free"), "{}", item.detail);
    assert!(item.detail.contains("5.0 GiB"), "{}", item.detail);
    assert!(item.detail.contains("/volume/tree"), "{}", item.detail);
    assert!(!item.answers.is_empty());

    let Err(DiskError::Paused(before_phase)) =
        guard.check(Path::new("/volume/tree"), Before::Phase)
    else {
        panic!("expected a pause");
    };
    assert!(before_phase.queue_item().title.contains("phase"));
}

#[test]
fn a_phase_that_ends_with_no_result_on_a_full_volume_fails_with_cause_disk() {
    let free = FakeFree::new(0);
    let guard = DiskGuard::new(&free, FLOOR);
    let full = guard.volume_full(Path::new("/anywhere")).unwrap();
    let state = phase_ending(&PhaseEnding {
        exit_recorded: false,
        result_seen: false,
        volume_full: full,
    });
    assert_eq!(
        state,
        PhaseState::Failed {
            cause: Some(FailCause::Disk)
        }
    );
    free.0.set(Some(u64::MAX));
    assert!(!guard.volume_full(Path::new("/anywhere")).unwrap());
}

#[test]
fn the_system_reader_reads_the_volume_of_a_path_that_does_not_exist_yet() {
    let fx = Fixture::new("system-free");
    let free = SystemFreeSpace;
    let here = free.free_bytes(&fx.root).unwrap();
    let below = free.free_bytes(&fx.root.join("not/made/yet")).unwrap();
    assert!(here > 0);
    // Same volume, read a moment apart: within a generous margin of each other.
    assert!(
        here.abs_diff(below) < 1024 * 1024 * 1024,
        "{here} vs {below}"
    );
}

#[test]
fn the_default_floor_is_five_gibibytes() {
    assert_eq!(Floor::default().min_free_bytes, 5 * 1024 * 1024 * 1024);
}

// --- Disk space at phase end ---

#[test]
fn the_disk_space_a_workspace_uses_is_returned_when_a_phase_ends() {
    let fx = Fixture::new("size");
    let free = FakeFree::new(u64::MAX);
    let guard = DiskGuard::new(&free, FLOOR);
    let mut ws = fx.workspace();
    ws.provision(&guard).unwrap();
    ws.start_phase(&guard).unwrap();
    let before = disk_use(&fx.tree()).unwrap();
    fs::write(fx.tree().join("big.bin"), vec![7u8; 300_000]).unwrap();
    let recorded = ws.end_phase().unwrap();
    assert_eq!(ws.state(), WorkspaceState::Idle);
    assert!(
        recorded >= before + 300_000,
        "recorded {recorded}, before {before}"
    );
    assert_eq!(recorded, disk_use(&fx.tree()).unwrap());
}

#[test]
fn a_hard_linked_file_is_counted_once() {
    let fx = Fixture::new("size-hardlink");
    let dir = fx.root.join("plain");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("a"), vec![1u8; 200_000]).unwrap();
    let one = disk_use(&dir).unwrap();
    fs::hard_link(dir.join("a"), dir.join("b")).unwrap();
    let two_names = disk_use(&dir).unwrap();
    assert!(two_names < one + 100_000, "{one} then {two_names}");
}

#[test]
fn a_symbolic_link_is_not_followed() {
    let fx = Fixture::new("size-symlink");
    let outside = fx.root.join("outside");
    fs::create_dir_all(&outside).unwrap();
    fs::write(outside.join("huge"), vec![1u8; 400_000]).unwrap();
    let dir = fx.root.join("plain");
    fs::create_dir_all(&dir).unwrap();
    let empty = disk_use(&dir).unwrap();
    std::os::unix::fs::symlink(outside.join("huge"), dir.join("to-file")).unwrap();
    std::os::unix::fs::symlink(&outside, dir.join("to-dir")).unwrap();
    let linked = disk_use(&dir).unwrap();
    assert!(linked < empty + 100_000, "{empty} then {linked}");
}

#[test]
fn the_shared_git_directory_is_not_part_of_a_workspace() {
    let fx = Fixture::new("size-gitdir");
    let ws = fx.idle();
    let before = disk_use(&fx.tree()).unwrap();
    // A large object in the main repository's history is shared by every worktree.
    fs::write(fx.repo.join("history.bin"), vec![3u8; 500_000]).unwrap();
    git(&fx.repo, &["add", "history.bin"]);
    git(&fx.repo, &["commit", "-q", "-m", "big"]);
    assert_eq!(disk_use(&fx.tree()).unwrap(), before);
    drop(ws);
}

#[test]
fn a_path_that_cannot_be_read_is_an_error_that_names_it() {
    let fx = Fixture::new("size-missing");
    let error = disk_use(&fx.root.join("missing")).unwrap_err();
    assert!(error.to_string().contains("missing"), "{error}");
}
