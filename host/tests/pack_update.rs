//! `config.pack_update` and `collide pack update` (ts-0100-pu; 0.7, Switching packs). A daemon runs in the test's own
//! process over a fixture checkout, and every remote is a bare repository on disk in a temp root, so nothing reaches a
//! network host. A test that names a host (`example.invalid`) runs a fake `git` on its fetch's `PATH`: the fetch stops
//! at the new-host question, or git is the fake script that records itself and fails. The temp root is the home, so
//! `~/.config/collide` is the test's own, and no test touches the real one.

mod common;

use collide_core::owner::{
    ConfigPackUpdateParams, ConfigPackUpdateResult, PackFileChangeKind, RpcError, code,
};
use collide_host::git_fetch::{FetchEnv, GitFetch};
use collide_host::hosts::KnownHosts;
use collide_host::owner_client::ClientError;
use collide_host::pack_cache::materialize;
use common::config_fixture::Fixture;
use common::remote::Remote;
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;

struct World {
    fixture: Fixture,
    remote: Remote,
    board: String,
    source: String,
    /// The pins before and after the update the remote has to give.
    first: (String, String),
    second: (String, String),
    /// A root that outlives the fixture's use of a fake `git` kept in it, and the file that git records itself in.
    fake: Option<(common::TempRoot, PathBuf)>,
}

fn real_path() -> OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

/// A `git` on its own `PATH` that records each run and fails. Returns the `PATH` and the file.
fn recording_git(root: &std::path::Path) -> (OsString, PathBuf) {
    let dir = root.join("fake-git");
    fs::create_dir(&dir).unwrap();
    let ran = root.join("fake-git-ran");
    let body = format!("#!/bin/sh\necho ran >> '{}'\nexit 1\n", ran.display());
    fs::write(dir.join("git"), body).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dir.join("git"), fs::Permissions::from_mode(0o755)).unwrap();
    (dir.into_os_string(), ran)
}

/// A remote with the pack `remote` at `pack/` on `main`, tagged `v1`, and the same pack changed in a second commit,
/// tagged `v2`, not yet published.
fn remote_with_two_versions() -> (Remote, (String, String), (String, String)) {
    let remote = Remote::new();
    remote.work.write(
        "pack/pack.yaml",
        b"apiVersion: collide/v0\nname: remote\nversion: 1.0.0\n",
    );
    remote
        .work
        .write("pack/hooks/run.sh", b"#!/bin/sh\necho one\n");
    let c1 = remote.work.commit_all("one");
    let t1 = remote.work.git(&["rev-parse", "HEAD:pack"]);
    remote.work.git(&["tag", "v1"]);
    remote.publish();
    remote.work.write(
        "pack/pack.yaml",
        b"apiVersion: collide/v0\nname: remote\nversion: 2.0.0\n",
    );
    remote
        .work
        .write("pack/hooks/run.sh", b"#!/bin/sh\necho two\n");
    let c2 = remote.work.commit_all("two");
    let t2 = remote.work.git(&["rev-parse", "HEAD:pack"]);
    remote.work.git(&["tag", "v2"]);
    // The remote's own repository is moved to the second commit by `publish`, which each test calls when it wants it.
    (remote, (c1, t1), (c2, t2))
}

/// A board that lists the pack at the remote's file URL, locked at the first commit, with the cache warm.
fn world() -> World {
    let (remote, first, second) = remote_with_two_versions();
    let fixture = Fixture::fetching(real_path());
    let source = format!("git:{}//pack@main", remote.file_url());
    fixture.write(
        ".collide/board.yaml",
        &format!("packs:\n  - source: {source}\n"),
    );
    fixture.write(
        ".collide/packs.lock",
        &format!(
            "packs:\n  - source: {source}\n    commit: {}\n    tree: {}\n",
            first.0, first.1
        ),
    );
    fixture.commit();
    let board = fixture.board();
    let world = World {
        fixture,
        remote,
        board,
        source,
        first,
        second,
        fake: None,
    };
    // The first load fetches the first commit (the second is not published yet) and warms the cache.
    let got: Result<collide_core::owner::ConfigGetResult, _> = world.fixture.client().call(
        "config.get",
        &collide_core::owner::ConfigGetParams {
            board: Some(world.board.clone()),
            key: None,
            profile: None,
        },
        Some(Duration::from_secs(60)),
    );
    got.expect("the board loads");
    world.remote.publish();
    world
}

