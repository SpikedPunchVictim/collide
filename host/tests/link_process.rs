//! The link between the daemon and its shims, over real processes (ts-0057-ck): `collide shim` processes with python3
//! children, a raw client on the shim's own socket, and `collide-daemon` processes.
//!
//! What is checked here: the shim's socket and its handshake, the messages it serves (`Write`, `CloseStdin`, `Signal`,
//! `AckExit`), a message cut in half over the real socket, a damaged frame, an exit record that is served until it is
//! acknowledged, and the whole chain of a phase that ends while the daemon is down (a daemon killed in the middle of a
//! phase, the shim starting the next daemon, and that daemon adopting the shim).
//!
//! Every process a test starts has a cleared environment with `HOME` and `COLLIDE_DAEMON_ROOT` in the test's own
//! folder under `/tmp`, so none of them touches the user's files. A guard stops each shim, daemon and child the test
//! started, by pid and start time, and signals nothing else.

mod common;

use collide_core::frame::encode_message;
use collide_core::journal::{Direction, read_journal};
use collide_core::shim::{
    DaemonMessage, ExitEnd, ExitRecord, SHIM_PROTOCOL, ShimMessage, StopSignal, parse_exit_record,
};
use collide_host::daemon::DaemonPaths;
use collide_host::shim::{ChildStdio, DaemonStart, ShimSpec, files};
use collide_host::shim_link::{FrameReader, Next};
use common::{PYTHON, ShimProc, TempRoot, wait_until};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const DAEMON: &str = env!("CARGO_BIN_EXE_collide-daemon");

/// A child that waits for its stdin to close.
const WAIT_STDIN: &str = "import sys; sys.stdin.read()";

fn spec(dir: &Path, script: &str, daemon: Option<DaemonStart>) -> ShimSpec {
    ShimSpec {
        program: PYTHON.into(),
        args: vec!["-u".into(), "-c".into(), script.into()],
        env: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
        cwd: dir.to_path_buf(),
        run_dir: dir.to_path_buf(),
        stdio: ChildStdio::Pipes,
        daemon,
    }
}

/// Makes a run directory `name` under the root, with `spec.json` for `script`, and starts a shim in it.
fn start_shim(root: &TempRoot, name: &str, script: &str) -> ShimProc {
    let dir = root.join(name);
    fs::create_dir_all(&dir).unwrap();
    fs::write(
        dir.join(files::SPEC),
        spec(&dir, script, None).to_json().to_string(),
    )
    .unwrap();
    ShimProc::start(root, &dir)
}

/// A client on a shim's socket that speaks frames.
struct Raw {
    stream: UnixStream,
    reader: FrameReader,
}

impl Raw {
    /// Connects, waiting for the shim to bind its socket.
    fn connect(dir: &Path) -> Raw {
        let socket = dir.join(files::SOCKET_DIR).join(files::SOCKET);
        let until = Instant::now() + Duration::from_secs(10);
        loop {
            match UnixStream::connect(&socket) {
                Ok(stream) => {
                    let reader =
                        FrameReader::new(stream.try_clone().unwrap(), Duration::from_millis(50));
                    return Raw { stream, reader };
                }
                Err(e) if Instant::now() < until => {
                    let _ = e;
                    std::thread::sleep(Duration::from_millis(20));
                }
                Err(e) => panic!("the shim's socket {} never answered: {e}", socket.display()),
            }
        }
    }

    /// The next message, or `None` when the shim closed the connection. Panics when nothing comes in `within`.
    fn next_within(&mut self, within: Duration) -> Option<ShimMessage> {
        let until = Instant::now() + within;
        loop {
            match self.reader.read_next::<ShimMessage>() {
                Ok(Next::Message(message)) => return Some(message),
                Ok(Next::Idle) => assert!(Instant::now() < until, "no message within {within:?}"),
                Ok(Next::Closed { .. }) => return None,
                Err(e) => panic!("reading from the shim: {e}"),
            }
        }
    }

    fn next(&mut self) -> Option<ShimMessage> {
        self.next_within(Duration::from_secs(10))
    }

    fn send(&mut self, message: &DaemonMessage) {
        self.stream
            .write_all(&encode_message(message).unwrap())
            .unwrap();
    }

