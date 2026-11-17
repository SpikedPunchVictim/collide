//! The lines of a board with their trust, and `board.trust` and `board.trust_review` (ts-0102-vu; 0.9, Untrusted
//! input). A daemon runs in the test's own process over a fixture checkout in a temp root, which is also the home, so
//! the user's `trust.yaml` is the test's own. A line's `snapshot` is the commit its branch names and its trust is the
//! hash of what the commit holds.

mod common;

use collide_core::owner::{
    BoardListParams, BoardListResult, BoardOpenParams, BoardOpenResult, BoardSummary,
    BoardTrustParams, BoardTrustResult, BoardTrustReviewParams, BoardTrustReviewResult,
    LineSummary, PackFileChangeKind, RpcError, TrustChangeKind, TrustState, code,
};
use collide_host::owner_client::ClientError;
use common::config_fixture::Fixture;
use std::fs;
use std::time::Duration;

const WAIT: Option<Duration> = Some(Duration::from_secs(20));

fn rpc<R: serde::de::DeserializeOwned>(
    fixture: &Fixture,
    method: &str,
    params: &impl serde::Serialize,
) -> Result<R, RpcError> {
    match fixture.client().call(method, params, WAIT) {
        Ok(result) => Ok(result),
        Err(ClientError::Rpc(error)) => Err(error),
        Err(other) => panic!("{other}"),
    }
}

fn list(fixture: &Fixture) -> Vec<BoardSummary> {
    rpc::<BoardListResult>(fixture, "board.list", &BoardListParams {})
        .unwrap()
        .boards
}

fn line(fixture: &Fixture, board: &str) -> LineSummary {
    let boards = list(fixture);
    let found = boards.iter().find(|summary| summary.id == board).unwrap();
    assert_eq!(found.lines.len(), 1, "{found:?}");
    found.lines[0].clone()
}

fn trust(fixture: &Fixture, board: &str, hash: &str) -> Result<BoardTrustResult, RpcError> {
    rpc(
        fixture,
        "board.trust",
        &BoardTrustParams {
            board: board.to_string(),
            line: "main".to_string(),
            hash: hash.to_string(),
            hosts: Vec::new(),
            dev_folders: Vec::new(),
        },
    )
}

fn review(fixture: &Fixture, board: &str) -> BoardTrustReviewResult {
    rpc(
        fixture,
        "board.trust_review",
        &BoardTrustReviewParams {
            board: board.to_string(),
            line: "main".to_string(),
        },
    )
    .unwrap()
}

fn hash_of(state: &TrustState) -> String {
    match state {
        TrustState::Untrusted { hash } | TrustState::Trusted { hash } => hash.clone(),
    }
}

fn trust_file(fixture: &Fixture) -> std::path::PathBuf {
    fixture.home().join(".config/collide/trust.yaml")
}

// ---- the line in board.list and board.open ----

#[test]
fn the_line_has_the_commit_its_branch_names_and_a_real_hash() {
    let fixture = Fixture::new();
    let board = fixture.board();
    let found = line(&fixture, &board);
    assert_eq!(found.ref_name, "main");
    assert_eq!(
        found.snapshot,
        Some(fixture.repo.git(&["rev-parse", "main"]))
    );
    assert!(matches!(&found.trust, TrustState::Untrusted { hash } if hash.len() == 40));
    assert!(found.problems.is_empty());
}

#[test]
fn board_open_answers_the_same_line_as_board_list() {
    let fixture = Fixture::new();
    let opened: BoardOpenResult = rpc(
        &fixture,
        "board.open",
        &BoardOpenParams {
            root: fs::canonicalize(fixture.repo.work())
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
            init_git: false,
        },
    )
    .unwrap();
    let board = opened.board.unwrap();
    assert_eq!(list(&fixture), vec![board.clone()]);
    assert!(board.lines[0].snapshot.is_some());
}