fn call(
    world: &World,
    name: &str,
    to: Option<&str>,
    confirm: bool,
    approve: &[&str],
) -> Result<ConfigPackUpdateResult, RpcError> {
    let params = ConfigPackUpdateParams {
        board: world.board.clone(),
        name: name.to_string(),
        to: to.map(str::to_string),
        confirm,
        approve_hosts: approve.iter().map(|host| host.to_string()).collect(),
    };
    match world
        .fixture
        .client()
        .call("config.pack_update", &params, Some(Duration::from_secs(60)))
    {
        Ok(result) => Ok(result),
        Err(ClientError::Rpc(error)) => Err(error),
        Err(other) => panic!("{other}"),
    }
}

fn lock_in_checkout(world: &World) -> String {
    fs::read_to_string(world.fixture.repo.work().join(".collide/packs.lock")).unwrap()
}

/// A board whose lock names a commit the remote never had, so no load can use it. Nothing is warmed: the remote has
/// both of its commits published and the cache is empty.
fn bad_pin_world() -> World {
    let (remote, first, second) = remote_with_two_versions();
    remote.publish();
    let fixture = Fixture::fetching(real_path());
    let source = format!("git:{}//pack@main", remote.file_url());
    fixture.write(
        ".collide/board.yaml",
        &format!("packs:\n  - source: {source}\n"),
    );
    fixture.write(
        ".collide/packs.lock",
        &format!(
            "packs:\n  - source: {source}\n    commit: {}\n    tree: {}\n",
            "a".repeat(40),
            "b".repeat(40)
        ),
    );
    fixture.commit();
    let board = fixture.board();
    World {
        fixture,
        remote,
        board,
        source,
        first,
        second,
        fake: None,
    }
}

#[test]
fn a_lock_whose_commit_is_gone_is_moved_by_update_to_a_ref() {
    let w = bad_pin_world();
    let load: Result<collide_core::owner::ConfigGetResult, _> = w.fixture.client().call(
        "config.get",
        &collide_core::owner::ConfigGetParams {
            board: Some(w.board.clone()),
            key: None,
            profile: None,
        },
        Some(Duration::from_secs(60)),
    );
    assert!(load.is_err(), "the premise: the line does not load");

    let preview = call(&w, "remote", Some("v1"), false, &[]).unwrap();
    assert_eq!(preview.old.commit, "a".repeat(40));
    assert_eq!(preview.new.as_ref().unwrap().commit, w.first.0);
    let done = call(&w, "remote", Some("v1"), true, &[]).unwrap();
    assert!(done.written);
    let text = lock_in_checkout(&w);
    assert!(
        text.contains(&w.first.0) && text.contains(&w.first.1),
        "{text}"
    );
    w.fixture.commit();
    let again = call(&w, "remote", Some("v2"), false, &[]).unwrap();
    assert_eq!(again.old.commit, w.first.0, "the repaired line loads");
}

#[test]
fn one_broken_pack_does_not_block_updating_another() {
    let w = world();
    // `ferry` is a repo pack with a manifest that does not parse: the line cannot load.
    w.fixture.write(
        ".collide/board.yaml",
        &format!(
            "packs:\n  - source: repo:.collide/packs/ferry\n  - source: {}\n",
            w.source
        ),
    );
    w.fixture
        .write(".collide/packs/ferry/pack.yaml", "name: [broken\n");
    w.fixture.commit();
    let result = call(&w, "remote", None, false, &[]).unwrap();
    assert_eq!(result.new.unwrap().commit, w.second.0);
}

#[test]
fn a_name_that_no_pack_at_its_pin_has_is_still_not_found_when_a_pin_is_bad() {
    let w = bad_pin_world();
    let error = call(&w, "nothing-by-this-name", None, false, &[]).unwrap_err();
    assert_eq!(error.code, code::NOT_FOUND);
    assert!(
        error.message.contains("nothing-by-this-name"),
        "{}",
        error.message
    );
}