    /// The shim's `Hello`, and the `Welcome` that answers it.
    fn handshake(&mut self) {
        assert_eq!(
            self.next(),
            Some(ShimMessage::Hello {
                protocol: SHIM_PROTOCOL
            })
        );
        self.send(&DaemonMessage::Welcome {
            protocol: SHIM_PROTOCOL,
        });
    }

    fn connect_and_handshake(dir: &Path) -> Raw {
        let mut raw = Raw::connect(dir);
        raw.handshake();
        raw
    }

    /// Messages until one that `pick` takes, skipping the others.
    fn until<T>(&mut self, mut pick: impl FnMut(ShimMessage) -> Option<T>) -> T {
        loop {
            let message = self.next().expect("the shim closed the connection");
            if let Some(found) = pick(message) {
                return found;
            }
        }
    }

    /// Reads until the shim closes the connection (messages it sends first are dropped).
    fn wait_closed(&mut self) {
        let until = Instant::now() + Duration::from_secs(10);
        while Instant::now() < until {
            if self.next_within(Duration::from_secs(10)).is_none() {
                return;
            }
        }
        panic!("the shim did not close the connection");
    }
}

fn socket_of(dir: &Path) -> PathBuf {
    dir.join(files::SOCKET_DIR).join(files::SOCKET)
}

fn journal_in_records(dir: &Path) -> Vec<Vec<u8>> {
    let bytes = fs::read(dir.join(files::JOURNAL)).unwrap();
    let read = read_journal(&bytes);
    read.records
        .into_iter()
        .filter(|record| record.direction == Direction::In)
        .map(|record| record.bytes)
        .collect()
}

fn exit_json(dir: &Path) -> ExitRecord {
    parse_exit_record(&fs::read(dir.join(files::EXIT_RECORD)).unwrap())
        .expect("exit.json is a record")
}

// ---- the socket and the handshake ----------------------------------------------------------------------------

#[test]
fn the_shim_speaks_first_and_its_socket_follows_the_daemons_rules() {
    let root = TempRoot::new();
    let shim = start_shim(&root, "d1", WAIT_STDIN);
    let mut raw = Raw::connect(&shim.dir);
    assert_eq!(
        raw.next(),
        Some(ShimMessage::Hello {
            protocol: SHIM_PROTOCOL
        })
    );
    let directory = fs::metadata(shim.dir.join(files::SOCKET_DIR)).unwrap();
    assert_eq!(directory.permissions().mode() & 0o7777, 0o700);
    assert_eq!(directory.uid(), unsafe { libc::geteuid() });
    let socket = fs::metadata(socket_of(&shim.dir)).unwrap();
    assert_eq!(socket.permissions().mode() & 0o7777, 0o600);
    assert!(socket_of(&shim.dir).as_os_str().len() <= 103);
}

#[test]
fn a_daemon_that_refuses_the_version_is_disconnected_and_the_shim_serves_the_next_one() {
    let root = TempRoot::new();
    let shim = start_shim(&root, "d1", WAIT_STDIN);
    let mut refused = Raw::connect(&shim.dir);
    assert!(matches!(refused.next(), Some(ShimMessage::Hello { .. })));
    refused.send(&DaemonMessage::Refused {
        message: "shim protocol 1 is older than this daemon accepts".to_string(),
    });
    refused.wait_closed();
    // The shim is still there for a daemon that does accept it.
    let mut next = Raw::connect_and_handshake(&shim.dir);
    next.send(&DaemonMessage::CloseStdin {});
    let exit = next.until(|m| match m {
        ShimMessage::Exited { exit } => Some(exit),
        _ => None,
    });
    assert_eq!(exit.end, ExitEnd::Exited { code: 0 });
}

#[test]
fn a_message_before_the_welcome_is_not_served() {
    let root = TempRoot::new();
    let shim = start_shim(&root, "d1", WAIT_STDIN);
    let mut early = Raw::connect(&shim.dir);
    assert!(matches!(early.next(), Some(ShimMessage::Hello { .. })));
    early.send(&DaemonMessage::Write {
        data: "too early\n".to_string(),
    });
    early.wait_closed();
    assert_eq!(journal_in_records(&shim.dir), Vec::<Vec<u8>>::new());
}

// ---- the messages the shim serves -----------------------------------------------------------------------------

