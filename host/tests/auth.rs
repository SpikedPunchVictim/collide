//! The daemon's connection rules (0.2, The socket and Who is asking; 0.10, the person-only operations), over fakes:
//! the peer, the process tree and the file metadata are all traits, so no test touches a real socket or a real
//! process. The few tests that do touch a file make it in a folder under `target/tmp` and remove it.

mod common;

use collide_core::change::ActorKind;
use collide_core::owner::{OwnerInitializeParams, code};
use collide_host::auth::{
    self, Caller, DirFs, DirStat, PeerCredentials, PersonOnlyItem, PersonOnlyOp, PersonOnlyRequest,
    ProcessTree, Verdict,
};
use collide_host::daemon::{DaemonPaths, MAX_SOCKET_PATH_BYTES, SocketError};
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::io;
use std::path::{Path, PathBuf};

const DAEMON_UID: u32 = 501;

// ---- fakes ----------------------------------------------------------------------------------------------------

#[derive(Default)]
struct FakeTree {
    procs: HashMap<u32, FakeProc>,
}

#[derive(Default, Clone)]
struct FakeProc {
    parent: Option<u32>,
    collide_run: Option<String>,
    is_app: bool,
}

impl FakeTree {
    fn with(mut self, pid: u32, parent: Option<u32>) -> Self {
        self.procs.entry(pid).or_default().parent = parent;
        self
    }

    fn run(mut self, pid: u32, run: &str) -> Self {
        self.procs.entry(pid).or_default().collide_run = Some(run.to_string());
        self
    }

    fn app(mut self, pid: u32) -> Self {
        self.procs.entry(pid).or_default().is_app = true;
        self
    }
}

impl ProcessTree for FakeTree {
    fn parent(&self, pid: u32) -> Option<u32> {
        self.procs.get(&pid).and_then(|p| p.parent)
    }

    fn run_marker(&self, pid: u32) -> Option<String> {
        self.procs.get(&pid).and_then(|p| p.collide_run.clone())
    }

    fn is_app(&self, pid: u32) -> bool {
        self.procs.get(&pid).is_some_and(|p| p.is_app)
    }
}

/// A peer that counts how often its pid is asked for, so a test can tell the pid was never read.
struct FakePeer {
    uid: io::Result<u32>,
    pid: u32,
    pid_reads: Cell<u32>,
}

impl FakePeer {
    fn new(uid: u32, pid: u32) -> Self {
        FakePeer {
            uid: Ok(uid),
            pid,
            pid_reads: Cell::new(0),
        }
    }
}

impl PeerCredentials for FakePeer {
    fn uid(&self) -> io::Result<u32> {
        match &self.uid {
            Ok(uid) => Ok(*uid),
            Err(e) => Err(io::Error::new(e.kind(), e.to_string())),
        }
    }

    fn pid(&self) -> io::Result<u32> {
        self.pid_reads.set(self.pid_reads.get() + 1);
        Ok(self.pid)
    }
}

/// Directories by path, and the mode a new directory gets (a fake umask: `0o700` unless a test says otherwise).
struct FakeFs {
    dirs: RefCell<HashMap<PathBuf, DirStat>>,
    new_dir_mode: u32,
    created: RefCell<Vec<PathBuf>>,
}

impl FakeFs {
    fn new() -> Self {
        FakeFs {
            dirs: RefCell::new(HashMap::new()),
            new_dir_mode: 0o700,
            created: RefCell::new(Vec::new()),
        }
    }

    fn dir(self, path: &str, uid: u32, mode: u32) -> Self {
        self.dirs.borrow_mut().insert(
            PathBuf::from(path),
            DirStat {
                is_dir: true,
                uid,
                mode,
            },
        );
        self
    }

    fn file(self, path: &str) -> Self {
        self.dirs.borrow_mut().insert(
            PathBuf::from(path),
            DirStat {
                is_dir: false,
                uid: DAEMON_UID,
                mode: 0o600,
            },
        );
        self
    }
}