#[test]
fn a_preview_shows_the_old_and_new_pins_and_the_diff_and_writes_nothing() {
    let w = world();
    let before = lock_in_checkout(&w);
    let result = call(&w, "remote", None, false, &[]).unwrap();
    assert_eq!(result.pack.name, "remote");
    assert_eq!(result.pack.source, w.source);
    assert_eq!(
        (result.old.commit.as_str(), result.old.tree.as_str()),
        (w.first.0.as_str(), w.first.1.as_str())
    );
    let new = result.new.expect("a new pin");
    assert_eq!(
        (new.commit.as_str(), new.tree.as_str()),
        (w.second.0.as_str(), w.second.1.as_str())
    );
    assert!(!result.written);
    assert_eq!(result.needs_host, None);
    let manifest = result
        .changes
        .iter()
        .find(|c| c.path == "pack.yaml")
        .expect("pack.yaml changed");
    assert_eq!(manifest.kind, PackFileChangeKind::Modified);
    assert!(
        manifest.lines.iter().any(|line| line == "+version: 2.0.0"),
        "{:?}",
        manifest.lines
    );
    assert!(result.changes.iter().any(|c| c.path == "hooks/run.sh"));
    assert_eq!(lock_in_checkout(&w), before, "a preview writes nothing");
}

#[test]
fn confirming_edits_the_checkouts_lock_and_leaves_it_uncommitted() {
    let w = world();
    let result = call(&w, "remote", None, true, &[]).unwrap();
    assert!(result.written);
    let lock_file = result.lock_file.expect("the file written");
    assert!(lock_file.ends_with(".collide/packs.lock"), "{lock_file}");
    let text = lock_in_checkout(&w);
    assert!(
        text.contains(&w.second.0) && text.contains(&w.second.1),
        "{text}"
    );
    // Uncommitted: the commit still has the old pin, and git sees the file as modified.
    let committed = w.fixture.repo.git(&["show", "HEAD:.collide/packs.lock"]);
    assert!(
        committed.contains(&w.first.0) && !committed.contains(&w.second.0),
        "{committed}"
    );
    assert_eq!(
        w.fixture.repo.git(&["status", "--porcelain"]),
        "M .collide/packs.lock"
    );
}

#[test]
fn once_the_lock_is_committed_the_next_load_uses_the_new_tree() {
    let w = world();
    call(&w, "remote", None, true, &[]).unwrap();
    w.fixture.commit();
    let again = call(&w, "remote", None, false, &[]).unwrap();
    assert_eq!(again.old.tree, w.second.1, "the loader read the new pin");
    assert_eq!(again.old.commit, w.second.0);
}

#[test]
fn the_pins_of_other_packs_in_the_lock_keep_their_place() {
    let w = world();
    let other = format!("git:{}//pack@v1", w.remote.file_url());
    w.fixture.write(
        ".collide/packs.lock",
        &format!(
            "packs:\n  - source: {}\n    commit: {}\n    tree: {}\n  - source: {other}\n    commit: {}\n    tree: {}\n",
            w.source, w.first.0, w.first.1, w.first.0, w.first.1
        ),
    );
    call(&w, "remote", None, true, &[]).unwrap();
    let text = lock_in_checkout(&w);
    assert!(
        text.find(&w.source).unwrap() < text.find(&other).unwrap(),
        "{text}"
    );
    assert!(
        text.contains(&format!("commit: \"{}\"", w.second.0)),
        "{text}"
    );
}

#[test]
fn to_takes_a_branch_a_tag_or_a_commit() {
    let w = world();
    for to in ["main", "v2", w.second.0.as_str()] {
        let result = call(&w, "remote", Some(to), false, &[]).unwrap();
        assert_eq!(result.new.unwrap().commit, w.second.0, "--to {to}");
    }
}

#[test]
fn to_an_older_tag_moves_the_pin_back() {
    let w = world();
    call(&w, "remote", None, true, &[]).unwrap();
    w.fixture.commit();
    let back = call(&w, "remote", Some("v1"), true, &[]).unwrap();
    assert_eq!(back.new.unwrap().commit, w.first.0);
    assert!(back.written);
}

#[test]
fn a_ref_the_remote_does_not_have_is_refused_naming_it() {
    let w = world();
    let error = call(&w, "remote", Some("no-such-ref"), false, &[]).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("no-such-ref"), "{}", error.message);
}

