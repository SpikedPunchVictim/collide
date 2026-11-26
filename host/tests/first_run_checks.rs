//! The first-run checks and Make it one in the daemon (ts-0192-es; 0.11, First run works with no pack and no tasks;
//! Stage 6a, No-setup open): `board.open` and `board.checks` answer whether `claude` is installed and logged in and
//! whether the folder is a git repo, over the daemon's real socket inside the test process.
//!
//! The `claude` the checks find is a fake script in the test's own `bin` folder, which the test's login shell puts on
//! the captured `PATH` ahead of `/usr/bin:/bin`: the real one is never found, and no test runs it. The fake's JSON is
//! made up and names the account `tester@example.invalid`; every answer and the daemon's log are checked for it. Every
//! path is below the test's root under `/tmp`, which is also the home. Git runs with no configuration of the user's.
//! The daemon's wait on a child is the test's own limit (`claude_check_timeout`), long where a fake must finish and
//! `CUT_AT` where a fake must be cut.

mod common;

use collide_core::owner::{
    BoardChecksParams, BoardChecksResult, BoardListParams, BoardListResult, BoardOpenParams,
    BoardOpenResult, CheckId, FirstRunCheck, code,
};
use collide_host::adapters::claude::auth_status::{AuthStatus, LoginProbe};
use collide_host::base_env::CaptureOptions;
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths};
use collide_host::owner_client::{ClientError, OwnerClient};
use common::{Grandchild, TempRoot, pid_alive, wait_until};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// What a fake must finish within on a machine that starts programs slowly, and what a fake that hangs is cut at.
const LONG: Duration = Duration::from_secs(60);
const CUT_AT: Duration = Duration::from_secs(10);

const INSTALL: &str = "curl -fsSL https://claude.ai/install.sh | bash";
const ACCOUNT: &str = "tester@example.invalid";
const ORGANIZATION: &str = "Example Org";
/// The variable the adapter strips from what the daemon starts (the one that makes a hook believe it runs in Claude).
const MARK: &str = collide_host::adapters::claude::STRIPPED_NAMES[0];

// ---- the fixtures -------------------------------------------------------------------------------------------------

/// The daemon's log, kept so a test can read it.
#[derive(Clone, Default)]
struct LogBuf(Arc<Mutex<Vec<u8>>>);