#[test]
fn a_write_is_journaled_before_the_child_sees_it_and_answered_with_its_sequence() {
    let root = TempRoot::new();
    let mut shim = start_shim(
        &root,
        "d1",
        "import sys; print('got:' + sys.stdin.readline().strip())",
    );
    let mut raw = Raw::connect_and_handshake(&shim.dir);
    raw.send(&DaemonMessage::Write {
        data: "hello\n".to_string(),
    });
    assert_eq!(raw.next(), Some(ShimMessage::Wrote { seq: 0 }));
    let exit = raw.until(|m| match m {
        ShimMessage::Exited { exit } => Some(exit),
        _ => None,
    });
    assert_eq!(exit.end, ExitEnd::Exited { code: 0 });
    assert_eq!(journal_in_records(&shim.dir), vec![b"hello\n".to_vec()]);
    assert_eq!(
        fs::read_to_string(shim.dir.join(files::STDOUT_LOG)).unwrap(),
        "got:hello\n"
    );
    raw.send(&DaemonMessage::AckExit {});
    let status = shim
        .wait_exit(Duration::from_secs(10))
        .expect("the shim ends");
    assert!(status.success(), "{status:?}");
}

#[test]
fn closing_stdin_ends_a_child_that_reads_to_the_end() {
    let root = TempRoot::new();
    let shim = start_shim(&root, "d1", WAIT_STDIN);
    let mut raw = Raw::connect_and_handshake(&shim.dir);
    raw.send(&DaemonMessage::CloseStdin {});
    let exit = raw.until(|m| match m {
        ShimMessage::Exited { exit } => Some(exit),
        _ => None,
    });
    assert_eq!(exit.end, ExitEnd::Exited { code: 0 });
    assert_eq!(exit, exit_json(&shim.dir));
}

#[test]
fn a_signal_reaches_the_childs_process_group() {
    let root = TempRoot::new();
    let shim = start_shim(&root, "d1", "import time; time.sleep(60)");
    let mut raw = Raw::connect_and_handshake(&shim.dir);
    raw.send(&DaemonMessage::Signal {
        signal: StopSignal::Term,
    });
    let exit = raw.until(|m| match m {
        ShimMessage::Exited { exit } => Some(exit),
        _ => None,
    });
    assert_eq!(
        exit.end,
        ExitEnd::Signaled {
            signal: libc::SIGTERM
        }
    );
}

// ---- the exit record is served until it is acknowledged --------------------------------------------------------

#[test]
fn an_exit_record_is_served_again_to_the_next_connection_until_it_is_acknowledged() {
    let root = TempRoot::new();
    let mut shim = start_shim(&root, "d1", "import sys; sys.exit(3)");
    let mut first = Raw::connect_and_handshake(&shim.dir);
    let record = first.until(|m| match m {
        ShimMessage::Exited { exit } => Some(exit),
        _ => None,
    });
    assert_eq!(record.end, ExitEnd::Exited { code: 3 });
    // The daemon that read it went away before it acknowledged. The next one is given the record again.
    drop(first);
    let mut second = Raw::connect_and_handshake(&shim.dir);
    let again = second.until(|m| match m {
        ShimMessage::Exited { exit } => Some(exit),
        _ => None,
    });
    assert_eq!(again, record);
    second.send(&DaemonMessage::AckExit {});
    let status = shim
        .wait_exit(Duration::from_secs(10))
        .expect("the shim ends");
    assert!(status.success(), "{status:?}");
    assert!(
        UnixStream::connect(socket_of(&shim.dir)).is_err(),
        "the shim stops serving once the record is acknowledged"
    );
    assert_eq!(
        exit_json(&shim.dir),
        record,
        "exit.json is left for the daemon"
    );
}

// ---- a message cut in half --------------------------------------------------------------------------------------

#[test]
fn a_message_cut_in_half_and_then_dropped_is_never_read_as_whole() {
    let root = TempRoot::new();
    let shim = start_shim(
        &root,
        "d1",
        "import sys; print('read:' + repr(sys.stdin.read()))",
    );
    let mut cut = Raw::connect_and_handshake(&shim.dir);
    let frame = encode_message(&DaemonMessage::Write {
        data: "secret\n".to_string(),
    })
    .unwrap();
    cut.stream.write_all(&frame[..frame.len() / 2]).unwrap();
    std::thread::sleep(Duration::from_millis(300));
    drop(cut);
    std::thread::sleep(Duration::from_millis(300));
    assert_eq!(journal_in_records(&shim.dir), Vec::<Vec<u8>>::new());

    let mut next = Raw::connect_and_handshake(&shim.dir);
    next.send(&DaemonMessage::CloseStdin {});
    next.until(|m| matches!(m, ShimMessage::Exited { .. }).then_some(()));
    assert_eq!(
        fs::read_to_string(shim.dir.join(files::STDOUT_LOG)).unwrap(),
        "read:''\n",
        "the child got nothing from the half message"
    );
}

