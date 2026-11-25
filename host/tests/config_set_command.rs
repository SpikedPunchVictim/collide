//! `collide config set` (ts-0099-bw): the command line, and the command over a daemon in the test's own process (a
//! fixture checkout, a temp home, a `MemorySecretStore` the test holds). `--secret` reads the value from standard input
//! and refuses one on the command line, whose arguments other processes can read. No test touches the real keychain,
//! home or launchd.

mod common;

use collide_host::config_set_cli::{SetArgs, parse_set, run_set_with};
use collide_host::secret_store::{MemorySecretStore, SecretStore};
use collide_host::task_cli::exit;
use common::config_fixture::Fixture;
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::Arc;

const CANARY: &str = "canary-91b7c4-never-printed";

fn args(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

// ---- reading the command line ----

#[test]
fn a_key_and_a_value_are_read() {
    let parsed = parse_set(&args(&["disk.floor_gib", "7"])).unwrap();
    assert_eq!(
        parsed,
        SetArgs {
            key: "disk.floor_gib".into(),
            value: Some("7".into()),
            secret: false,
            repo: false,
            global: false,
        }
    );
}

#[test]
fn secret_takes_a_key_and_no_value() {
    let parsed = parse_set(&args(&["ferry.notifyHook", "--secret"])).unwrap();
    assert!(parsed.secret);
    assert_eq!(parsed.value, None);
}

#[test]
fn the_options_may_come_first_and_layer_and_global_are_read() {
    let parsed = parse_set(&args(&[
        "--layer",
        "repo",
        "--global",
        "ask.timeout_s",
        "60",
    ]))
    .unwrap();
    assert!(parsed.repo && parsed.global);
    assert_eq!(parsed.value.as_deref(), Some("60"));
    assert!(
        !parse_set(&args(&["--layer", "user", "a", "b"]))
            .unwrap()
            .repo
    );
}

#[test]
fn after_two_dashes_a_value_may_start_with_a_dash() {
    let parsed = parse_set(&args(&["a.b", "--", "-3"])).unwrap();
    assert_eq!(parsed.value.as_deref(), Some("-3"));
}

#[test]
fn a_value_with_secret_is_refused_without_repeating_it() {
    for list in [
        args(&["ferry.notifyHook", CANARY, "--secret"]),
        args(&["--secret", "ferry.notifyHook", CANARY]),
    ] {
        let message = parse_set(&list).unwrap_err();
        assert!(message.contains("standard input"), "{message}");
        assert!(!message.contains(CANARY), "{message}");
    }
}

#[test]
fn a_bad_command_line_is_refused() {
    for bad in [
        args(&[]),
        args(&["a"]),
        args(&["a", "b", "c"]),
        args(&["--secret"]),
        args(&["a", "b", "--json"]),
        args(&["a", "b", "--layer"]),
        args(&["a", "b", "--layer", "profile"]),
        args(&["a", "b", "--layer", "user", "--layer", "repo"]),
        args(&["", "b"]),
        args(&["a", "--secret", "--secret"]),
    ] {
        assert!(parse_set(&bad).is_err(), "{bad:?}");
    }
}

// ---- the command ----

struct Rig {
    fixture: Fixture,
    secret_store: Arc<MemorySecretStore>,
}

fn rig() -> Rig {
    let secret_store = Arc::new(MemorySecretStore::new());
    let held = secret_store.clone();
    Rig {
        fixture: Fixture::with_options(move |options| options.secret_store = held),
        secret_store,
    }
}

impl Rig {
    fn board(&self) {
        self.fixture.ferry();
        self.fixture.commit();
    }

    /// `collide config set <list>` with `stdin`, from `cwd`: the exit code, standard output and standard error.
    fn set_in(&self, cwd: &Path, list: &[&str], stdin: &[u8]) -> (u8, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_set_with(
            self.fixture.daemon.paths(),
            &args(list),
            cwd,
            &mut Cursor::new(stdin.to_vec()),
            &mut out,
            &mut err,
        );
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    fn set(&self, list: &[&str], stdin: &[u8]) -> (u8, String, String) {
        self.set_in(self.fixture.repo.work(), list, stdin)
    }

    fn user_file(&self, rel: &str) -> String {
        fs::read_to_string(self.fixture.home().join(".config/collide").join(rel))
            .unwrap_or_default()
    }
}

#[test]
fn a_secret_from_standard_input_is_kept_in_the_keychain_and_printed_as_set() {
    let rig = rig();
    rig.board();
    let (code, out, err) = rig.set(
        &["ferry.notifyHook", "--secret"],
        format!("{CANARY}\n").as_bytes(),
    );
    assert_eq!(code, exit::OK, "{err}");
    assert_eq!(
        rig.secret_store
            .get("pack.ferry.notifyHook")
            .unwrap()
            .expose(),
        CANARY
    );
    assert_eq!(
        out,
        "ferry.notifyHook = <secret, in the keychain>  (user)\n"
    );
    assert!(!out.contains(CANARY) && !err.contains(CANARY));
}

#[test]
fn one_trailing_newline_is_trimmed_and_no_more() {
    let rig = rig();
    rig.board();
    for (input, kept) in [
        ("v", "v"),
        ("v\n", "v"),
        ("v\r\n", "v"),
        ("v\n\n", "v\n"),
        (" v \n", " v "),
        ("a\nb\n", "a\nb"),
    ] {
        let (code, _, err) = rig.set(&["ferry.notifyHook", "--secret"], input.as_bytes());
        assert_eq!(code, exit::OK, "{input:?}: {err}");
        assert_eq!(
            rig.secret_store
                .get("pack.ferry.notifyHook")
                .unwrap()
                .expose(),
            kept,
            "{input:?}"
        );
    }
}

#[test]
fn a_secret_on_the_command_line_is_refused_with_the_usage_and_nothing_is_kept() {
    let rig = rig();
    rig.board();
    let (code, out, err) = rig.set(&["ferry.notifyHook", CANARY, "--secret"], b"other\n");
    assert_eq!(code, exit::USAGE);
    assert!(out.is_empty());
    assert!(err.contains("usage: collide config set"), "{err}");
    assert!(!err.contains(CANARY), "{err}");
    assert_eq!(
        rig.secret_store.contains("pack.ferry.notifyHook"),
        Ok(false)
    );
}

#[test]
fn an_empty_standard_input_is_refused_and_nothing_is_kept() {
    let rig = rig();
    rig.board();
    for input in [&b""[..], b"\n", b"\r\n"] {
        let (code, _, err) = rig.set(&["ferry.notifyHook", "--secret"], input);
        assert_eq!(code, exit::FAILED, "{input:?}");
        assert!(err.contains("empty"), "{err}");
    }
    assert_eq!(
        rig.secret_store.contains("pack.ferry.notifyHook"),
        Ok(false)
    );
}

#[test]
fn standard_input_that_is_not_text_or_is_far_too_long_is_refused() {
    let rig = rig();
    rig.board();
    let (code, _, err) = rig.set(&["ferry.notifyHook", "--secret"], &[0xff, 0xfe, b'a']);
    assert_eq!(code, exit::FAILED);
    assert!(err.contains("text"), "{err}");
    let (code, _, err) = rig.set(&["ferry.notifyHook", "--secret"], &vec![b'a'; 70_000]);
    assert_eq!(code, exit::FAILED);
    assert!(err.contains("long"), "{err}");
    assert_eq!(
        rig.secret_store.contains("pack.ferry.notifyHook"),
        Ok(false)
    );
}

#[test]
fn a_key_that_is_no_secret_exits_1_naming_it_and_not_the_value() {
    let rig = rig();
    rig.board();
    let (code, _, err) = rig.set(&["disk.floor_gib", "--secret"], CANARY.as_bytes());
    assert_eq!(code, exit::FAILED);
    assert!(
        err.contains("disk.floor_gib") && !err.contains(CANARY),
        "{err}"
    );
}

#[test]
fn a_plain_value_in_a_checkout_goes_to_the_boards_own_file_and_is_printed_with_its_layer() {
    let rig = rig();
    rig.board();
    let (code, out, err) = rig.set(&["ferry.greeting", "hi"], b"");
    assert_eq!(code, exit::OK, "{err}");
    assert_eq!(out, "ferry.greeting = \"hi\"  (user)\n");
    assert_eq!(rig.user_file("config.yaml"), "");
    // The board's name is random (ts-0191-nv), so its folder is the one folder under `boards/`.
    let folders: Vec<_> = fs::read_dir(rig.fixture.home().join(".config/collide/boards"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(folders.len(), 1, "{folders:?}");
    assert!(
        rig.user_file(&format!("boards/{}/config.yaml", folders[0]))
            .contains("greeting: \"hi\"")
    );
}

#[test]
fn global_writes_the_users_own_file_and_needs_no_checkout() {
    let rig = rig();
    let outside = rig.fixture.home().to_path_buf();
    let (code, out, err) = rig.set_in(&outside, &["disk.floor_gib", "7"], b"");
    assert_eq!(code, exit::OK, "{err}");
    assert_eq!(out, "disk.floor_gib = 7  (user)\n");
    assert_eq!(rig.user_file("config.yaml"), "disk:\n  floor_gib: 7\n");
    rig.board();
    let (code, _, err) = rig.set(&["--global", "ask.timeout_s", "60"], b"");
    assert_eq!(code, exit::OK, "{err}");
    assert!(rig.user_file("config.yaml").contains("timeout_s: 60"));
}

#[test]
fn a_repo_layer_is_refused_with_the_way_to_do_it() {
    let rig = rig();
    rig.board();
    let (code, out, err) = rig.set(&["--layer", "repo", "ask.timeout_s", "60"], b"");
    assert_eq!(code, exit::FAILED);
    assert!(out.is_empty());
    assert!(
        err.contains("edit and commit `.collide/config.yaml`"),
        "{err}"
    );
}

#[test]
fn an_invalid_value_exits_1_with_the_reason_and_writes_nothing() {
    let rig = rig();
    let outside = rig.fixture.home().to_path_buf();
    let (code, _, err) = rig.set_in(&outside, &["disk.floor_gib", "0"], b"");
    assert_eq!(code, exit::FAILED);
    assert!(err.contains("disk.floor_gib"), "{err}");
    assert_eq!(rig.user_file("config.yaml"), "");
}

#[test]
fn a_file_with_a_comment_exits_1_and_says_to_edit_it_by_hand() {
    let rig = rig();
    let outside = rig.fixture.home().to_path_buf();
    rig.fixture
        .write_user("config.yaml", "# mine\ndisk: { floor_gib: 3 }\n");
    let (code, _, err) = rig.set_in(&outside, &["ask.timeout_s", "60"], b"");
    assert_eq!(code, exit::FAILED);
    assert!(err.contains("comment") && err.contains("by hand"), "{err}");
    assert_eq!(
        rig.user_file("config.yaml"),
        "# mine\ndisk: { floor_gib: 3 }\n"
    );
}