impl LogBuf {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl Write for LogBuf {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

// What the fake `claude auth status` does, as a fragment of shell.

/// Logged in, by the recorded probe: exit 0 and `"loggedIn": true`, with the account named as the real one names it.
const LOGGED_IN: &str = "echo '{\"loggedIn\": true, \"authMethod\": \"claude.ai\", \
     \"email\": \"tester@example.invalid\", \"orgName\": \"Example Org\"}'\nexit 0";
/// Logged out, by the recorded probe: exit 1 and `"loggedIn": false`.
const LOGGED_OUT: &str = "echo '{\"loggedIn\": false}'\nexit 1";

/// A `claude` that logs each call (its arguments, `HOME` and the stripped variable) to `calls`, answers `--version`,
/// and does `status` for `auth status`. Any other arguments exit 64.
fn fake_claude(root: &TempRoot, bin: &Path, status: &str) {
    let body = format!(
        "#!/bin/sh\necho \"$* HOME=$HOME MARK=${{{mark}-unset}}\" >> '{calls}'\n\
         if [ \"$1\" = \"--version\" ]; then echo '2.1.330 (Claude Code)'; exit 0; fi\n\
         if [ \"$1 $2\" = \"auth status\" ]; then\n{status}\nfi\nexit 64\n",
        calls = root.join("calls").display(),
        mark = MARK,
        status = status
    );
    fs::write(bin.join("claude"), body).unwrap();
    fs::set_permissions(
        bin.join("claude"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
}

/// A fake `claude` whose `auth status` never ends: it starts a sleeper of its own, writes the sleeper's pid to
/// `sleeper`, and waits for it. The sleeper ends by itself after 30 s.
fn hanging_claude(root: &TempRoot, bin: &Path) -> PathBuf {
    let sleeper = root.join("sleeper");
    let status = format!("/bin/sleep 30 &\necho $! > '{}'\nwait", sleeper.display());
    fake_claude(root, bin, &status);
    sleeper
}

/// What git reads: the root of the test as home, and no configuration of the user's or the system's.
fn git_env(root: &TempRoot) -> Vec<(String, String)> {
    vec![
        (
            "HOME".to_string(),
            root.path().to_str().unwrap().to_string(),
        ),
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
    ]
}

/// A daemon whose login shell puts `bin` on the `PATH` and sets a home of the test's root, and a variable of the
/// adapter's stripped names, which the daemon must strip from what it starts. `probe` and `timeout` are the daemon's
/// login probe and its wait on a child.
fn start_with(
    root: &TempRoot,
    bin: &Path,
    probe: Option<Arc<dyn LoginProbe>>,
    timeout: Duration,
    log: &LogBuf,
) -> Daemon {
    let shell = root.script(
        "path-shell",
        &format!(
            "#!/bin/sh\nexport PATH='{}':/usr/bin:/bin\nexport HOME='{}'\nexport {}=1\neval \"$4\"\n",
            bin.display(),
            root.path().display(),
            MARK
        ),
    );
    let mut options = DaemonOptions::new(
        DaemonPaths::under(root.path()),
        shell.to_str().unwrap().to_string(),
        Box::new(log.clone()),
    );
    options.process_tree = common::test_process_tree();
    options.capture = CaptureOptions {
        timeout: Duration::from_secs(10),
        cap_bytes: 1024 * 1024,
        fallback: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    options.home = Some(root.path().to_str().unwrap().to_string());
    options.git_env = git_env(root);
    options.load_context = Some(common::config_fixture::plain_factory(root.path()));
    options.claude_check_timeout = timeout;
    if let Some(probe) = probe {
        options.claude_login = probe;
    }
    Daemon::start(options).expect("the daemon starts")
}

/// A daemon with the real probe (`claude auth status`), which waits up to `timeout` for a fake.
fn start(root: &TempRoot, bin: &Path, timeout: Duration) -> (Daemon, LogBuf) {
    let log = LogBuf::default();
    let daemon = start_with(root, bin, Some(Arc::new(AuthStatus)), timeout, &log);
    (daemon, log)
}

/// An empty `bin` folder below the root.
fn bin(root: &TempRoot) -> PathBuf {
    let bin = root.join("bin");
    fs::create_dir_all(&bin).unwrap();
    bin
}

/// Runs git in `dir` with a cleared environment.
fn git(root: &TempRoot, dir: &Path, args: &[&str]) {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(["-c", "commit.gpgsign=false"])
        .args(args)
        .current_dir(dir)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root.path())
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_SYSTEM", "/dev/null")
        .env("COLLIDE_DAEMON_ROOT", root.path())
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// A checkout named `name` below the root, on `main`, with one commit. Returns its real path.
fn checkout(root: &TempRoot, name: &str) -> PathBuf {
    let dir = root.join(name);
    fs::create_dir_all(dir.join("src")).unwrap();
    git(root, &dir, &["init", "-q", "-b", "main"]);
    fs::write(dir.join("src/lib.rs"), "fn main() {}\n").unwrap();
    git(root, &dir, &["add", "."]);
    git(root, &dir, &["commit", "-q", "-m", "start"]);
    fs::canonicalize(dir).unwrap()
}

/// A folder named `name` below the root that is no checkout. The root itself is under `/tmp`, which is no checkout.
fn plain(root: &TempRoot, name: &str) -> PathBuf {
    let dir = root.join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::canonicalize(dir).unwrap()
}

fn connect(daemon: &Daemon) -> OwnerClient {
    OwnerClient::connect(daemon.paths(), "first-run-test").expect("the client connects")
}

fn call<P: Serialize, R: DeserializeOwned>(
    client: &mut OwnerClient,
    method: &str,
    params: &P,
) -> R {
    client
        .call(method, params, Some(Duration::from_secs(120)))
        .unwrap_or_else(|e| panic!("{method}: {e}"))
}

fn open_with(client: &mut OwnerClient, folder: &Path, init_git: bool) -> BoardOpenResult {
    call(
        client,
        "board.open",
        &BoardOpenParams {
            root: folder.to_str().unwrap().to_string(),
            init_git,
        },
    )
}

fn open(client: &mut OwnerClient, folder: &Path) -> BoardOpenResult {
    open_with(client, folder, false)
}

fn checks(client: &mut OwnerClient, folder: &Path) -> Vec<FirstRunCheck> {
    call::<_, BoardChecksResult>(
        client,
        "board.checks",
        &BoardChecksParams {
            root: folder.to_str().unwrap().to_string(),
        },
    )
    .checks
}

fn boards(client: &mut OwnerClient) -> usize {
    call::<_, BoardListResult>(client, "board.list", &BoardListParams {})
        .boards
        .len()
}

/// The check with `id`, or a panic naming what was answered.
fn check(all: &[FirstRunCheck], id: CheckId) -> &FirstRunCheck {
    all.iter()
        .find(|check| check.id == id)
        .unwrap_or_else(|| panic!("no {id:?} check in {all:?}"))
}

fn ids(all: &[FirstRunCheck]) -> Vec<CheckId> {
    all.iter().map(|check| check.id).collect()
}

/// Nothing of the made-up account is in what was answered.
fn assert_no_account(what: &str, text: &str) {
    assert!(!text.contains(ACCOUNT), "{what} names the account: {text}");
    assert!(
        !text.contains(ORGANIZATION),
        "{what} names the organization: {text}"
    );
}

/// Every path below `dir`, relative to it, but SQLite's side files.
fn paths_below(dir: &Path) -> BTreeSet<String> {
    fn walk(base: &Path, dir: &Path, found: &mut BTreeSet<String>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = path.strip_prefix(base).unwrap().display().to_string();
            // SQLite makes and removes these beside the store as connections open and close; what the store holds is
            // checked through the protocol instead.
            if ["-wal", "-shm", "-journal"]
                .iter()
                .any(|side| name.ends_with(side))
            {
                continue;
            }
            found.insert(name);
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                walk(base, &path, found);
            }
        }
    }
    let mut found = BTreeSet::new();
    walk(dir, dir, &mut found);
    found
}

/// What the fake `claude` was called with, one line per call.
fn calls(root: &TempRoot) -> Vec<String> {
    fs::read_to_string(root.join("calls"))
        .unwrap_or_default()
        .lines()
        .map(str::to_string)
        .collect()
}

// ---- claude installed and logged in -------------------------------------------------------------------------------

#[test]
fn a_missing_claude_is_named_with_its_install_command_and_the_board_opens() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let (daemon, _log) = start(&root, &bin(&root), LONG);
    let mut client = connect(&daemon);

    let opened = open(&mut client, &repo);

    assert!(
        opened.board.is_some(),
        "a missing claude does not stop the board"
    );
    let installed = check(&opened.checks, CheckId::ClaudeInstalled);
    assert!(!installed.ok);
    assert!(
        !installed.required,
        "claude is not required to open a board"
    );
    assert!(installed.message.contains(INSTALL), "{}", installed.message);
    assert_eq!(installed.copy.as_deref(), Some(INSTALL));
    assert!(!installed.could_not_check);
    assert_eq!(
        ids(&opened.checks),
        [CheckId::GitRepo, CheckId::ClaudeInstalled],
        "the login check is left out when claude is missing"
    );
    assert!(check(&opened.checks, CheckId::GitRepo).ok);
}

#[test]
fn a_logged_out_claude_is_named_with_its_fix_and_the_board_opens() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let bin = bin(&root);
    fake_claude(&root, &bin, LOGGED_OUT);
    let (daemon, log) = start(&root, &bin, LONG);
    let mut client = connect(&daemon);