#[test]
fn after_board_trust_with_that_hash_the_state_is_trusted_at_the_same_hash() {
    let fixture = Fixture::new();
    let board = fixture.board();
    let hash = hash_of(&line(&fixture, &board).trust);
    let result = trust(&fixture, &board, &hash).unwrap();
    assert_eq!(result.trust, TrustState::Trusted { hash: hash.clone() });
    assert_eq!(line(&fixture, &board).trust, TrustState::Trusted { hash });
}

#[test]
fn a_commit_that_changes_board_yaml_makes_the_line_untrusted_at_another_hash() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    let board = fixture.board();
    let before = hash_of(&line(&fixture, &board).trust);
    trust(&fixture, &board, &before).unwrap();
    fixture.write(
        ".collide/board.yaml",
        "# a comment\npacks:\n  - source: repo:.collide/packs/ferry\n",
    );
    fixture.commit();
    let after = line(&fixture, &board);
    assert!(matches!(&after.trust, TrustState::Untrusted { hash } if *hash != before));
    assert_eq!(
        after.snapshot,
        Some(fixture.repo.git(&["rev-parse", "main"]))
    );
}

#[test]
fn a_stale_hash_is_a_conflict_that_records_nothing() {
    let fixture = Fixture::new();
    let board = fixture.board();
    let error = trust(&fixture, &board, &"0".repeat(40)).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(!trust_file(&fixture).exists());
    assert!(matches!(
        line(&fixture, &board).trust,
        TrustState::Untrusted { .. }
    ));
}

#[test]
fn a_hash_that_is_stale_after_a_commit_is_refused_and_the_old_one_is_not_trusted_either() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    let board = fixture.board();
    let shown = hash_of(&line(&fixture, &board).trust);
    fixture.write(
        ".collide/packs/ferry/pack.yaml",
        "apiVersion: collide/v0\nname: ferry\nversion: 2.0.0\n",
    );
    fixture.commit();
    let error = trust(&fixture, &board, &shown).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(!trust_file(&fixture).exists());
}

#[test]
fn a_bad_board_yaml_lists_the_line_with_its_problems_and_no_hash() {
    let fixture = Fixture::new();
    fixture.write(".collide/board.yaml", "packs: 7\n");
    fixture.commit();
    let board = fixture.board();
    let found = line(&fixture, &board);
    assert_eq!(
        found.trust,
        TrustState::Untrusted {
            hash: String::new()
        }
    );
    assert!(found.snapshot.is_some());
    assert!(
        found
            .problems
            .iter()
            .any(|problem| problem.contains("board.yaml")),
        "{:?}",
        found.problems
    );
}

#[test]
fn a_checkout_with_no_commit_has_no_line() {
    let fixture = Fixture::new();
    let empty = common::git_repo::GitRepo::new();
    let opened: BoardOpenResult = rpc(
        &fixture,
        "board.open",
        &BoardOpenParams {
            root: fs::canonicalize(empty.work())
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
            init_git: false,
        },
    )
    .unwrap();
    assert!(opened.board.unwrap().lines.is_empty());
    assert!(list(&fixture).iter().all(|board| board.lines.is_empty()));
}

#[test]
fn a_folder_that_is_no_longer_a_checkout_is_listed_with_no_line() {
    let fixture = Fixture::new();
    let board = fixture.board();
    fs::remove_dir_all(fixture.repo.git_dir()).unwrap();
    let boards = list(&fixture);
    let found = boards.iter().find(|summary| summary.id == board).unwrap();
    assert!(found.lines.is_empty());
}

#[test]
fn a_detached_head_is_the_line_named_head() {
    let fixture = Fixture::new();
    let board = fixture.board();
    fixture.repo.git(&["checkout", "--quiet", "--detach"]);
    let found = line(&fixture, &board);
    assert_eq!(found.ref_name, "HEAD");
    assert_eq!(
        found.snapshot,
        Some(fixture.repo.git(&["rev-parse", "HEAD"]))
    );
}