impl DirFs for FakeFs {
    fn stat(&self, path: &Path) -> io::Result<Option<DirStat>> {
        Ok(self.dirs.borrow().get(path).copied())
    }

    fn create_private_dir(&self, path: &Path) -> io::Result<()> {
        self.created.borrow_mut().push(path.to_path_buf());
        self.dirs.borrow_mut().insert(
            path.to_path_buf(),
            DirStat {
                is_dir: true,
                uid: DAEMON_UID,
                mode: self.new_dir_mode,
            },
        );
        Ok(())
    }
}

fn message(error: &SocketError) -> String {
    error.to_string()
}

// ---- who is asking --------------------------------------------------------------------------------------------

/// pids: 1 launchd, 100 the app, 200 a terminal, 300 the daemon's run agent (claude), 400 a python3 child of it.
fn the_machine() -> FakeTree {
    FakeTree::default()
        .with(1, None)
        .with(100, Some(1))
        .app(100)
        .with(101, Some(100))
        .with(102, Some(101))
        .with(200, Some(1))
        .with(201, Some(200))
        .with(300, Some(1))
        .run(300, "r-0007")
        .with(310, Some(300))
        .with(400, Some(310))
}

#[test]
fn the_actor_classification_table_over_a_fake_process_tree() {
    let table: Vec<(&str, FakeTree, u32, Caller)> = vec![
        (
            "the peer itself holds COLLIDE_RUN",
            the_machine(),
            300,
            Caller::RunAgent {
                run: "r-0007".into(),
            },
        ),
        (
            "a python3 child of a run's agent, two parents below the marker",
            the_machine(),
            400,
            Caller::RunAgent {
                run: "r-0007".into(),
            },
        ),
        (
            "a process under the app is the app",
            the_machine(),
            102,
            Caller::App,
        ),
        (
            "the app's own process is the app",
            the_machine(),
            100,
            Caller::App,
        ),
        (
            "a process in a terminal is the cli",
            the_machine(),
            201,
            Caller::Cli,
        ),
        (
            "a run's marker wins over the app above it",
            the_machine().with(300, Some(100)).run(300, "r-0007"),
            400,
            Caller::RunAgent {
                run: "r-0007".into(),
            },
        ),
        (
            "the nearest marker names the run",
            the_machine().with(310, Some(300)).run(310, "r-0008"),
            400,
            Caller::RunAgent {
                run: "r-0008".into(),
            },
        ),
        (
            "an empty marker is still a marker",
            the_machine().run(310, ""),
            400,
            Caller::RunAgent { run: "".into() },
        ),
        (
            "a pid the table does not know is the cli",
            the_machine(),
            999,
            Caller::Cli,
        ),
        (
            "a tree that loops back on itself ends, as the cli",
            FakeTree::default().with(10, Some(11)).with(11, Some(10)),
            10,
            Caller::Cli,
        ),
    ];
    for (name, tree, pid, expected) in table {
        assert_eq!(auth::classify_peer(&tree, pid), expected, "{name}");
    }
}

#[test]
fn a_marker_deeper_than_the_walk_limit_is_not_found() {
    let mut tree = FakeTree::default();
    for pid in 1..=(auth::MAX_TREE_DEPTH as u32 + 5) {
        tree = tree.with(pid, Some(pid + 1));
    }
    let top = auth::MAX_TREE_DEPTH as u32 + 6;
    tree = tree.with(top, None).run(top, "r-far");
    assert_eq!(auth::classify_peer(&tree, 1), Caller::Cli);
}

#[test]
fn the_actor_a_caller_is_recorded_as() {
    let person = Caller::App.actor();
    assert_eq!(person.kind, ActorKind::Person);
    assert_eq!(Caller::Cli.actor().kind, ActorKind::Person);
    let agent = Caller::RunAgent { run: "r-1".into() }.actor();
    assert_eq!(agent.kind, ActorKind::Agent);
    assert_eq!(agent.run.as_deref(), Some("r-1"));
    assert_eq!(agent.phase, None);
}