    let opened = open(&mut client, &repo);

    assert!(
        opened.board.is_some(),
        "a logged-out claude does not stop the board"
    );
    assert!(check(&opened.checks, CheckId::ClaudeInstalled).ok);
    let login = check(&opened.checks, CheckId::ClaudeLoggedIn);
    assert!(!login.ok);
    assert!(!login.required);
    assert!(!login.could_not_check);
    assert!(
        login
            .message
            .contains("Run `claude` in a terminal and log in."),
        "{}",
        login.message
    );
    assert_eq!(login.copy.as_deref(), Some("claude"));
    assert_no_account("the answer", &serde_json::to_string(&opened).unwrap());
    assert_no_account("the log", &log.text());
}

#[test]
fn a_logged_in_claude_passes_both_checks_and_the_account_is_never_returned() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let bin = bin(&root);
    fake_claude(&root, &bin, LOGGED_IN);
    let (daemon, log) = start(&root, &bin, LONG);
    let mut client = connect(&daemon);

    let opened = open(&mut client, &repo);

    let installed = check(&opened.checks, CheckId::ClaudeInstalled);
    assert!(installed.ok);
    assert!(
        installed.message.contains("2.1.330"),
        "{}",
        installed.message
    );
    assert_eq!(installed.copy, None);
    let login = check(&opened.checks, CheckId::ClaudeLoggedIn);
    assert!(login.ok);
    assert!(!login.could_not_check);
    assert_eq!(login.copy, None);
    assert_eq!(
        ids(&opened.checks),
        [
            CheckId::GitRepo,
            CheckId::ClaudeInstalled,
            CheckId::ClaudeLoggedIn
        ]
    );
    assert_no_account("board.open", &serde_json::to_string(&opened).unwrap());
    assert_no_account("board.checks", &format!("{:?}", checks(&mut client, &repo)));
    assert_no_account("the log", &log.text());
}