#[test]
fn an_update_to_the_pin_the_pack_already_has_writes_nothing_even_when_confirmed() {
    let w = world();
    let result = call(&w, "remote", Some("v1"), true, &[]).unwrap();
    assert!(!result.written);
    assert!(result.changes.is_empty());
    assert!(result.note.is_some());
    assert_eq!(w.fixture.repo.git(&["status", "--porcelain"]), "");
}

#[test]
fn a_repo_pack_is_refused_because_it_changes_by_a_commit_alone() {
    let w = world();
    w.fixture.ferry();
    w.fixture.write(
        ".collide/board.yaml",
        &format!(
            "packs:\n  - source: repo:.collide/packs/ferry\n  - source: {}\n",
            w.source
        ),
    );
    w.fixture.commit();
    let error = call(&w, "ferry", None, false, &[]).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(
        error
            .message
            .contains("a repo pack changes by a commit alone"),
        "{}",
        error.message
    );
}

#[test]
fn a_built_in_pack_is_refused() {
    let w = world();
    let error = call(&w, "collide", None, false, &[]).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("built-in"), "{}", error.message);
}

#[test]
fn a_name_no_pack_has_is_not_found() {
    let w = world();
    let error = call(&w, "nothing-by-this-name", None, false, &[]).unwrap_err();
    assert_eq!(error.code, code::NOT_FOUND);
    assert!(
        error.message.contains("nothing-by-this-name"),
        "{}",
        error.message
    );
}

#[test]
fn a_dev_pack_is_refused_with_the_reason() {
    let w = world();
    let folder = w.fixture.home().join("devpack");
    fs::create_dir_all(&folder).unwrap();
    fs::write(
        folder.join("pack.yaml"),
        "apiVersion: collide/v0\nname: devpack\n",
    )
    .unwrap();
    w.fixture.write_user(
        "packs.yaml",
        &format!("packs:\n  - source: dev:{}\n", folder.display()),
    );
    let error = call(&w, "devpack", None, false, &[]).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(
        error.message.contains("dev:") && error.message.contains("working copy"),
        "{}",
        error.message
    );
}

/// The user's pack `remote`: listed in the user's `packs.yaml`, locked in the user's `packs.lock`, with the mirror
/// already fetched, and no board that lists it.
fn user_world() -> World {
    let (remote, first, second) = remote_with_two_versions();
    remote.publish();
    let fixture = Fixture::fetching(real_path());
    let source = format!("git:{}//pack@main", remote.file_url());
    fixture.write_user("packs.yaml", &format!("packs:\n  - source: {source}\n"));
    fixture.write_user(
        "packs.lock",
        &format!(
            "packs:\n  - source: {source}\n    commit: {}\n    tree: {}\n",
            first.0, first.1
        ),
    );
    // The mirror, as a first `pack add` would have left it.
    let home = fixture.home().to_path_buf();
    let fetch = GitFetch::new(FetchEnv {
        path: real_path(),
        home: home.clone(),
        ssh_auth_sock: None,
        xdg_config_home: None,
    })
    .without_system_config();
    let mirror = collide_host::git_fetch::mirror_path(&home.join("cache"), &remote.file_url());
    fetch
        .fetch(&mirror, &remote.file_url(), &KnownHosts::for_home(&home))
        .unwrap();
    let board = fixture.board();
    World {
        fixture,
        remote,
        board,
        source,
        first,
        second,
        fake: None,
    }
}