// ---- person-only operations -----------------------------------------------------------------------------------

fn request(op: PersonOnlyOp, kind: PersonOnlyItem) -> PersonOnlyRequest {
    PersonOnlyRequest {
        op,
        item_kind: kind,
        item: "pr-0003-km".into(),
    }
}

const ALL_OPS: [PersonOnlyOp; 4] = [
    PersonOnlyOp::Accept,
    PersonOnlyOp::Defer,
    PersonOnlyOp::Reject,
    PersonOnlyOp::Resolve,
];
const ALL_KINDS: [PersonOnlyItem; 2] = [PersonOnlyItem::Proposal, PersonOnlyItem::Question];

#[test]
fn the_person_only_table_per_actor() {
    let agent = Caller::RunAgent {
        run: "r-0007".into(),
    };
    for op in ALL_OPS {
        for kind in ALL_KINDS {
            let req = request(op, kind);
            let label = format!("{op:?} of a {kind:?}");
            assert!(
                matches!(auth::judge(&Caller::App, &req), Verdict::Allow),
                "app: {label}"
            );
            match auth::judge(&Caller::Cli, &req) {
                Verdict::Confirm(confirmation) => {
                    assert_eq!(confirmation.request, req, "cli: {label}");
                    assert!(
                        confirmation.title.contains("pr-0003-km"),
                        "{}",
                        confirmation.title
                    );
                }
                other => panic!("cli: {label} should be a confirmation, was {other:?}"),
            }
            match auth::judge(&agent, &req) {
                Verdict::Refuse(error) => {
                    assert_eq!(error.code, code::NOT_PERMITTED, "run agent: {label}");
                    assert!(
                        error.message.contains("r-0007") && error.message.contains("pr-0003-km"),
                        "{}",
                        error.message
                    );
                }
                other => panic!("run agent: {label} should be refused, was {other:?}"),
            }
        }
    }
}

#[test]
fn which_requests_are_person_only() {
    use PersonOnlyItem::{Proposal, Question};
    use PersonOnlyOp::{Accept, Defer, Reject, Resolve};
    type Row = (
        &'static str,
        &'static str,
        Option<&'static str>,
        Option<(PersonOnlyOp, PersonOnlyItem)>,
    );
    let table: Vec<Row> = vec![
        ("accept", "proposal", None, Some((Accept, Proposal))),
        ("defer", "proposal", None, Some((Defer, Proposal))),
        ("reject", "proposal", None, Some((Reject, Proposal))),
        ("resolve", "question", None, Some((Resolve, Question))),
        ("resolve", "proposal", None, Some((Resolve, Proposal))),
        ("accept", "question", None, Some((Accept, Question))),
        (
            "setStatus",
            "proposal",
            Some("accepted"),
            Some((Accept, Proposal)),
        ),
        (
            "setStatus",
            "proposal",
            Some("deferred"),
            Some((Defer, Proposal)),
        ),
        (
            "setStatus",
            "proposal",
            Some("rejected"),
            Some((Reject, Proposal)),
        ),
        ("setStatus", "proposal", Some("proposed"), None),
        ("setStatus", "proposal", None, None),
        ("setStatus", "task", Some("accepted"), None),
        ("accept", "task", None, None),
        ("resolve", "spike", None, None),
        ("record", "proposal", None, None),
        ("Accept", "proposal", None, None),
    ];
    for (op, kind, status, expected) in table {
        let got = auth::person_only_request(op, kind, "pr-1", status).map(|r| (r.op, r.item_kind));
        assert_eq!(got, expected, "{op} on a {kind} with status {status:?}");
    }
}

// ---- the socket directory and path ----------------------------------------------------------------------------