#[test]
fn the_checks_run_only_the_version_and_auth_status_with_the_daemons_own_environment() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let bin = bin(&root);
    fake_claude(&root, &bin, LOGGED_IN);
    let (daemon, _log) = start(&root, &bin, LONG);
    let mut client = connect(&daemon);

    open(&mut client, &repo);
    checks(&mut client, &repo);

    let home = format!("HOME={}", root.path().display());
    let seen = calls(&root);
    assert!(!seen.is_empty());
    for line in &seen {
        // The shell the test gave the daemon sets the stripped variable, which the daemon strips from what it starts.
        assert!(
            line.starts_with("--version ") || line.starts_with("auth status "),
            "an unexpected call: {line}"
        );
        assert!(line.ends_with(&format!("{home} MARK=unset")), "{line}");
    }
    let probes = seen
        .iter()
        .filter(|line| line.starts_with("auth status "))
        .count();
    assert_eq!(probes, 2, "one probe for each of the two answers: {seen:?}");
}

#[test]
fn a_probe_that_hangs_is_cut_with_its_children_and_says_could_not_check() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let bin = bin(&root);
    let sleeper = hanging_claude(&root, &bin);
    let mut grandchild = Grandchild::new(sleeper);
    let (daemon, _log) = start(&root, &bin, CUT_AT);
    let mut client = connect(&daemon);

    // The open waits for the probe on a thread, so the sleeper's pid is read while it runs: its start time is kept, and
    // a reused pid is never signalled.
    let asking = std::thread::spawn(move || open(&mut client, &repo));
    let pid = grandchild.pid(Duration::from_secs(60));
    let opened = asking.join().unwrap();

    assert!(
        opened.board.is_some(),
        "a probe that never answers does not stop the board"
    );
    let login = check(&opened.checks, CheckId::ClaudeLoggedIn);
    assert!(login.ok, "could not check is not a failure");
    assert!(login.could_not_check);
    assert!(!login.required);
    assert!(
        login.message.to_lowercase().contains("could not check"),
        "{}",
        login.message
    );
    wait_until(
        "the probe's sleeper to be gone with it",
        Duration::from_secs(10),
        || !pid_alive(pid),
    );
}

#[test]
fn an_answer_that_does_not_parse_or_disagrees_with_its_exit_code_says_could_not_check() {
    let forms = [
        (
            "not JSON",
            "echo 'logged in, as tester@example.invalid'\nexit 0",
        ),
        ("no output", "exit 0"),
        (
            "no loggedIn field",
            "echo '{\"email\": \"tester@example.invalid\"}'\nexit 0",
        ),
        (
            "logged out by its field, logged in by its exit code",
            "echo '{\"loggedIn\": false, \"email\": \"tester@example.invalid\"}'\nexit 0",
        ),
        (
            "logged in by its field, logged out by its exit code",
            "echo '{\"loggedIn\": true, \"email\": \"tester@example.invalid\"}'\nexit 1",
        ),
        (
            "an exit code that is neither",
            "echo '{\"loggedIn\": true}'\nexit 3",
        ),
    ];
    for (form, status) in forms {
        let root = TempRoot::new();
        let repo = checkout(&root, "repo");
        let bin = bin(&root);
        fake_claude(&root, &bin, status);
        let (daemon, log) = start(&root, &bin, LONG);
        let mut client = connect(&daemon);

        let opened = open(&mut client, &repo);

        let login = check(&opened.checks, CheckId::ClaudeLoggedIn);
        assert!(login.ok && login.could_not_check, "{form}: {login:?}");
        assert!(!login.required, "{form}");
        assert_no_account(form, &serde_json::to_string(&opened).unwrap());
        assert_no_account(form, &log.text());
        assert!(opened.board.is_some(), "{form}");
    }
}