#[test]
fn a_message_that_arrives_in_pieces_is_read_once_whole_wherever_it_is_cut() {
    let root = TempRoot::new();
    let shim = start_shim(&root, "d1", WAIT_STDIN);
    let mut raw = Raw::connect_and_handshake(&shim.dir);
    let data = "piece\n";
    let frame = encode_message(&DaemonMessage::Write {
        data: data.to_string(),
    })
    .unwrap();
    // Cut inside the length prefix, right after it, inside the JSON, and one byte short of the end.
    let cuts = [1, 2, 3, 4, 5, frame.len() / 2, frame.len() - 1];
    for (round, cut) in cuts.iter().enumerate() {
        raw.stream.write_all(&frame[..*cut]).unwrap();
        raw.stream.flush().unwrap();
        std::thread::sleep(Duration::from_millis(100));
        // Nothing is answered while the message is not all there.
        assert_eq!(
            raw.next_within_or_idle(Duration::from_millis(100)),
            None,
            "round {round}: answered a message that was cut at {cut}"
        );
        raw.stream.write_all(&frame[*cut..]).unwrap();
        assert_eq!(
            raw.next(),
            Some(ShimMessage::Wrote { seq: round as u64 }),
            "round {round}, cut at {cut}"
        );
    }
    // And a message dribbled one byte at a time.
    for byte in &frame {
        raw.stream.write_all(std::slice::from_ref(byte)).unwrap();
        raw.stream.flush().unwrap();
    }
    assert_eq!(
        raw.next(),
        Some(ShimMessage::Wrote {
            seq: cuts.len() as u64
        })
    );
    let records = journal_in_records(&shim.dir);
    assert_eq!(records.len(), cuts.len() + 1);
    assert!(records.iter().all(|bytes| bytes == data.as_bytes()));
}

impl Raw {
    /// The next message if one arrives within `within`, and `None` for silence. A closed connection is a failure.
    fn next_within_or_idle(&mut self, within: Duration) -> Option<ShimMessage> {
        let until = Instant::now() + within;
        while Instant::now() < until {
            match self.reader.read_next::<ShimMessage>() {
                Ok(Next::Message(message)) => return Some(message),
                Ok(Next::Idle) => {}
                Ok(Next::Closed { .. }) => panic!("the shim closed the connection"),
                Err(e) => panic!("reading from the shim: {e}"),
            }
        }
        None
    }
}

#[test]
fn a_damaged_frame_closes_that_connection_and_the_shim_serves_the_next() {
    let root = TempRoot::new();
    let shim = start_shim(&root, "d1", WAIT_STDIN);

    // A length prefix far over the limit: the shim does not wait for 4 GiB.
    let mut huge = Raw::connect_and_handshake(&shim.dir);
    huge.stream.write_all(&[0xff, 0xff, 0xff, 0xff]).unwrap();
    huge.wait_closed();

    // A whole frame that is JSON but not a message.
    let mut wrong = Raw::connect_and_handshake(&shim.dir);
    let payload = br#"{"type":"no_such_message"}"#;
    let mut frame = (payload.len() as u32).to_be_bytes().to_vec();
    frame.extend_from_slice(payload);
    wrong.stream.write_all(&frame).unwrap();
    wrong.wait_closed();

    let mut good = Raw::connect_and_handshake(&shim.dir);
    good.send(&DaemonMessage::Write {
        data: "still here\n".to_string(),
    });
    assert_eq!(good.next(), Some(ShimMessage::Wrote { seq: 0 }));
}

#[test]
fn a_connection_that_says_nothing_does_not_stop_another() {
    let root = TempRoot::new();
    let shim = start_shim(&root, "d1", WAIT_STDIN);
    let mut silent = Raw::connect(&shim.dir);
    assert!(matches!(silent.next(), Some(ShimMessage::Hello { .. })));
    // `silent` never answers the Hello.
    let mut busy = Raw::connect_and_handshake(&shim.dir);
    busy.send(&DaemonMessage::Write {
        data: "x\n".to_string(),
    });
    assert_eq!(busy.next(), Some(ShimMessage::Wrote { seq: 0 }));
}