#[test]
fn a_directory_of_mode_0700_owned_by_the_daemon_is_accepted() {
    let fs = FakeFs::new().dir("/run/collide", DAEMON_UID, 0o700);
    assert!(auth::check_socket_dir(&fs, DAEMON_UID, Path::new("/run/collide")).is_ok());
    assert!(fs.created.borrow().is_empty());
}

#[test]
fn a_directory_of_mode_0750_is_refused_and_named() {
    let fs = FakeFs::new().dir("/run/collide", DAEMON_UID, 0o750);
    let error = auth::check_socket_dir(&fs, DAEMON_UID, Path::new("/run/collide")).unwrap_err();
    let text = message(&error);
    assert!(text.contains("/run/collide"), "{text}");
    assert!(text.contains("0750"), "{text}");
    assert!(text.contains("0700"), "{text}");
}

#[test]
fn a_directory_of_a_foreign_owner_is_refused_with_both_uids() {
    let fs = FakeFs::new().dir("/run/collide", 0, 0o700);
    let error = auth::check_socket_dir(&fs, DAEMON_UID, Path::new("/run/collide")).unwrap_err();
    let text = message(&error);
    assert!(text.contains("/run/collide"), "{text}");
    assert!(text.contains("uid 0"), "{text}");
    assert!(text.contains("501"), "{text}");
}

#[test]
fn a_directory_that_is_too_open_and_foreign_names_both_faults() {
    let fs = FakeFs::new().dir("/run/collide", 0, 0o777);
    let text =
        message(&auth::check_socket_dir(&fs, DAEMON_UID, Path::new("/run/collide")).unwrap_err());
    assert!(text.contains("0777") && text.contains("uid 0"), "{text}");
}

#[test]
fn a_mode_0500_directory_is_refused_too_because_the_mode_must_be_exactly_0700() {
    let fs = FakeFs::new().dir("/run/collide", DAEMON_UID, 0o500);
    assert!(auth::check_socket_dir(&fs, DAEMON_UID, Path::new("/run/collide")).is_err());
}

#[test]
fn a_missing_directory_is_made_private() {
    let fs = FakeFs::new();
    assert!(auth::check_socket_dir(&fs, DAEMON_UID, Path::new("/run/collide")).is_ok());
    assert_eq!(*fs.created.borrow(), vec![PathBuf::from("/run/collide")]);
}

#[test]
fn a_directory_made_with_a_mode_the_umask_loosened_is_refused_after_making_it() {
    let mut fs = FakeFs::new();
    fs.new_dir_mode = 0o755;
    let text =
        message(&auth::check_socket_dir(&fs, DAEMON_UID, Path::new("/run/collide")).unwrap_err());
    assert!(
        text.contains("/run/collide") && text.contains("0755"),
        "{text}"
    );
}

#[test]
fn a_path_that_is_a_file_and_not_a_directory_is_refused_and_named() {
    let fs = FakeFs::new().file("/run/collide");
    let text =
        message(&auth::check_socket_dir(&fs, DAEMON_UID, Path::new("/run/collide")).unwrap_err());
    assert!(
        text.contains("/run/collide") && text.contains("not a directory"),
        "{text}"
    );
}

#[test]
fn a_socket_path_over_103_bytes_fails_naming_the_path_and_the_limit() {
    let long = PathBuf::from(format!("/{}", "a".repeat(MAX_SOCKET_PATH_BYTES)));
    assert_eq!(long.as_os_str().len(), MAX_SOCKET_PATH_BYTES + 1);
    let text = message(&auth::check_socket_path(&long).unwrap_err());
    assert!(text.contains(long.to_str().unwrap()), "{text}");
    assert!(text.contains("104"), "{text}");
    assert!(text.contains("103"), "{text}");
}

#[test]
fn a_socket_path_of_exactly_103_bytes_is_accepted() {
    let exact = PathBuf::from(format!("/{}", "a".repeat(MAX_SOCKET_PATH_BYTES - 1)));
    assert_eq!(exact.as_os_str().len(), MAX_SOCKET_PATH_BYTES);
    assert!(auth::check_socket_path(&exact).is_ok());
}