#[test]
fn a_daemon_with_no_probe_recorded_says_could_not_check_and_runs_no_status_command() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let bin = bin(&root);
    fake_claude(&root, &bin, LOGGED_OUT);
    let log = LogBuf::default();
    // `DaemonOptions::new` gives the probe that never runs `claude`: a test daemon is not one that asks.
    let daemon = start_with(&root, &bin, None, LONG, &log);
    let mut client = connect(&daemon);

    let opened = open(&mut client, &repo);

    let login = check(&opened.checks, CheckId::ClaudeLoggedIn);
    assert!(login.ok && login.could_not_check, "{login:?}");
    assert!(
        login.message.to_lowercase().contains("could not check"),
        "{}",
        login.message
    );
    assert!(opened.board.is_some());
    assert!(
        calls(&root).iter().all(|line| !line.starts_with("auth")),
        "{:?}",
        calls(&root)
    );
}

#[test]
fn a_claude_that_prints_no_version_is_still_installed_and_says_so() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let bin = bin(&root);
    fs::write(
        bin.join("claude"),
        "#!/bin/sh\necho 'no version here'\nexit 0\n",
    )
    .unwrap();
    fs::set_permissions(
        bin.join("claude"),
        std::os::unix::fs::PermissionsExt::from_mode(0o755),
    )
    .unwrap();
    let (daemon, _log) = start(&root, &bin, LONG);
    let mut client = connect(&daemon);

    let opened = open(&mut client, &repo);

    let installed = check(&opened.checks, CheckId::ClaudeInstalled);
    assert!(installed.ok, "{installed:?}");
    assert!(
        installed.message.contains("version"),
        "{}",
        installed.message
    );
    assert!(opened.board.is_some());
}

// ---- a git repo and Make it one -----------------------------------------------------------------------------------

#[test]
fn a_folder_that_is_not_a_repo_answers_no_board_and_a_failed_required_git_repo_with_the_claude_checks()
 {
    let root = TempRoot::new();
    let folder = plain(&root, "plain");
    let bin = bin(&root);
    fake_claude(&root, &bin, LOGGED_OUT);
    let (daemon, _log) = start(&root, &bin, LONG);
    let mut client = connect(&daemon);

    let opened = open(&mut client, &folder);

    assert!(opened.board.is_none());
    let git_repo = check(&opened.checks, CheckId::GitRepo);
    assert!(!git_repo.ok);
    assert!(git_repo.required);
    assert!(
        git_repo.message.contains("not inside a git checkout"),
        "{}",
        git_repo.message
    );
    assert_eq!(
        opened.checks[0].id,
        CheckId::GitRepo,
        "git comes first, as the page lists it"
    );
    assert!(
        !check(&opened.checks, CheckId::ClaudeLoggedIn).ok,
        "the page shows every check"
    );
    assert!(
        !folder.join(".git").exists(),
        "nothing is made without init_git"
    );
    assert_eq!(boards(&mut client), 0);
}

#[test]
fn init_git_on_a_folder_that_is_not_a_repo_makes_it_one_and_opens_it() {
    let root = TempRoot::new();
    let folder = plain(&root, "plain");
    let (daemon, _log) = start(&root, &bin(&root), LONG);
    let mut client = connect(&daemon);

    let opened = open_with(&mut client, &folder, true);

    let board = opened.board.expect("the new repository opens");
    assert_eq!(board.root, folder.to_str().unwrap());
    assert!(folder.join(".git").is_dir());
    let git_repo = check(&opened.checks, CheckId::GitRepo);
    assert!(git_repo.ok, "{git_repo:?}");
    assert_eq!(boards(&mut client), 1);
    let again = open(&mut client, &folder);
    assert_eq!(
        again.board.unwrap().id,
        board.id,
        "it is the same board next time"
    );
}

#[test]
fn init_git_on_a_folder_in_a_checkout_does_nothing_more() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let head = fs::read(repo.join(".git/HEAD")).unwrap();
    let before = paths_below(&repo.join(".git"));
    let (daemon, _log) = start(&root, &bin(&root), LONG);
    let mut client = connect(&daemon);

    let opened = open_with(&mut client, &repo.join("src"), true);

    let board = opened.board.expect("the checkout opens");
    assert_eq!(
        board.root,
        repo.to_str().unwrap(),
        "the board is the checkout's, not the subfolder's"
    );
    assert!(
        !repo.join("src/.git").exists(),
        "no repository is made inside a checkout"
    );
    assert_eq!(fs::read(repo.join(".git/HEAD")).unwrap(), head);
    assert_eq!(
        paths_below(&repo.join(".git")),
        before,
        "the checkout's git directory is untouched"
    );
}