// ---- a phase that ends while the daemon is down -------------------------------------------------------------

/// A login shell that prints its environment, so a daemon's capture never runs the user's own shell.
fn fake_shell(root: &TempRoot) -> String {
    root.script("fake-shell", "#!/bin/sh\neval \"$4\"\n")
        .to_str()
        .unwrap()
        .to_string()
}

/// The environment of a daemon of this test, and of the shims that start one.
fn daemon_env(root: &TempRoot, shell: &str) -> BTreeMap<String, String> {
    BTreeMap::from([
        ("PATH".to_string(), "/usr/bin:/bin".to_string()),
        ("HOME".to_string(), root.path().display().to_string()),
        ("SHELL".to_string(), shell.to_string()),
        (
            "COLLIDE_DAEMON_ROOT".to_string(),
            root.path().display().to_string(),
        ),
    ])
}

fn start_command(env: &BTreeMap<String, String>) -> Command {
    let mut command = Command::new(DAEMON);
    command
        .arg("start")
        .env_clear()
        .envs(env)
        .stdin(Stdio::null());
    command
}

/// The daemon's status over its real socket, with the token from its token file. `None` when nothing answers.
fn status(paths: &DaemonPaths) -> Option<Value> {
    let stream = UnixStream::connect(&paths.socket).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(5))).ok()?;
    let token = fs::read_to_string(&paths.token_file).ok()?;
    let mut writer = stream.try_clone().ok()?;
    let mut reader = BufReader::new(stream);
    let mut ask = |id: u64, method: &str, params: Value| -> Option<Value> {
        let request = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        writer.write_all(format!("{request}\n").as_bytes()).ok()?;
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        serde_json::from_str(&line).ok()
    };
    ask(
        1,
        "initialize",
        json!({"protocol": 1, "token": token.trim()}),
    )?;
    Some(ask(2, "daemon.status", json!({}))?["result"].clone())
}

/// Stops every daemon that answered on a test's socket while the guard lived (`SIGTERM`, a wait, then `SIGKILL`), by
/// the pids those daemons reported themselves. It signals no other process.
struct Daemons {
    paths: DaemonPaths,
    pids: Vec<i32>,
}

impl Daemons {
    fn new(root: &TempRoot) -> Daemons {
        Daemons {
            paths: DaemonPaths::under(root.path()),
            pids: Vec::new(),
        }
    }

    /// The pid of the daemon that answers now, remembered for the stop.
    fn pid(&mut self) -> i32 {
        let pid = status(&self.paths).expect("a daemon answers")["pid"]
            .as_i64()
            .unwrap() as i32;
        if !self.pids.contains(&pid) {
            self.pids.push(pid);
        }
        pid
    }

    fn kill(&self, pid: i32, signal: i32) {
        // SAFETY: `kill` takes two integers; the pid is a daemon this test started, from its own socket.
        unsafe { libc::kill(pid, signal) };
    }