#[test]
fn the_path_is_checked_before_the_directory_is_touched() {
    let long = PathBuf::from(format!("/run/collide/{}", "a".repeat(120)));
    let paths = DaemonPaths {
        socket_dir: PathBuf::from("/run/collide"),
        socket: long,
        token_file: PathBuf::from("/run/collide/token"),
        lock_file: PathBuf::from("/run/collide/lock"),
        runs_dir: PathBuf::from("/run/collide/runs"),
        store_dir: PathBuf::from("/run/collide/store"),
    };
    let fs = FakeFs::new();
    let error = auth::prepare_daemon_socket(&fs, DAEMON_UID, &paths).unwrap_err();
    assert!(matches!(error, SocketError::PathTooLong { .. }), "{error}");
    assert!(fs.created.borrow().is_empty());
}

#[test]
fn the_daemons_socket_is_prepared_when_path_and_directory_are_right() {
    let paths = DaemonPaths {
        socket_dir: PathBuf::from("/run/collide"),
        socket: PathBuf::from("/run/collide/daemon.sock"),
        token_file: PathBuf::from("/run/collide/token"),
        lock_file: PathBuf::from("/run/collide/lock"),
        runs_dir: PathBuf::from("/run/collide/runs"),
        store_dir: PathBuf::from("/run/collide/store"),
    };
    let fs = FakeFs::new().dir("/run/collide", DAEMON_UID, 0o700);
    assert!(auth::prepare_daemon_socket(&fs, DAEMON_UID, &paths).is_ok());
}

// ---- the uid check, and the token --------------------------------------------------------------------------------

fn params(token: Option<&str>) -> OwnerInitializeParams {
    OwnerInitializeParams {
        protocol: collide_core::owner::OWNER_PROTOCOL,
        token: token.map(str::to_string),
        client: None,
    }
}

#[test]
fn a_connection_from_another_uid_is_refused_before_its_token_is_read() {
    let peer = FakePeer::new(502, 201);
    let tree = the_machine();
    let error = auth::admit(DAEMON_UID, &peer, &tree).unwrap_err();
    assert!(
        matches!(error, SocketError::ForeignUid { uid: 502, .. }),
        "{error}"
    );
    let text = message(&error);
    assert!(text.contains("502") && text.contains("501"), "{text}");
    assert_eq!(
        peer.pid_reads.get(),
        0,
        "the tree is not walked for a foreign uid"
    );
}

#[test]
fn the_token_reader_is_never_called_for_a_connection_that_was_not_admitted() {
    let token_read = Cell::new(false);
    let peer = FakePeer::new(502, 201);
    let admitted = auth::admit(DAEMON_UID, &peer, &the_machine());
    // `initialize` is a method of the admitted connection alone, so a refused one has nothing to call it on.
    if let Ok(admitted) = admitted {
        let _ = admitted.initialize(
            || {
                token_read.set(true);
                params(Some("secret"))
            },
            "secret",
        );
    }
    assert!(!token_read.get());
}

#[test]
fn a_peer_whose_uid_cannot_be_read_is_refused_naming_what_failed() {
    let peer = FakePeer {
        uid: Err(io::Error::other("getpeereid failed")),
        pid: 1,
        pid_reads: Cell::new(0),
    };
    let text = message(&auth::admit(DAEMON_UID, &peer, &the_machine()).unwrap_err());
    assert!(
        text.contains("uid") && text.contains("getpeereid failed"),
        "{text}"
    );
}

#[test]
fn a_same_uid_peer_with_the_right_token_gets_the_actor_of_its_tree() {
    let peer = FakePeer::new(DAEMON_UID, 102);
    let admitted = auth::admit(DAEMON_UID, &peer, &the_machine()).unwrap();
    assert_eq!(admitted.caller(), &Caller::App);
    let session = admitted
        .initialize(|| params(Some("secret")), "secret")
        .unwrap();
    assert_eq!(session.caller(), &Caller::App);
    assert_eq!(session.actor().kind, ActorKind::Person);
}

