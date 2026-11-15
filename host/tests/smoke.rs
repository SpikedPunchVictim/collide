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

#[test]
fn collide_daemon_binary_prints_its_name() {
    assert_eq!(
        stdout_of(env!("CARGO_BIN_EXE_collide-daemon")).trim(),
        "collide-daemon"
    );
}