#[test]
fn a_trust_file_that_cannot_be_read_leaves_the_line_untrusted_and_names_the_file() {
    let fixture = Fixture::new();
    let board = fixture.board();
    fixture.write_user("trust.yaml", "boards: [unclosed\n");
    let found = line(&fixture, &board);
    assert!(matches!(found.trust, TrustState::Untrusted { .. }));
    assert!(
        found
            .problems
            .iter()
            .any(|problem| problem.contains("trust.yaml")),
        "{:?}",
        found.problems
    );
}

#[test]
fn board_trust_for_a_board_the_daemon_does_not_know_is_not_found() {
    let fixture = Fixture::new();
    let error = trust(&fixture, "board-99", "x").unwrap_err();
    assert_eq!(error.code, code::NOT_FOUND);
}

// ---- the review ----

#[test]
fn before_any_trust_the_review_lists_everything_as_added_with_the_packs_review() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    let board = fixture.board();
    let shown = review(&fixture, &board);
    let names: Vec<&str> = shown
        .changes
        .iter()
        .map(|change| change.name.as_str())
        .collect();
    assert!(names.contains(&"board.yaml"), "{names:?}");
    assert!(
        names.contains(&"pack repo:.collide/packs/ferry"),
        "{names:?}"
    );
    assert!(
        shown
            .changes
            .iter()
            .all(|change| change.kind == TrustChangeKind::Added)
    );
    assert_eq!(shown.packs.len(), 1);
    assert_eq!(shown.packs[0].source, "repo:.collide/packs/ferry");
    assert_eq!(shown.packs[0].review.name, "ferry");
    assert_eq!(
        hash_of(&shown.trust),
        hash_of(&line(&fixture, &board).trust)
    );
    assert!(shown.problems.is_empty() && shown.hosts.is_empty());
}

#[test]
fn after_trust_a_committed_pack_change_is_listed_with_its_file_and_lines() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    let board = fixture.board();
    let hash = hash_of(&line(&fixture, &board).trust);
    trust(&fixture, &board, &hash).unwrap();
    fixture.write(
        ".collide/packs/ferry/pack.yaml",
        "apiVersion: collide/v0\nname: ferry\nversion: 1.1.0\n",
    );
    fixture.commit();
    let shown = review(&fixture, &board);
    assert_eq!(shown.changes.len(), 1, "{:?}", shown.changes);
    let change = &shown.changes[0];
    assert_eq!(change.name, "pack repo:.collide/packs/ferry");
    assert_eq!(change.kind, TrustChangeKind::Changed);
    assert_eq!(change.files.len(), 1);
    assert_eq!(change.files[0].path, "pack.yaml");
    assert_eq!(change.files[0].kind, PackFileChangeKind::Modified);
    assert!(
        change.files[0]
            .lines
            .contains(&"+version: 1.1.0".to_string())
    );
    assert_eq!(shown.packs[0].review.version.as_deref(), Some("1.1.0"));
}

#[test]
fn a_changed_file_input_lists_its_changed_lines() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    let board = fixture.board();
    let hash = hash_of(&line(&fixture, &board).trust);
    trust(&fixture, &board, &hash).unwrap();
    fixture.write(
        ".collide/board.yaml",
        "# changed\npacks:\n  - source: repo:.collide/packs/ferry\n",
    );
    fixture.commit();
    let shown = review(&fixture, &board);
    assert_eq!(shown.changes.len(), 1, "{:?}", shown.changes);
    assert_eq!(shown.changes[0].name, "board.yaml");
    assert!(shown.changes[0].lines.contains(&"+# changed".to_string()));
}

#[test]
fn a_review_writes_nothing() {
    let fixture = Fixture::new();
    fixture.ferry();
    fixture.commit();
    let board = fixture.board();
    review(&fixture, &board);
    review(&fixture, &board);
    assert!(!trust_file(&fixture).exists());
    assert!(!fixture.home().join(".config/collide/hosts.yaml").exists());
    assert!(matches!(
        line(&fixture, &board).trust,
        TrustState::Untrusted { .. }
    ));
}