#[test]
fn a_same_uid_peer_with_a_wrong_or_missing_token_is_refused_with_the_token_code() {
    for given in [Some("wrong"), None] {
        let peer = FakePeer::new(DAEMON_UID, 201);
        let admitted = auth::admit(DAEMON_UID, &peer, &the_machine()).unwrap();
        let error = admitted.initialize(|| params(given), "secret").unwrap_err();
        assert_eq!(error.code, code::TOKEN_REFUSED, "{given:?}");
        assert!(!error.message.contains("secret"), "{}", error.message);
    }
}

#[test]
fn a_run_agent_with_the_token_is_still_a_run_agent() {
    let peer = FakePeer::new(DAEMON_UID, 400);
    let session = auth::admit(DAEMON_UID, &peer, &the_machine())
        .unwrap()
        .initialize(|| params(Some("secret")), "secret")
        .unwrap();
    assert_eq!(
        session.caller(),
        &Caller::RunAgent {
            run: "r-0007".into()
        }
    );
    let verdict = session.judge(&request(PersonOnlyOp::Accept, PersonOnlyItem::Proposal));
    assert!(matches!(verdict, Verdict::Refuse(_)), "{verdict:?}");
}

// ---- a shim's socket ----------------------------------------------------------------------------------------------

#[test]
fn a_shims_socket_is_under_its_run_directory_in_a_private_directory() {
    let run_dir = Path::new("/run/collide/runs/r-0007");
    let fs = FakeFs::new();
    let socket = auth::prepare_shim_socket(&fs, DAEMON_UID, run_dir).unwrap();
    assert_eq!(
        socket,
        run_dir
            .join(collide_host::shim::files::SOCKET_DIR)
            .join(collide_host::shim::files::SOCKET)
    );
    assert_eq!(
        *fs.created.borrow(),
        vec![run_dir.join(collide_host::shim::files::SOCKET_DIR)]
    );
}

#[test]
fn a_shims_directory_follows_the_same_mode_and_owner_rule() {
    let run_dir = Path::new("/run/collide/runs/r-0007");
    let shim_dir = run_dir.join(collide_host::shim::files::SOCKET_DIR);
    let open = FakeFs::new().dir(shim_dir.to_str().unwrap(), DAEMON_UID, 0o750);
    assert!(auth::prepare_shim_socket(&open, DAEMON_UID, run_dir).is_err());
    let foreign = FakeFs::new().dir(shim_dir.to_str().unwrap(), 0, 0o700);
    assert!(auth::prepare_shim_socket(&foreign, DAEMON_UID, run_dir).is_err());
}

#[test]
fn a_shims_socket_path_has_the_same_103_byte_limit() {
    let run_dir = PathBuf::from(format!("/run/{}", "r".repeat(100)));
    let fs = FakeFs::new();
    let error = auth::prepare_shim_socket(&fs, DAEMON_UID, &run_dir).unwrap_err();
    assert!(matches!(error, SocketError::PathTooLong { .. }), "{error}");
}

#[test]
fn a_shims_peer_is_checked_for_uid_and_nothing_else() {
    let same = FakePeer::new(DAEMON_UID, 1);
    assert!(auth::admit_shim(DAEMON_UID, &same).is_ok());
    assert_eq!(same.pid_reads.get(), 0, "a shim's peer is not classified");
    let foreign = FakePeer::new(502, 1);
    assert!(matches!(
        auth::admit_shim(DAEMON_UID, &foreign).unwrap_err(),
        SocketError::ForeignUid { uid: 502, .. }
    ));
}

// ---- the real files: std only, in a folder under target/tmp ---------------------------------------------------------

