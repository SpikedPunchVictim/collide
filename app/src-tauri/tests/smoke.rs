#[test]
fn crate_identifies_itself() {
    assert_eq!(collide_app::NAME, "collide-app");
}

/// `tauri dev` runs `cargo run` with no `--bin`. The crate has two binaries (the app and the test-only bridge), so
/// without `default-run` cargo cannot pick one and the dev app does not start.
#[test]
fn cargo_run_starts_the_app_not_the_bridge() {
    let manifest = include_str!("../Cargo.toml");
    assert!(
        manifest
            .lines()
            .any(|line| line.trim() == r#"default-run = "collide-app""#),
        "app/src-tauri/Cargo.toml must set default-run = \"collide-app\" in [package], or `tauri dev` fails"
    );
}