#[test]
fn a_review_of_a_line_that_does_not_load_has_no_hash_and_the_problems() {
    let fixture = Fixture::new();
    fixture.write(".collide/board.yaml", "packs: 7\n");
    fixture.commit();
    let board = fixture.board();
    let shown = review(&fixture, &board);
    assert_eq!(
        shown.trust,
        TrustState::Untrusted {
            hash: String::new()
        }
    );
    assert!(
        shown
            .problems
            .iter()
            .any(|problem| problem.contains("board.yaml"))
    );
    assert!(shown.changes.is_empty());
}

#[test]
fn a_review_of_a_ref_that_does_not_exist_is_not_found() {
    let fixture = Fixture::new();
    let board = fixture.board();
    let error = rpc::<BoardTrustReviewResult>(
        &fixture,
        "board.trust_review",
        &BoardTrustReviewParams {
            board,
            line: "no-such-branch".to_string(),
        },
    )
    .unwrap_err();
    assert_eq!(error.code, code::NOT_FOUND);
}

// ---- a first fetch from a host, and a git pack's diff ----

/// A `git` on its own `PATH` that records each run and fails. Returns the `PATH` and the file it records in.
fn recording_git(root: &std::path::Path) -> (std::ffi::OsString, std::path::PathBuf) {
    use std::os::unix::fs::PermissionsExt;
    let dir = root.join("fake-git");
    fs::create_dir(&dir).unwrap();
    let ran = root.join("fake-git-ran");
    let body = format!("#!/bin/sh\necho ran >> '{}'\nexit 1\n", ran.display());
    fs::write(dir.join("git"), body).unwrap();
    fs::set_permissions(dir.join("git"), fs::Permissions::from_mode(0o755)).unwrap();
    (dir.into_os_string(), ran)
}

const COLD_SOURCE: &str = "git:https://example.invalid/o/r//pack@v1";

/// A board whose one pack is a `git:` pack pinned at a commit the cache does not have, on a host never fetched from.
fn cold_world() -> (common::TempRoot, Fixture, String, std::path::PathBuf) {
    let fake = common::TempRoot::new();
    let (path, ran) = recording_git(fake.path());
    let fixture = Fixture::fetching(path);
    fixture.write(
        ".collide/board.yaml",
        &format!("packs:\n  - source: {COLD_SOURCE}\n"),
    );
    fixture.write(
        ".collide/packs.lock",
        &format!(
            "packs:\n  - source: {COLD_SOURCE}\n    commit: {}\n    tree: {}\n",
            "c".repeat(40),
            "d".repeat(40)
        ),
    );
    fixture.commit();
    let board = fixture.board();
    (fake, fixture, board, ran)
}

fn approve(fixture: &Fixture, board: &str, hosts: &[&str]) -> Result<BoardTrustResult, RpcError> {
    rpc(
        fixture,
        "board.trust",
        &BoardTrustParams {
            board: board.to_string(),
            line: "main".to_string(),
            hash: String::new(),
            hosts: hosts.iter().map(|host| host.to_string()).collect(),
            dev_folders: Vec::new(),
        },
    )
}

fn hosts_file(fixture: &Fixture) -> std::path::PathBuf {
    fixture.home().join(".config/collide/hosts.yaml")
}

#[test]
fn a_cold_cache_pack_on_a_new_host_is_a_review_with_the_host_and_no_hash() {
    let (_fake, fixture, board, ran) = cold_world();
    let shown = review(&fixture, &board);
    assert_eq!(shown.hosts, vec!["example.invalid".to_string()]);
    assert_eq!(
        shown.trust,
        TrustState::Untrusted {
            hash: String::new()
        }
    );
    assert!(
        shown
            .problems
            .iter()
            .any(|problem| problem.contains("example.invalid"))
    );
    assert!(!ran.exists(), "git ran");
    assert!(!hosts_file(&fixture).exists());
}

