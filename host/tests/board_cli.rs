//! `collide init` and `collide trust` (ts-0102-vu; 0.9 Trust, 0.11 Share with team) over a daemon in the test's own
//! process: a fixture checkout, a temp home, and a person's answers given as input. What reaches the binary (the usage
//! and what a pipe with no terminal gets) is in `board_process.rs`.

mod common;

use collide_core::owner::{BoardListParams, BoardListResult, BoardSummary, TrustState};
use collide_host::board_cli::{USAGE, init_with, trust_with};
use collide_host::task_cli::exit;
use common::config_fixture::Fixture;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

struct Run {
    code: u8,
    out: String,
    err: String,
}

fn init(fixture: &Fixture, cwd: &Path, list: &[&str]) -> Run {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = init_with(fixture.daemon.paths(), &args(list), cwd, &mut out, &mut err);
    Run {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

/// `collide trust <list>` from the checkout, at a terminal when `tty`, with `input` typed.
fn trust_at(fixture: &Fixture, cwd: &Path, list: &[&str], tty: bool, input: &str) -> Run {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let code = trust_with(
        fixture.daemon.paths(),
        &args(list),
        cwd,
        tty,
        &mut Cursor::new(input.as_bytes().to_vec()),
        &mut out,
        &mut err,
    );
    Run {
        code,
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
    }
}

fn trust(fixture: &Fixture, list: &[&str], tty: bool, input: &str) -> Run {
    trust_at(fixture, fixture.repo.work(), list, tty, input)
}

fn boards(fixture: &Fixture) -> Vec<BoardSummary> {
    fixture
        .client()
        .call::<_, BoardListResult>(
            "board.list",
            &BoardListParams {},
            Some(std::time::Duration::from_secs(30)),
        )
        .unwrap()
        .boards
}

/// The trust state of the one line of the fixture's board.
fn state(fixture: &Fixture) -> TrustState {
    fixture.board();
    let boards = boards(fixture);
    assert_eq!(boards.len(), 1, "{boards:?}");
    assert_eq!(boards[0].lines.len(), 1, "{:?}", boards[0]);
    boards[0].lines[0].trust.clone()
}

fn is_trusted(fixture: &Fixture) -> bool {
    matches!(state(fixture), TrustState::Trusted { .. })
}

fn hash_of(fixture: &Fixture) -> String {
    match state(fixture) {
        TrustState::Trusted { hash } | TrustState::Untrusted { hash } => hash,
    }
}

fn head(fixture: &Fixture) -> String {
    fixture.repo.git(&["rev-parse", "HEAD"]).trim().to_string()
}

// ---- collide init ----

#[test]
fn init_writes_the_one_commented_board_file_and_names_it() {
    let fixture = Fixture::new();
    let run = init(&fixture, fixture.repo.work(), &[]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(run.out.contains(".collide/board.yaml"), "{}", run.out);
    let text = fs::read_to_string(fixture.repo.work().join(".collide/board.yaml")).unwrap();
    assert!(text.starts_with('#'), "{text}");
    assert!(text.contains("packs: []"), "{text}");
    let found: Vec<_> = fs::read_dir(fixture.repo.work().join(".collide"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name())
        .collect();
    assert_eq!(found.len(), 1, "{found:?}");
}

#[test]
fn init_commits_nothing_and_leaves_gitignore_alone() {
    let fixture = Fixture::new();
    let before = head(&fixture);
    let run = init(&fixture, fixture.repo.work(), &[]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert_eq!(head(&fixture), before);
    assert!(!fixture.repo.work().join(".gitignore").exists());
}

#[test]
fn init_with_dry_run_prints_the_file_it_would_write_and_writes_nothing() {
    let fixture = Fixture::new();
    let run = init(&fixture, fixture.repo.work(), &["--dry-run"]);
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(run.out.contains("would write"), "{}", run.out);
    assert!(run.out.contains(".collide/board.yaml"), "{}", run.out);
    assert!(!fixture.repo.work().join(".collide").exists());
}

#[test]
fn init_refuses_when_a_collide_folder_exists_and_changes_nothing() {
    let fixture = Fixture::new();
    fixture.write(".collide/config.yaml", "disk: { floor_gib: 3 }\n");
    let run = init(&fixture, fixture.repo.work(), &[]);
    assert_eq!(run.code, exit::FAILED);
    assert!(run.err.contains(".collide"), "{}", run.err);
    assert!(!fixture.repo.work().join(".collide/board.yaml").exists());
}

#[test]
fn init_outside_a_checkout_fails_and_says_so() {
    let fixture = Fixture::new();
    let outside = common::TempRoot::new();
    let run = init(&fixture, outside.path(), &[]);
    assert_eq!(run.code, exit::FAILED);
    assert!(run.err.contains("not in a git checkout"), "{}", run.err);
}

#[test]
fn a_bad_init_command_line_exits_2_with_the_usage() {
    let fixture = Fixture::new();
    for bad in [
        &["--nope"][..],
        &["extra"][..],
        &["--dry-run", "--dry-run"][..],
    ] {
        let run = init(&fixture, fixture.repo.work(), bad);
        assert_eq!(run.code, exit::USAGE, "{bad:?}");
        assert!(run.err.contains("usage: collide init"), "{}", run.err);
    }
    assert!(USAGE.contains("collide trust"));
}

#[test]
fn after_init_and_a_commit_of_exactly_that_the_line_is_trusted() {
    // `init` records the hash of what it wrote (ts-0194-qp), so the person's own commit of it asks nothing.
    let fixture = Fixture::new();
    assert_eq!(init(&fixture, fixture.repo.work(), &[]).code, exit::OK);
    fixture.commit();
    match state(&fixture) {
        TrustState::Trusted { hash } => assert!(!hash.is_empty()),
        other => panic!("{other:?}"),
    }
}

#[test]
fn after_init_and_a_commit_that_adds_more_the_line_is_untrusted_at_a_real_hash() {
    let fixture = Fixture::new();
    assert_eq!(init(&fixture, fixture.repo.work(), &[]).code, exit::OK);
    fixture.write(
        ".collide/views/extra.yaml",
        "name: extra\nwhere: \"true\"\n",
    );
    fixture.commit();
    match state(&fixture) {
        TrustState::Untrusted { hash } => assert!(!hash.is_empty()),
        other => panic!("{other:?}"),
    }
}

// ---- collide trust: the command line ----

#[test]
fn a_bad_trust_command_line_exits_2_with_the_usage() {
    let fixture = Fixture::new();
    for bad in [
        &["--line"][..],
        &["--line", ""][..],
        &["extra"][..],
        &["--nope"][..],
        &["--yes", "--yes"][..],
        &["--allow-host"][..],
    ] {
        let run = trust(&fixture, bad, true, "");
        assert_eq!(run.code, exit::USAGE, "{bad:?}");
        assert!(run.err.contains("usage: collide trust"), "{}", run.err);
    }
}

// ---- collide trust: the review and the question ----

fn a_repo_pack(fixture: &Fixture) {
    fixture.ferry();
    fixture.commit();
}

#[test]
fn the_review_shows_the_hash_each_change_and_each_packs_review_before_it_asks() {
    let fixture = Fixture::new();
    a_repo_pack(&fixture);
    let hash = hash_of(&fixture);
    let run = trust(&fixture, &[], true, "n\n");
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(run.out.contains(&hash), "{}", run.out);
    assert!(run.out.contains("added board.yaml"), "{}", run.out);
    assert!(
        run.out.contains("pack repo:.collide/packs/ferry"),
        "{}",
        run.out
    );
    assert!(run.out.contains("Pack ferry 1.0.0"), "{}", run.out);
    let asked = run.out.find("Trust this? [y/N]").expect("the question");
    assert!(asked > run.out.find("Pack ferry").unwrap(), "{}", run.out);
}

#[test]
fn a_yes_at_the_terminal_trusts_the_shown_hash() {
    let fixture = Fixture::new();
    a_repo_pack(&fixture);
    let hash = hash_of(&fixture);
    let run = trust(&fixture, &[], true, "y\n");
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert_eq!(state(&fixture), TrustState::Trusted { hash });
}

#[test]
fn a_no_an_empty_answer_and_a_closed_input_each_trust_nothing() {
    for input in ["n\n", "\n", ""] {
        let fixture = Fixture::new();
        a_repo_pack(&fixture);
        let run = trust(&fixture, &[], true, input);
        assert_eq!(run.code, exit::OK, "{input:?}: {}", run.err);
        assert!(run.out.contains("Nothing was trusted"), "{}", run.out);
        assert!(!is_trusted(&fixture), "{input:?}");
    }
}

#[test]
fn with_no_terminal_and_no_yes_it_refuses_names_the_way_out_and_trusts_nothing() {
    let fixture = Fixture::new();
    a_repo_pack(&fixture);
    let run = trust(&fixture, &[], false, "y\n");
    assert_eq!(run.code, exit::REFUSED);
    assert!(run.err.contains("--yes"), "{}", run.err);
    assert!(run.err.contains("terminal"), "{}", run.err);
    assert!(!is_trusted(&fixture));
}

#[test]
fn yes_trusts_the_shown_hash_with_no_question_asked() {
    let fixture = Fixture::new();
    a_repo_pack(&fixture);
    let hash = hash_of(&fixture);
    let run = trust(&fixture, &["--yes"], false, "");
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(!run.out.contains("[y/N]"), "{}", run.out);
    assert!(run.out.contains(&hash), "{}", run.out);
    assert_eq!(state(&fixture), TrustState::Trusted { hash });
}

#[test]
fn a_line_that_is_trusted_already_is_said_so_and_asks_nothing() {
    let fixture = Fixture::new();
    a_repo_pack(&fixture);
    assert_eq!(trust(&fixture, &["--yes"], false, "").code, exit::OK);
    let run = trust(&fixture, &[], false, "");
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(run.out.contains("already trusted"), "{}", run.out);
}

#[test]
fn a_committed_change_to_a_repo_pack_loads_after_the_prompt_that_showed_its_diff() {
    let fixture = Fixture::new();
    a_repo_pack(&fixture);
    assert_eq!(trust(&fixture, &["--yes"], false, "").code, exit::OK);
    fixture.write(
        ".collide/packs/ferry/pack.yaml",
        "apiVersion: collide/v0\nname: ferry\nversion: 1.1.0\n",
    );
    fixture.commit();
    assert!(!is_trusted(&fixture));
    let run = trust(&fixture, &[], true, "y\n");
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(
        run.out.contains("changed pack repo:.collide/packs/ferry"),
        "{}",
        run.out
    );
    assert!(run.out.contains("modified pack.yaml"), "{}", run.out);
    assert!(run.out.contains("+version: 1.1.0"), "{}", run.out);
    assert!(is_trusted(&fixture));
    assert!(!fixture.repo.work().join(".collide/packs.lock").exists());
}

#[test]
fn a_line_that_does_not_exist_exits_with_the_id_code() {
    let fixture = Fixture::new();
    a_repo_pack(&fixture);
    let run = trust(&fixture, &["--line", "no-such-branch", "--yes"], false, "");
    assert_eq!(run.code, exit::ID, "{}", run.err);
    assert!(!is_trusted(&fixture));
}

#[test]
fn a_line_named_with_line_is_the_one_trusted() {
    let fixture = Fixture::new();
    a_repo_pack(&fixture);
    let run = trust(&fixture, &["--line", "main", "--yes"], false, "");
    assert_eq!(run.code, exit::OK, "{}", run.err);
    assert!(is_trusted(&fixture));
}

#[test]
fn a_line_that_does_not_load_prints_its_problems_and_exits_1() {
    let fixture = Fixture::new();
    fixture.write(".collide/board.yaml", "packs: 7\n");
    fixture.commit();
    let run = trust(&fixture, &["--yes"], false, "");
    assert_eq!(run.code, exit::FAILED);
    assert!(run.err.contains("board.yaml"), "{}", run.err);
    assert!(!is_trusted(&fixture));
}

#[test]
fn a_checkout_that_is_no_board_is_said_so() {
    let fixture = Fixture::new();
    let outside = common::TempRoot::new();
    let run = trust_at(&fixture, outside.path(), &["--yes"], false, "");
    assert_eq!(run.code, exit::FAILED);
    assert!(run.err.contains("not in a git checkout"), "{}", run.err);
}

// ---- collide trust: a host and a dev folder the load needs ----

mod cold {
    use super::*;
    use std::os::unix::fs::PermissionsExt;

    const SOURCE: &str = "git:https://example.invalid/o/r//pack@v1";

    /// A `git` on its own `PATH` that records each run and fails, so no fetch goes anywhere.
    fn recording_git(root: &Path) -> std::ffi::OsString {
        let dir = root.join("fake-git");
        fs::create_dir(&dir).unwrap();
        let body = format!(
            "#!/bin/sh\necho ran >> '{}'\nexit 1\n",
            root.join("git-ran").display()
        );
        fs::write(dir.join("git"), body).unwrap();
        fs::set_permissions(dir.join("git"), fs::Permissions::from_mode(0o755)).unwrap();
        dir.into_os_string()
    }

    /// A board whose one pack is a `git:` pack at a commit the cache lacks, on a host never fetched from.
    pub fn world() -> (common::TempRoot, Fixture) {
        let fake = common::TempRoot::new();
        let fixture = Fixture::fetching(recording_git(fake.path()));
        fixture.write(
            ".collide/board.yaml",
            &format!("packs:\n  - source: {SOURCE}\n"),
        );
        fixture.write(
            ".collide/packs.lock",
            &format!(
                "packs:\n  - source: {SOURCE}\n    commit: {}\n    tree: {}\n",
                "c".repeat(40),
                "d".repeat(40)
            ),
        );
        fixture.commit();
        (fake, fixture)
    }

    pub fn hosts_file(fixture: &Fixture) -> PathBuf {
        fixture.home().join(".config/collide/hosts.yaml")
    }
}

#[test]
fn a_new_host_is_asked_about_by_name_and_recorded_on_a_yes() {
    let (_fake, fixture) = cold::world();
    let run = trust(&fixture, &[], true, "y\n");
    assert!(run.out.contains("example.invalid"), "{}", run.out);
    assert!(
        run.out.contains("Fetch packs from example.invalid? [y/N]"),
        "{}",
        run.out
    );
    let recorded = fs::read_to_string(cold::hosts_file(&fixture)).unwrap();
    assert!(recorded.contains("example.invalid"), "{recorded}");
    // The fetch itself fails here (the stand-in git), so the line still has problems and nothing is trusted.
    assert_eq!(run.code, exit::FAILED, "{}", run.err);
    assert!(!is_trusted(&fixture));
}

#[test]
fn a_no_to_a_new_host_records_nothing_and_fetches_nothing() {
    let (fake, fixture) = cold::world();
    let run = trust(&fixture, &[], true, "n\n");
    assert_eq!(run.code, exit::REFUSED, "{}", run.err);
    assert!(!cold::hosts_file(&fixture).exists());
    assert!(!fake.path().join("git-ran").exists());
}

#[test]
fn with_no_terminal_a_new_host_needs_allow_host_and_names_it() {
    let (_fake, fixture) = cold::world();
    let run = trust(&fixture, &["--yes"], false, "");
    assert_eq!(run.code, exit::REFUSED, "{}", run.err);
    assert!(
        run.err.contains("--allow-host example.invalid"),
        "{}",
        run.err
    );
    assert!(!cold::hosts_file(&fixture).exists());
}

#[test]
fn allow_host_records_the_host_without_a_question() {
    let (_fake, fixture) = cold::world();
    let run = trust(
        &fixture,
        &["--yes", "--allow-host", "example.invalid"],
        false,
        "",
    );
    assert!(!run.out.contains("[y/N]"), "{}", run.out);
    assert!(
        fs::read_to_string(cold::hosts_file(&fixture))
            .unwrap()
            .contains("example.invalid")
    );
}

/// A user pack from a `dev:` folder written into `packs.yaml` by hand, switched on for the board.
fn hand_written_dev_pack(fixture: &Fixture) -> PathBuf {
    let folder = fixture.home().join("devs/mine");
    fs::create_dir_all(&folder).unwrap();
    fs::write(
        folder.join("pack.yaml"),
        "apiVersion: collide/v0\nname: mine\nversion: 9.0.0\n",
    )
    .unwrap();
    fixture.write_user(
        "packs.yaml",
        &format!("packs:\n  - source: dev:{}\n", folder.display()),
    );
    let board = fixture.board();
    let boards = fixture.home().join(".config/collide/boards").join(board);
    fs::create_dir_all(&boards).unwrap();
    fs::write(boards.join("packs.yaml"), "on: [mine]\n").unwrap();
    fs::canonicalize(folder).unwrap()
}

#[test]
fn a_dev_folder_never_trusted_is_asked_about_and_trusted_on_a_yes_and_then_the_line_is_reviewed() {
    let fixture = Fixture::new();
    let folder = hand_written_dev_pack(&fixture);
    let run = trust(&fixture, &[], true, "y\ny\n");
    assert_eq!(run.code, exit::OK, "{}", run.err);
    let question = format!("Trust the dev: folder {}? [y/N]", folder.display());
    assert!(run.out.contains(&question), "{}", run.out);
    assert!(run.out.contains("Trust this? [y/N]"), "{}", run.out);
    assert!(is_trusted(&fixture));
}

#[test]
fn a_no_to_a_dev_folder_trusts_nothing_and_exits_refused() {
    let fixture = Fixture::new();
    hand_written_dev_pack(&fixture);
    let run = trust(&fixture, &[], true, "n\n");
    assert_eq!(run.code, exit::REFUSED, "{}", run.err);
    assert!(!is_trusted(&fixture));
}

#[test]
fn with_no_terminal_a_dev_folder_needs_allow_dev_and_names_it() {
    let fixture = Fixture::new();
    let folder = hand_written_dev_pack(&fixture);
    let run = trust(&fixture, &["--yes"], false, "");
    assert_eq!(run.code, exit::REFUSED, "{}", run.err);
    assert!(
        run.err
            .contains(&format!("--allow-dev {}", folder.display())),
        "{}",
        run.err
    );
    let again = trust(
        &fixture,
        &["--yes", "--allow-dev", folder.to_str().unwrap()],
        false,
        "",
    );
    assert_eq!(again.code, exit::OK, "{}", again.err);
    assert!(is_trusted(&fixture));
}
