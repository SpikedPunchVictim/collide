//! The shim's side of the link, inside the test process (ts-0057-ck): a real [`Shim`] over a python3 child, with a
//! [`ShimLink`] on its socket. What is checked here and not in `link_process.rs`: the uid check (a link told that the
//! daemon is another user refuses the connection), the socket directory's rules, and the socket removed when the link
//! ends. The messages themselves, over a real `collide shim` process, are `link_process.rs`.

mod common;

use collide_core::shim::{DaemonMessage, SHIM_PROTOCOL, ShimMessage};
use collide_host::shim::{
    ChildStdio, Limits, LinkOptions, Shim, ShimError, ShimLink, ShimSpec, files,
};
use collide_host::shim_link::{FrameReader, Next};
use collide_host::sysproc;
use common::{PYTHON, TempRoot, kill_child_of};
use std::collections::BTreeMap;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// A shim in a run directory under the test's root, whose child waits for stdin to close. `Drop` ends the child.
struct InProcess {
    dir: PathBuf,
    shim: Arc<Shim>,
}

impl InProcess {
    fn start(dir: &Path) -> InProcess {
        fs::create_dir_all(dir).unwrap();
        let spec = ShimSpec {
            program: PYTHON.into(),
            args: vec!["-c".into(), "import sys; sys.stdin.read()".into()],
            env: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
            cwd: dir.to_path_buf(),
            run_dir: dir.to_path_buf(),
            stdio: ChildStdio::Pipes,
            daemon: None,
        };
        let shim = Arc::new(Shim::start(&spec, Limits::default()).expect("the shim starts"));
        InProcess {
            dir: dir.to_path_buf(),
            shim,
        }
    }
}

impl Drop for InProcess {
    fn drop(&mut self) {
        // The child ends, and the shim is taken, so nothing is left running.
        kill_child_of(&self.dir);
        let _ = self.shim.finish();
    }
}

fn socket_of(dir: &Path) -> PathBuf {
    dir.join(files::SOCKET_DIR).join(files::SOCKET)
}

fn next(reader: &mut FrameReader, within: Duration) -> Option<ShimMessage> {
    let until = Instant::now() + within;
    loop {
        match reader.read_next::<ShimMessage>().expect("a frame") {
            Next::Message(message) => return Some(message),
            Next::Idle => assert!(Instant::now() < until, "nothing came"),
            Next::Closed { .. } => return None,
        }
    }
}

#[test]
fn a_connection_is_answered_with_hello_and_the_socket_goes_when_the_link_ends() {
    let root = TempRoot::new();
    let run = InProcess::start(&root.join("d1"));
    let link = ShimLink::start(run.shim.clone(), &run.dir, LinkOptions::default()).unwrap();
    assert_eq!(link.socket(), socket_of(&run.dir));
    let stream = UnixStream::connect(link.socket()).unwrap();
    let mut reader = FrameReader::new(stream, Duration::from_millis(50));
    assert_eq!(
        next(&mut reader, Duration::from_secs(5)),
        Some(ShimMessage::Hello {
            protocol: SHIM_PROTOCOL
        })
    );
    drop(link);
    assert!(!socket_of(&run.dir).exists());
    assert!(!run.dir.join(files::SOCKET_DIR).exists());
}

#[test]
fn a_connection_from_another_uid_is_refused_before_anything_is_said() {
    let root = TempRoot::new();
    let run = InProcess::start(&root.join("d1"));
    // The link is told its user is someone else, so the real peer, this process, is a foreign uid to it.
    let options = LinkOptions {
        uid: sysproc::daemon_uid().wrapping_add(1),
        ..LinkOptions::default()
    };
    let link = ShimLink::start(run.shim.clone(), &run.dir, options).unwrap();
    let stream = UnixStream::connect(link.socket()).unwrap();
    let mut reader = FrameReader::new(stream, Duration::from_millis(50));
    assert_eq!(
        next(&mut reader, Duration::from_secs(5)),
        None,
        "no Hello, only a close"
    );
    assert_eq!(link.connections(), 0);
}

#[test]
fn a_socket_directory_that_is_not_private_is_refused() {
    let root = TempRoot::new();
    let run = InProcess::start(&root.join("d1"));
    let dir = run.dir.join(files::SOCKET_DIR);
    fs::create_dir(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    let Err(error) = ShimLink::start(run.shim.clone(), &run.dir, LinkOptions::default()) else {
        panic!("a directory of mode 0755 is not taken");
    };
    let message = error.to_string();
    assert!(
        message.contains("0755") && message.contains("0700"),
        "{message}"
    );
    assert!(matches!(error, ShimError::Write { .. }));
}

#[test]
fn a_socket_path_over_the_limit_is_refused_naming_the_path() {
    let root = TempRoot::new();
    let long = root.join(&"x".repeat(100));
    let run = InProcess::start(&long);
    let Err(error) = ShimLink::start(run.shim.clone(), &run.dir, LinkOptions::default()) else {
        panic!("a path over 103 bytes is not taken");
    };
    let message = error.to_string();
    assert!(message.contains("103"), "{message}");
}

#[test]
fn a_foreign_message_is_never_taken_for_the_daemons_welcome() {
    // The shim only speaks once the daemon has said `Welcome`: a `CloseStdin` as the first message ends nothing.
    let root = TempRoot::new();
    let run = InProcess::start(&root.join("d1"));
    let link = ShimLink::start(run.shim.clone(), &run.dir, LinkOptions::default()).unwrap();
    let mut stream = UnixStream::connect(link.socket()).unwrap();
    let mut reader = FrameReader::new(stream.try_clone().unwrap(), Duration::from_millis(50));
    assert!(matches!(
        next(&mut reader, Duration::from_secs(5)),
        Some(ShimMessage::Hello { .. })
    ));
    use std::io::Write;
    stream
        .write_all(&collide_core::frame::encode_message(&DaemonMessage::CloseStdin {}).unwrap())
        .unwrap();
    assert_eq!(next(&mut reader, Duration::from_secs(5)), None);
    // The child still runs: its stdin was not closed.
    assert!(run.shim.write_to_child(b"x").is_ok());
}
