//! The floor under which no run starts, read from `disk.floor_gib` through the settings layers for a board
//! (ts-0102-vu; 0.2, Durability; 0.7, Settings). The files are fixtures in a temp home and a fixture checkout. A bad
//! value is reported and the 5 GiB default holds.

mod common;

use collide_host::config_get::{BoardRef, ConfigService};
use collide_host::secret_store::MemorySecretStore;
use common::config_fixture::{Fixture, plain_factory};
use std::ffi::OsString;
use std::sync::Arc;

const GIB: u64 = 1024 * 1024 * 1024;

struct World {
    fixture: Fixture,
    board: String,
    service: ConfigService,
}

fn world() -> World {
    let fixture = Fixture::new();
    let board = fixture.board();
    let home = fixture.home().to_path_buf();
    let service = ConfigService::new(
        Some(home.to_str().unwrap().to_string()),
        plain_factory(&home),
        vec![
            ("HOME".to_string(), home.to_str().unwrap().to_string()),
            ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
            ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
        ],
        Arc::new(MemorySecretStore::default()),
    );
    World {
        fixture,
        board,
        service,
    }
}

impl World {
    /// The floor in GiB for the board, and the problems that were reported.
    fn floor(&self) -> (u64, Vec<String>) {
        let root = std::fs::canonicalize(self.fixture.repo.work()).unwrap();
        let board = BoardRef {
            id: &self.board,
            root: &root,
        };
        let read = self
            .service
            .floor(Some(board), &OsString::from("/usr/bin:/bin"));
        (read.floor.min_free_bytes / GIB, read.problems)
    }

    fn repo_sets(&self, gib: &str) {
        self.fixture.write(
            ".collide/config.yaml",
            &format!("disk: {{ floor_gib: {gib} }}\n"),
        );
        self.fixture.commit();
    }

    fn user_sets(&self, gib: &str) {
        self.fixture
            .write_user("config.yaml", &format!("disk: {{ floor_gib: {gib} }}\n"));
    }
}

#[test]
fn with_no_setting_the_floor_is_5_gib() {
    let w = world();
    assert_eq!(w.floor(), (5, Vec::new()));
}

#[test]
fn the_users_file_sets_the_floor() {
    let w = world();
    w.user_sets("1");
    assert_eq!(w.floor(), (1, Vec::new()));
}

#[test]
fn the_repos_file_sets_it_when_the_user_sets_nothing() {
    let w = world();
    w.repo_sets("9");
    assert_eq!(w.floor(), (9, Vec::new()));
}

#[test]
fn the_users_value_wins_over_the_repos_because_the_key_is_overridable() {
    let w = world();
    w.repo_sets("9");
    w.user_sets("2");
    assert_eq!(w.floor(), (2, Vec::new()));
}

#[test]
fn a_value_of_zero_is_reported_and_the_default_holds() {
    let w = world();
    w.user_sets("0");
    let (gib, problems) = w.floor();
    assert_eq!(gib, 5);
    assert!(
        problems.iter().any(|problem| problem.contains("floor_gib")),
        "{problems:?}"
    );
}

#[test]
fn a_string_is_reported_and_the_default_holds() {
    let w = world();
    w.user_sets("big");
    let (gib, problems) = w.floor();
    assert_eq!(gib, 5);
    assert!(
        problems.iter().any(|problem| problem.contains("floor_gib")),
        "{problems:?}"
    );
}

#[test]
fn a_bad_value_in_the_repos_file_is_reported_and_the_default_holds() {
    let w = world();
    w.repo_sets("0");
    let (gib, problems) = w.floor();
    assert_eq!(gib, 5);
    assert!(
        problems.iter().any(|problem| problem.contains("floor_gib")),
        "{problems:?}"
    );
}

#[test]
fn with_no_board_only_the_users_layers_count() {
    let w = world();
    w.repo_sets("9");
    w.user_sets("3");
    let read = w.service.floor(None, &OsString::from("/usr/bin:/bin"));
    assert_eq!(read.floor.min_free_bytes, 3 * GIB);
    assert!(read.problems.is_empty());
}

#[test]
fn a_board_whose_line_does_not_load_still_has_the_users_floor_and_the_problem() {
    let w = world();
    w.user_sets("4");
    w.fixture.write(".collide/board.yaml", "packs: 7\n");
    w.fixture.commit();
    let (gib, problems) = w.floor();
    assert_eq!(gib, 4);
    assert!(!problems.is_empty());
}
