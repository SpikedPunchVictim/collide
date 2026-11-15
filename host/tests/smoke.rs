use std::process::Command;

fn stdout_of(bin: &str) -> String {
    let output = Command::new(bin).output().expect("binary should run");
    assert!(
        output.status.success(),
        "{bin} exited with {}",
        output.status
    );
    String::from_utf8(output.stdout).expect("stdout should be UTF-8")
}

#[test]
fn crate_identifies_itself() {
    assert_eq!(collide_host::NAME, "collide-host");
}

#[test]
fn collide_binary_prints_its_name() {
    assert_eq!(stdout_of(env!("CARGO_BIN_EXE_collide")).trim(), "collide");
}

/// `collide-daemon` serves until it is stopped, so the smoke test does not run it: it gives the binary an argument it
/// does not know, with an environment of its own, and checks that it says so and ends. The daemon's real runs are in
/// `daemon_process.rs`, each in a folder of its own.
#[test]
fn collide_daemon_binary_refuses_an_argument_it_does_not_know() {
    let output = Command::new(env!("CARGO_BIN_EXE_collide-daemon"))
        .arg("--no-such-argument")
        .env_clear()
        .env("COLLIDE_DAEMON_ROOT", "/tmp/collide-smoke-never-made")
        .output()
        .expect("binary should run");
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--no-such-argument"), "{stderr}");
}