#[test]
fn a_user_packs_update_is_written_to_the_users_lock_and_not_the_checkouts() {
    let w = user_world();
    let preview = call(&w, "remote", None, false, &[]).unwrap();
    assert_eq!(preview.pack.origin, collide_core::owner::PackOrigin::User);
    assert_eq!(preview.old.commit, w.first.0);
    assert_eq!(preview.new.as_ref().unwrap().commit, w.second.0);
    assert!(!preview.written);

    let done = call(&w, "remote", None, true, &[]).unwrap();
    assert!(done.written);
    let file = w.fixture.home().join(".config/collide/packs.lock");
    assert_eq!(done.lock_file.as_deref(), Some(file.to_str().unwrap()));
    let text = fs::read_to_string(&file).unwrap();
    assert!(
        text.contains(&w.second.0) && text.contains(&w.second.1),
        "{text}"
    );
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        fs::metadata(&file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(!w.fixture.repo.work().join(".collide/packs.lock").exists());
}

// ---- the new-host question ----

/// A board whose pack is at `https://example.invalid/...`, with the pack already in the cache, so the load needs no
/// fetch and the update is what reaches for the host. The fetch's `git` is a fake that records itself and fails.
fn host_world() -> World {
    let (remote, first, second) = remote_with_two_versions();
    remote.publish();
    let keep = common::TempRoot::new();
    let (path, ran) = recording_git(keep.path());
    let fixture = Fixture::fetching(path);
    let source = "git:https://example.invalid/o/r//pack@main".to_string();
    // The first commit's tree, taken from the remote's repository into the fixture's cache.
    materialize(
        &fixture.home().join("cache"),
        &fixture.home().join("sdk-copy"),
        remote.path(),
        &first.1,
    )
    .unwrap();
    fixture.write(
        ".collide/board.yaml",
        &format!("packs:\n  - source: {source}\n"),
    );
    fixture.write(
        ".collide/packs.lock",
        &format!(
            "packs:\n  - source: {source}\n    commit: {}\n    tree: {}\n",
            first.0, first.1
        ),
    );
    fixture.commit();
    let board = fixture.board();
    World {
        fixture,
        remote,
        board,
        source,
        first,
        second,
        fake: Some((keep, ran)),
    }
}

fn fake_ran(world: &World) -> bool {
    world.fake.as_ref().is_some_and(|(_, ran)| ran.exists())
}

#[test]
fn a_new_host_is_reported_with_nothing_fetched_and_nothing_written() {
    let w = host_world();
    let result = call(&w, "remote", None, true, &[]).unwrap();
    assert_eq!(result.needs_host.as_deref(), Some("example.invalid"));
    assert_eq!(result.new, None);
    assert!(!result.written);
    assert!(!fake_ran(&w), "git ran");
    assert!(!KnownHosts::for_home(w.fixture.home()).file().exists());
    assert_eq!(w.fixture.repo.git(&["status", "--porcelain"]), "");
}

#[test]
fn approving_the_host_records_it_and_goes_on_to_fetch() {
    let w = host_world();
    // The fetch is the fake git, which fails: the update fails, which shows git was started after the host was noted.
    let error = call(&w, "remote", None, false, &["example.invalid"]).unwrap_err();
    assert!(error.message.contains("remote"), "{}", error.message);
    assert!(fake_ran(&w));
    assert!(
        KnownHosts::for_home(w.fixture.home())
            .is_known("example.invalid")
            .unwrap()
    );
}

#[test]
fn approving_some_other_host_records_nothing() {
    let w = host_world();
    let result = call(&w, "remote", None, false, &["other.example"]).unwrap();
    assert_eq!(result.needs_host.as_deref(), Some("example.invalid"));
    assert!(!fake_ran(&w));
    assert!(!KnownHosts::for_home(w.fixture.home()).file().exists());
}

// ---- the command ----

mod command {
    use super::*;
    use collide_host::pack_cli::{USAGE, run_with};
    use collide_host::task_cli::exit;

    fn pack(world: &World, args: &[&str], input: &str) -> (u8, String, String) {
        let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with(
            world.fixture.daemon.paths(),
            &args,
            world.fixture.repo.work(),
            &mut input.as_bytes(),
            &mut out,
            &mut err,
        );
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn with_yes_the_preview_is_printed_and_the_lock_written() {
        let w = world();
        let (code, out, err) = pack(&w, &["update", "remote", "--yes"], "");
        assert_eq!((code, err.as_str()), (exit::OK, ""));
        assert!(out.contains("remote"), "{out}");
        assert!(
            out.contains(&w.first.0[..7]) && out.contains(&w.second.0[..7]),
            "{out}"
        );
        assert!(out.contains("modified pack.yaml"), "{out}");
        assert!(out.contains("+version: 2.0.0"), "{out}");
        assert!(out.contains(".collide/packs.lock"), "{out}");
        assert!(lock_in_checkout(&w).contains(&w.second.0));
    }

    #[test]
    fn without_yes_the_person_is_asked_and_a_no_writes_nothing() {
        let w = world();
        let before = lock_in_checkout(&w);
        for input in ["n\n", "", "\n", "what\n"] {
            let (code, out, _) = pack(&w, &["update", "remote"], input);
            assert_eq!(code, exit::OK, "{input:?}");
            assert!(out.contains("nothing was written"), "{out}");
        }
        assert_eq!(lock_in_checkout(&w), before);
    }

    #[test]
    fn without_yes_a_y_writes_the_lock() {
        let w = world();
        let (code, _, _) = pack(&w, &["update", "remote"], "y\n");
        assert_eq!(code, exit::OK);
        assert!(lock_in_checkout(&w).contains(&w.second.0));
    }

    #[test]
    fn to_is_passed_on() {
        let w = world();
        let (code, out, _) = pack(&w, &["update", "remote", "--to", "v1"], "");
        assert_eq!(code, exit::OK);
        assert!(out.contains("already"), "{out}");
    }

    #[test]
    fn a_new_host_asks_and_a_no_fetches_nothing() {
        let w = host_world();
        let (code, out, err) = pack(&w, &["update", "remote", "--yes"], "n\n");
        assert_eq!(code, exit::REFUSED, "{out}{err}");
        assert!(
            out.contains("example.invalid") || err.contains("example.invalid"),
            "{out}{err}"
        );
        assert!(!fake_ran(&w));
        assert!(!KnownHosts::for_home(w.fixture.home()).file().exists());
    }

    #[test]
    fn a_new_host_with_allow_host_is_recorded_without_asking() {
        let w = host_world();
        let (code, _, err) = pack(
            &w,
            &[
                "update",
                "remote",
                "--yes",
                "--allow-host",
                "example.invalid",
            ],
            "",
        );
        assert_eq!(code, exit::FAILED, "the fake git fails");
        assert!(err.contains("remote"), "{err}");
        assert!(fake_ran(&w));
        assert!(
            KnownHosts::for_home(w.fixture.home())
                .is_known("example.invalid")
                .unwrap()
        );
    }

    #[test]
    fn a_new_host_with_a_yes_at_the_question_is_recorded() {
        let w = host_world();
        let (_, _, _) = pack(&w, &["update", "remote", "--yes"], "y\n");
        assert!(
            KnownHosts::for_home(w.fixture.home())
                .is_known("example.invalid")
                .unwrap()
        );
    }

    #[test]
    fn a_refusal_exits_1_with_the_reason_and_a_missing_pack_exits_3() {
        let w = world();
        let (code, out, err) = pack(&w, &["update", "collide", "--yes"], "");
        assert_eq!((code, out.as_str()), (exit::FAILED, ""));
        assert!(err.contains("built-in"), "{err}");
        let (code, _, err) = pack(&w, &["update", "nothing-by-this-name"], "");
        assert_eq!(code, exit::ID);
        assert!(err.contains("nothing-by-this-name"), "{err}");
    }

    #[test]
    fn a_bad_command_line_exits_2_with_the_usage_and_starts_no_daemon() {
        // No fixture: the root is a temp folder with no daemon, and a daemon would have to be started to answer.
        let root = common::TempRoot::new();
        let paths = collide_host::daemon::DaemonPaths::under(root.path());
        for args in [
            vec!["update"],
            vec!["update", "a", "b"],
            vec!["update", "a", "--to"],
            vec!["update", "a", "--to", ""],
            vec!["update", "a", "--nope"],
            vec!["update", "a", "--allow-host"],
            vec!["update", ""],
            vec!["frob"],
            vec![],
        ] {
            let args: Vec<String> = args.iter().map(|a| a.to_string()).collect();
            let (mut out, mut err) = (Vec::new(), Vec::new());
            let code = run_with(
                &paths,
                &args,
                root.path(),
                &mut "".as_bytes(),
                &mut out,
                &mut err,
            );
            let err = String::from_utf8(err).unwrap();
            assert_eq!(code, exit::USAGE, "{args:?}: {err}");
            assert!(err.contains(USAGE), "{args:?}: {err}");
        }
        assert!(common::daemon_status(&paths).is_none());
    }
}