#[test]
fn init_git_that_git_refuses_is_an_error_naming_git_init_and_opens_nothing() {
    use std::os::unix::fs::PermissionsExt;
    let root = TempRoot::new();
    let folder = plain(&root, "locked");
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o500)).unwrap();
    let (daemon, _log) = start(&root, &bin(&root), LONG);
    let mut client = connect(&daemon);

    let answer: Result<Value, ClientError> = client.call(
        "board.open",
        &BoardOpenParams {
            root: folder.to_str().unwrap().to_string(),
            init_git: true,
        },
        Some(Duration::from_secs(60)),
    );

    // The test's root makes the folder writable again when it is dropped.
    match answer {
        Err(ClientError::Rpc(error)) => {
            assert_eq!(error.code, code::INTERNAL_ERROR);
            assert!(
                error.message.contains("`git init` failed"),
                "{}",
                error.message
            );
        }
        other => panic!("git init in a folder it cannot write should fail, and gave {other:?}"),
    }
    assert_eq!(boards(&mut client), 0);
}

#[test]
fn init_git_on_a_folder_that_is_gone_makes_nothing() {
    let root = TempRoot::new();
    let gone = root.join("not-there");
    let (daemon, _log) = start(&root, &bin(&root), LONG);
    let mut client = connect(&daemon);

    let opened = open_with(&mut client, &gone, true);

    assert!(opened.board.is_none());
    let git_repo = check(&opened.checks, CheckId::GitRepo);
    assert!(!git_repo.ok && git_repo.required, "{git_repo:?}");
    assert!(!gone.exists(), "the folder was not made");
    assert_eq!(boards(&mut client), 0);
}

// ---- board.checks -------------------------------------------------------------------------------------------------

#[test]
fn board_checks_opens_nothing_and_writes_nothing() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let folder = plain(&root, "plain");
    let bin = bin(&root);
    fake_claude(&root, &bin, LOGGED_IN);
    let (daemon, _log) = start(&root, &bin, LONG);
    let mut client = connect(&daemon);
    // The first call settles what the daemon itself makes on first use, so the next ones are compared with it.
    checks(&mut client, &repo);
    let before = paths_below(root.path());

    let for_repo = checks(&mut client, &repo);
    let for_plain = checks(&mut client, &folder);

    assert!(check(&for_repo, CheckId::GitRepo).ok);
    assert!(!check(&for_plain, CheckId::GitRepo).ok);
    assert_eq!(boards(&mut client), 0, "no store row");
    assert_eq!(
        paths_below(root.path()),
        before,
        "no folder, file or trust entry"
    );
    assert!(!folder.join(".git").exists());
    // Opening the same folder does record a board: the count above can tell.
    open(&mut client, &repo);
    assert_eq!(boards(&mut client), 1);
}

#[test]
fn board_checks_and_board_open_answer_the_same_checks() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let folder = plain(&root, "plain");
    let bin = bin(&root);
    fake_claude(&root, &bin, LOGGED_OUT);
    let (daemon, _log) = start(&root, &bin, LONG);
    let mut client = connect(&daemon);

    for folder in [&folder, &repo] {
        let asked = checks(&mut client, folder);
        let opened = open(&mut client, folder);
        assert_eq!(asked, opened.checks, "{}", folder.display());
    }
}

#[test]
fn board_checks_on_a_folder_inside_a_checkout_answers_the_checkouts_git_repo() {
    let root = TempRoot::new();
    let repo = checkout(&root, "repo");
    let (daemon, _log) = start(&root, &bin(&root), LONG);
    let mut client = connect(&daemon);

    let asked = checks(&mut client, &repo.join("src"));

    let git_repo = check(&asked, CheckId::GitRepo);
    assert!(git_repo.ok);
    assert!(
        git_repo.message.contains(repo.to_str().unwrap()),
        "{}",
        git_repo.message
    );
}

#[test]
fn board_checks_refuses_a_root_that_is_not_an_absolute_path() {
    let root = TempRoot::new();
    let (daemon, _log) = start(&root, &bin(&root), LONG);
    let mut client = connect(&daemon);

    let answer: Result<Value, ClientError> = client.call(
        "board.checks",
        &BoardChecksParams {
            root: "relative/folder".to_string(),
        },
        Some(Duration::from_secs(60)),
    );

    match answer {
        Err(ClientError::Rpc(error)) => {
            assert_eq!(error.code, code::INVALID_PARAMS);
            assert!(error.message.contains("absolute"), "{}", error.message);
        }
        other => panic!("a relative root should be refused, and gave {other:?}"),
    }
}