mod real_files {
    use super::*;
    use std::fs;
    use std::os::unix::fs::{MetadataExt, PermissionsExt};

    fn fresh(name: &str) -> PathBuf {
        let dir = common::scratch(name);
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// The uid that owns a file this process just made: the daemon's own, without a system call.
    fn own_uid(dir: &Path) -> u32 {
        fs::metadata(dir).unwrap().uid()
    }

    #[test]
    fn the_token_file_is_created_with_mode_0600() {
        let dir = fresh("auth-token-mode");
        let path = dir.join("token");
        auth::create_token_file(&path, "abc123").unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o7777;
        assert_eq!(mode, 0o600, "mode {mode:o}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "abc123");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn the_token_file_is_never_replaced_once_made() {
        let dir = fresh("auth-token-twice");
        let path = dir.join("token");
        auth::create_token_file(&path, "first").unwrap();
        let text = message(&auth::create_token_file(&path, "second").unwrap_err());
        assert!(text.contains(path.to_str().unwrap()), "{text}");
        assert_eq!(fs::read_to_string(&path).unwrap(), "first");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_token_file_is_read_back_when_its_mode_is_0600() {
        let dir = fresh("auth-token-read");
        let path = dir.join("token");
        auth::create_token_file(&path, "abc123").unwrap();
        assert_eq!(auth::read_token_file(&path).unwrap(), "abc123");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_token_file_open_to_others_is_refused_unread_and_named() {
        let dir = fresh("auth-token-open");
        let path = dir.join("token");
        auth::create_token_file(&path, "abc123").unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
        let text = message(&auth::read_token_file(&path).unwrap_err());
        assert!(
            text.contains(path.to_str().unwrap()) && text.contains("0644"),
            "{text}"
        );
        assert!(!text.contains("abc123"), "{text}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_minted_token_is_64_hex_characters_and_differs_each_time() {
        let a = auth::mint_token().unwrap();
        let b = auth::mint_token().unwrap();
        assert_eq!(a.len(), 64);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()), "{a}");
        assert_ne!(a, b);
    }

    #[test]
    fn the_real_socket_directory_is_made_0700_and_then_accepted() {
        let root = fresh("auth-dir-real");
        let uid = own_uid(&root);
        let dir = root.join("sock");
        auth::check_socket_dir(&auth::StdFs, uid, &dir).unwrap();
        let mode = fs::metadata(&dir).unwrap().permissions().mode() & 0o7777;
        assert_eq!(mode, 0o700, "mode {mode:o}");
        auth::check_socket_dir(&auth::StdFs, uid, &dir).unwrap();
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn the_real_directory_of_mode_0750_is_refused_and_a_foreign_uid_too() {
        let root = fresh("auth-dir-open");
        let uid = own_uid(&root);
        let dir = root.join("sock");
        auth::check_socket_dir(&auth::StdFs, uid, &dir).unwrap();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o750)).unwrap();
        let text = message(&auth::check_socket_dir(&auth::StdFs, uid, &dir).unwrap_err());
        assert!(
            text.contains(dir.to_str().unwrap()) && text.contains("0750"),
            "{text}"
        );
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700)).unwrap();
        let text = message(&auth::check_socket_dir(&auth::StdFs, uid + 1, &dir).unwrap_err());
        assert!(text.contains(dir.to_str().unwrap()), "{text}");
        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn a_symlink_in_place_of_the_directory_is_not_followed() {
        let root = fresh("auth-dir-link");
        let uid = own_uid(&root);
        let target = root.join("elsewhere");
        auth::check_socket_dir(&auth::StdFs, uid, &target).unwrap();
        let link = root.join("sock");
        std::os::unix::fs::symlink(&target, &link).unwrap();
        let text = message(&auth::check_socket_dir(&auth::StdFs, uid, &link).unwrap_err());
        assert!(text.contains(link.to_str().unwrap()), "{text}");
        let _ = fs::remove_dir_all(&root);
    }
}