    fn wait_gone(pid: i32, within: Duration) -> bool {
        let until = Instant::now() + within;
        while Instant::now() < until {
            if !common::pid_alive(pid as u32) {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        !common::pid_alive(pid as u32)
    }

    /// Ends a daemon the way an operator does: `SIGTERM`, then `SIGKILL` if it stays.
    fn stop(&self, pid: i32) {
        self.kill(pid, libc::SIGTERM);
        if !Daemons::wait_gone(pid, Duration::from_secs(5)) {
            self.kill(pid, libc::SIGKILL);
            Daemons::wait_gone(pid, Duration::from_secs(5));
        }
    }
}

impl Drop for Daemons {
    fn drop(&mut self) {
        if let Some(live) = status(&self.paths).and_then(|s| s["pid"].as_i64())
            && !self.pids.contains(&(live as i32))
        {
            self.pids.push(live as i32);
        }
        for pid in std::mem::take(&mut self.pids) {
            self.stop(pid);
        }
    }
}

fn phase_transitions(paths: &DaemonPaths) -> Vec<(String, Option<String>, String)> {
    let store = collide_host::store::Store::open(&paths.store_dir).expect("the store opens");
    store
        .events_since(0)
        .unwrap()
        .into_iter()
        .filter_map(|event| match event.body {
            collide_core::events::EventBody::PhaseTransition {
                run,
                phase,
                from,
                to,
                ..
            } => Some((
                format!("{run}/{phase}"),
                from.map(|state| serde_json::to_string(&state).unwrap()),
                serde_json::to_string(&to).unwrap(),
            )),
            _ => None,
        })
        .collect()
}

#[test]
fn a_phase_that_ends_while_the_daemon_is_down_starts_the_daemon_which_adopts_it_and_keeps_the_totals()
 {
    let root = TempRoot::new();
    let shell = fake_shell(&root);
    let env = daemon_env(&root, &shell);
    let paths = DaemonPaths::under(root.path());
    let mut daemons = Daemons::new(&root);

    // The shim of one phase, started the way a daemon would (here, by the test, with nothing else running). The child
    // works for a while and ends with code 7 and some CPU time, long after the first daemon is gone.
    let dir = paths.runs_dir.join("r1").join("p1");
    fs::create_dir_all(&dir).unwrap();
    let script = "import time, sys\ntime.sleep(2.5)\nt = time.time()\nwhile time.time() - t < 0.3:\n    pass\nsys.exit(7)";
    let daemon_exe = fs::canonicalize(DAEMON).unwrap();
    let start = DaemonStart {
        exe: daemon_exe,
        env: env.clone(),
    };
    fs::write(
        dir.join(files::SPEC),
        spec(&dir, script, Some(start)).to_json().to_string(),
    )
    .unwrap();
    let mut shim = ShimProc::start(&root, &dir);

    // The first daemon adopts the shim that is running.
    let started = start_command(&env).output().unwrap();
    assert!(started.status.success(), "{started:?}");
    let first = daemons.pid();
    wait_until(
        "the first daemon counts the phase as live",
        Duration::from_secs(10),
        || status(&paths).is_some_and(|s| s["live_runs"] == 1),
    );

    // The daemon is killed in the middle of the phase.
    daemons.kill(first, libc::SIGKILL);
    assert!(Daemons::wait_gone(first, Duration::from_secs(5)));
    assert!(
        status(&paths).is_none(),
        "nothing answers while the daemon is down"
    );

    // The child ends while nothing runs. The shim starts the daemon, with no app and no CLI, and that daemon adopts
    // the shim and takes the record.
    wait_until("a new daemon answers", Duration::from_secs(30), || {
        status(&paths).is_some()
    });
    let second = daemons.pid();
    assert_ne!(first, second, "the shim started a new daemon");
    let ended = shim
        .wait_exit(Duration::from_secs(15))
        .expect("the shim ends once the daemon has the record");
    assert!(
        ended.success(),
        "the daemon acknowledged the record: {ended:?}"
    );

    // The exit code and the totals are the child's own, in exit.json.
    let record = exit_json(&dir);
    assert_eq!(record.end, ExitEnd::Exited { code: 7 });
    assert!(record.cpu_user_us > 0, "{record:?}");

    // The phase went Running (the first daemon) and then ended, once, by the second. The log line comes after the
    // event is stored, so the store is read once the second daemon is stopped.
    wait_until("the end is logged", Duration::from_secs(10), || {
        fs::read_to_string(&paths.log_file).is_ok_and(|log| log.contains("code 7"))
    });
    daemons.stop(second);
    let transitions = phase_transitions(&paths);
    assert_eq!(transitions.len(), 2, "{transitions:?}");
    assert_eq!(transitions[0].0, "r1/p1");
    assert_eq!(transitions[0].1, None);
    assert!(transitions[0].2.contains("running"), "{transitions:?}");
    assert!(transitions[1].1.as_deref().unwrap().contains("running"));
    assert!(
        transitions[1].2.contains("failed"),
        "exit code 7 is not a result: {transitions:?}"
    );
    let log = fs::read_to_string(&paths.log_file).unwrap();
    assert!(
        log.contains("r1/p1"),
        "the daemon's log names the phase: {log}"
    );

    // A third daemon finds the same ended phase and records nothing again.
    let started = start_command(&env).output().unwrap();
    assert!(started.status.success(), "{started:?}");
    let third = daemons.pid();
    daemons.stop(third);
    assert_eq!(phase_transitions(&paths).len(), 2, "adoption is idempotent");
}