#[test]
fn the_line_in_board_list_carries_the_same_problem() {
    let (_fake, fixture, board, _ran) = cold_world();
    let found = line(&fixture, &board);
    assert_eq!(
        found.trust,
        TrustState::Untrusted {
            hash: String::new()
        }
    );
    assert!(
        found
            .problems
            .iter()
            .any(|problem| problem.contains("example.invalid"))
    );
}

#[test]
fn approving_the_host_records_it_and_lets_the_fetch_go_on() {
    let (_fake, fixture, board, ran) = cold_world();
    approve(&fixture, &board, &["example.invalid"]).unwrap();
    let hosts = fs::read_to_string(hosts_file(&fixture)).unwrap();
    assert!(hosts.contains("example.invalid"), "{hosts}");
    assert!(ran.exists(), "the fetch was not tried after the yes");
}

#[test]
fn approving_a_host_the_line_does_not_need_records_nothing() {
    let (_fake, fixture, board, ran) = cold_world();
    approve(&fixture, &board, &["other.invalid"]).unwrap();
    assert!(!hosts_file(&fixture).exists());
    assert!(!ran.exists(), "git ran");
}

#[test]
fn an_empty_hash_with_nothing_to_approve_is_refused() {
    let fixture = Fixture::new();
    let board = fixture.board();
    let error = approve(&fixture, &board, &[]).unwrap_err();
    assert_eq!(error.code, code::INVALID_PARAMS);
    assert!(!trust_file(&fixture).exists());
}

/// A board whose pack is `pack/` of a remote on disk, pinned in the repo's lock.
struct GitWorld {
    fixture: Fixture,
    remote: common::remote::Remote,
    source: String,
    board: String,
}

fn git_world() -> GitWorld {
    let remote = common::remote::Remote::new();
    remote.work.write(
        "pack/pack.yaml",
        b"apiVersion: collide/v0\nname: remote\nversion: 1.0.0\n",
    );
    remote.work.commit_all("one");
    remote.publish();
    let fixture = Fixture::fetching(std::env::var_os("PATH").unwrap_or_default());
    let source = format!("git:{}//pack@main", remote.file_url());
    fixture.write(
        ".collide/board.yaml",
        &format!("packs:\n  - source: {source}\n"),
    );
    let world = GitWorld {
        fixture,
        remote,
        source,
        board: String::new(),
    };
    world.pin();
    let board = world.fixture.board();
    GitWorld { board, ..world }
}

impl GitWorld {
    /// Pins the lock at what the remote has now and commits the repo.
    fn pin(&self) {
        let commit = self.remote.work.git(&["rev-parse", "HEAD"]);
        let tree = self.remote.work.git(&["rev-parse", "HEAD:pack"]);
        self.fixture.write(
            ".collide/packs.lock",
            &format!(
                "packs:\n  - source: {}\n    commit: {commit}\n    tree: {tree}\n",
                self.source
            ),
        );
        self.fixture.commit();
    }
}

#[test]
fn a_git_packs_change_shows_its_files_from_the_mirror() {
    let w = git_world();
    let hash = hash_of(&line(&w.fixture, &w.board).trust);
    assert_eq!(hash.len(), 40, "{:?}", line(&w.fixture, &w.board));
    trust(&w.fixture, &w.board, &hash).unwrap();
    w.remote.work.write(
        "pack/pack.yaml",
        b"apiVersion: collide/v0\nname: remote\nversion: 2.0.0\n",
    );
    w.remote.work.commit_all("two");
    w.remote.publish();
    w.pin();
    let shown = review(&w.fixture, &w.board);
    assert_eq!(shown.changes.len(), 2, "{:?}", shown.changes);
    let pack = shown
        .changes
        .iter()
        .find(|change| change.name == format!("pack {}", w.source))
        .unwrap_or_else(|| panic!("{:?}", shown.changes));
    assert_eq!(pack.kind, TrustChangeKind::Changed);
    assert_eq!(pack.files.len(), 1, "{pack:?}");
    assert_eq!(pack.files[0].path, "pack.yaml");
    assert!(pack.files[0].lines.contains(&"+version: 2.0.0".to_string()));
}
