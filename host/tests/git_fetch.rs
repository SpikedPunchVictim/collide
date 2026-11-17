//! The mirror and the fetch rules (ts-0100-pu; 0.7, Pack sources and Fetch rules). Every remote here is a bare
//! repository on disk, in a temp root, reached by a path or a `file://` URL: nothing contacts a network host. A URL
//! with a host (`https://`, `ssh://`, scp-like) appears only where the fetch stops before git runs (the host is new) or
//! where git is a fake script on a test `PATH` that records how it was called. The environment of every git is the
//! fetch's own (`FetchEnv`), made from the test's temp root, and no test reads or writes the real home.

mod common;

use collide_host::git_fetch::{
    FETCH_TIMEOUT, FetchEnv, FetchError, GitFetch, mirror_name, mirror_path,
};
use collide_host::git_objects::{GitObjects, ObjectKind};
use collide_host::hosts::KnownHosts;
use common::git_repo::git_text;
use common::remote::Remote;
use common::{TempRoot, find_on_path};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

fn real_path() -> OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

fn env_of(root: &TempRoot, path: OsString) -> FetchEnv {
    FetchEnv {
        path,
        home: root.path().to_path_buf(),
        ssh_auth_sock: None,
        xdg_config_home: None,
    }
}

/// A fetch with real git, the test's own home and no system git config.
fn real_fetch(root: &TempRoot) -> GitFetch {
    GitFetch::new(env_of(root, real_path())).without_system_config()
}

fn mirror_in(root: &TempRoot) -> PathBuf {
    root.join("cache/git/mirror")
}

fn new_bare(root: &TempRoot, path: &Path) {
    fs::create_dir_all(path).unwrap();
    git_text(root.path(), path, &["init", "--quiet", "--bare"]);
}

fn kind(mirror: &Path, id: &str) -> Option<ObjectKind> {
    GitObjects::new().object_kind(mirror, id).unwrap()
}

// ---- the mirror's name ----

#[test]
fn a_mirrors_name_is_the_url_made_safe_and_a_hash_of_the_url() {
    let name = mirror_name("https://github.com/org/ferry-pack");
    assert!(
        name.starts_with("https___github.com_org_ferry-pack-"),
        "{name}"
    );
    let (_, hash) = name.rsplit_once('-').unwrap();
    assert_eq!(hash.len(), 8, "{name}");
    assert!(hash.bytes().all(|b| b.is_ascii_hexdigit()), "{name}");
}

#[test]
fn a_mirrors_name_has_only_letters_digits_dots_dashes_and_underscores() {
    for url in [
        "git@h:o/r",
        "/tmp/a b/../c",
        "file:///x?y=z#w",
        "ext::sh -c 'x;y'",
        "naïve/üri",
    ] {
        let name = mirror_name(url);
        assert!(
            name.chars()
                .all(|c| c.is_ascii_alphanumeric() || "._-".contains(c)),
            "{url} gave {name}"
        );
        assert!(!name.starts_with('.'), "{name}");
    }
}

#[test]
fn a_mirrors_name_is_bounded_and_a_long_url_still_gets_its_own() {
    let one = mirror_name(&format!("https://h/{}1", "a".repeat(300)));
    let two = mirror_name(&format!("https://h/{}2", "a".repeat(300)));
    assert!(one.len() <= 109, "{}", one.len());
    assert_ne!(one, two);
}

#[test]
fn a_mirrors_name_is_stable_and_two_urls_that_make_the_same_text_differ() {
    assert_eq!(mirror_name("https://h/a"), mirror_name("https://h/a"));
    assert_ne!(mirror_name("https://h/a b"), mirror_name("https://h/a_b"));
    // The same repository by two URLs is two mirrors: the name is of the URL as written.
    assert_ne!(mirror_name("https://h/a"), mirror_name("https://h/a.git"));
}

#[test]
fn a_mirrors_name_never_changes_between_builds() {
    // FNV-1a, 32 bits, of the URL's bytes. A change here would orphan every mirror on disk.
    assert_eq!(mirror_name("a"), "a-e40c292c");
}

#[test]
fn a_mirror_is_in_the_caches_git_folder() {
    assert_eq!(
        mirror_path(Path::new("/c"), "https://h/a"),
        Path::new("/c/git").join(mirror_name("https://h/a"))
    );
}

// ---- the fetch against a remote on disk ----

fn remote_with_a_commit() -> (Remote, String) {
    let remote = Remote::new();
    remote.work.write("pack/pack.yaml", b"name: ferry\n");
    let commit = remote.work.commit_all("first");
    remote.publish();
    (remote, commit)
}

#[test]
fn a_fetch_makes_the_mirror_when_it_is_not_there_and_gets_every_branch_and_tag() {
    let (remote, first) = remote_with_a_commit();
    remote.work.git(&["tag", "v1"]);
    remote.work.git(&["checkout", "--quiet", "-b", "feature"]);
    remote.work.write("pack/more.txt", b"more\n");
    let second = remote.work.commit_all("second");
    remote.publish();
    let root = TempRoot::new();
    let mirror = mirror_in(&root);
    let hosts = KnownHosts::for_home(root.path());

    real_fetch(&root)
        .fetch(&mirror, &remote.file_url(), &hosts)
        .unwrap();

    assert_eq!(kind(&mirror, &first), Some(ObjectKind::Commit));
    let git = GitObjects::new();
    assert_eq!(git.resolve_commit(&mirror, "main").unwrap(), first);
    assert_eq!(git.resolve_commit(&mirror, "feature").unwrap(), second);
    assert_eq!(git.resolve_commit(&mirror, "v1").unwrap(), first);
}

#[test]
fn a_remote_that_is_a_path_on_disk_is_fetched_with_no_host_asked_about() {
    let (remote, first) = remote_with_a_commit();
    let root = TempRoot::new();
    let mirror = mirror_in(&root);
    let hosts = KnownHosts::for_home(root.path());
    real_fetch(&root)
        .fetch(&mirror, remote.path().to_str().unwrap(), &hosts)
        .unwrap();
    assert_eq!(kind(&mirror, &first), Some(ObjectKind::Commit));
    assert!(!hosts.file().exists(), "nothing was recorded");
}

#[test]
fn a_ref_less_remote_head_is_in_the_mirror_under_the_name_the_resolver_uses() {
    let (remote, first) = remote_with_a_commit();
    let root = TempRoot::new();
    let mirror = mirror_in(&root);
    real_fetch(&root)
        .fetch(
            &mirror,
            &remote.file_url(),
            &KnownHosts::for_home(root.path()),
        )
        .unwrap();
    assert_eq!(
        GitObjects::new()
            .resolve_commit(&mirror, "refs/collide/head")
            .unwrap(),
        first
    );
}

#[test]
fn a_second_fetch_moves_a_branch_adds_a_tag_and_keeps_the_commit_the_branch_left() {
    let (remote, first) = remote_with_a_commit();
    let root = TempRoot::new();
    let mirror = mirror_in(&root);
    let hosts = KnownHosts::for_home(root.path());
    let fetch = real_fetch(&root);
    fetch.fetch(&mirror, &remote.file_url(), &hosts).unwrap();

    remote
        .work
        .write("pack/pack.yaml", b"name: ferry\nversion: 2\n");
    let second = remote.work.commit_all("second");
    remote.work.git(&["tag", "v2"]);
    remote.publish();
    fetch.fetch(&mirror, &remote.file_url(), &hosts).unwrap();

    let git = GitObjects::new();
    assert_eq!(git.resolve_commit(&mirror, "main").unwrap(), second);
    assert_eq!(git.resolve_commit(&mirror, "v2").unwrap(), second);
    assert_eq!(
        kind(&mirror, &first),
        Some(ObjectKind::Commit),
        "an old commit is not pruned"
    );
}

#[test]
fn a_branch_that_was_rewritten_is_moved_to_the_new_commit() {
    let (remote, first) = remote_with_a_commit();
    let root = TempRoot::new();
    let mirror = mirror_in(&root);
    let hosts = KnownHosts::for_home(root.path());
    let fetch = real_fetch(&root);
    fetch.fetch(&mirror, &remote.file_url(), &hosts).unwrap();

    remote.work.write("pack/pack.yaml", b"name: rewritten\n");
    remote.work.git(&[
        "commit",
        "--quiet",
        "--amend",
        "--all",
        "--message",
        "rewritten",
    ]);
    let rewritten = remote.work.git(&["rev-parse", "HEAD"]);
    assert_ne!(rewritten, first);
    remote.publish();
    fetch.fetch(&mirror, &remote.file_url(), &hosts).unwrap();

    assert_eq!(
        GitObjects::new().resolve_commit(&mirror, "main").unwrap(),
        rewritten
    );
}

#[test]
fn a_remote_that_is_not_a_repository_fails_with_gits_own_words() {
    let root = TempRoot::new();
    let empty = root.join("empty");
    fs::create_dir(&empty).unwrap();
    let error = real_fetch(&root)
        .fetch(
            &mirror_in(&root),
            empty.to_str().unwrap(),
            &KnownHosts::for_home(root.path()),
        )
        .unwrap_err();
    assert!(matches!(error, FetchError::Failed { .. }), "{error:?}");
    assert!(error.to_string().contains("git repository"), "{error}");
}

// ---- the new-host question ----

/// A `git` that records that it ran, in `ran`, and fails.
fn recording_git(root: &TempRoot, dir_name: &str) -> (OsString, PathBuf) {
    let dir = root.join(dir_name);
    fs::create_dir(&dir).unwrap();
    let ran = root.join(&format!("{dir_name}-ran"));
    let body = format!("#!/bin/sh\necho ran >> '{}'\nexit 1\n", ran.display());
    fs::write(dir.join("git"), body).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dir.join("git"), fs::Permissions::from_mode(0o755)).unwrap();
    (dir.into_os_string(), ran)
}

#[test]
fn a_url_with_a_host_that_was_never_fetched_from_needs_approval_and_runs_nothing() {
    let root = TempRoot::new();
    let (path, ran) = recording_git(&root, "fake");
    let mirror = mirror_in(&root);
    let fetch = GitFetch::new(env_of(&root, path));
    let hosts = KnownHosts::for_home(root.path());
    for (url, host) in [
        ("https://example.invalid/o/r", "example.invalid"),
        ("ssh://git@Example.Invalid:2222/o/r", "example.invalid"),
        ("git@example.invalid:o/r", "example.invalid"),
    ] {
        let error = fetch.fetch(&mirror, url, &hosts).unwrap_err();
        match error {
            FetchError::NeedsApproval { host: found } => assert_eq!(found, host, "{url}"),
            other => panic!("{url}: {other:?}"),
        }
    }
    assert!(!ran.exists(), "git ran");
    assert!(!mirror.exists(), "the mirror was made");
    assert!(!hosts.file().exists(), "a host was recorded");
}

#[test]
fn a_host_once_recorded_is_fetched_from_without_asking() {
    let root = TempRoot::new();
    let (path, ran) = recording_git(&root, "fake");
    let mirror = mirror_in(&root);
    new_bare(&root, &mirror);
    let hosts = KnownHosts::for_home(root.path());
    hosts.record("example.invalid").unwrap();
    let error = GitFetch::new(env_of(&root, path))
        .fetch(&mirror, "https://example.invalid/o/r", &hosts)
        .unwrap_err();
    assert!(matches!(error, FetchError::Failed { .. }), "{error:?}");
    assert!(ran.exists(), "git ran");
}

#[test]
fn a_host_that_cannot_be_read_from_the_hosts_file_is_an_error_naming_the_file_and_runs_nothing() {
    let root = TempRoot::new();
    let (path, ran) = recording_git(&root, "fake");
    let hosts = KnownHosts::for_home(root.path());
    fs::create_dir_all(hosts.file().parent().unwrap()).unwrap();
    fs::write(hosts.file(), "hosts: [\n").unwrap();
    let error = GitFetch::new(env_of(&root, path))
        .fetch(&mirror_in(&root), "https://example.invalid/o/r", &hosts)
        .unwrap_err();
    assert!(error.to_string().contains("hosts.yaml"), "{error}");
    assert!(!ran.exists());
}

// ---- how git is run ----

/// A `git` that writes its arguments (one per line) to `args` and its environment to `env`, and ends well.
fn tracing_git(root: &TempRoot) -> (OsString, PathBuf, PathBuf) {
    let dir = root.join("trace");
    fs::create_dir(&dir).unwrap();
    let args = root.join("trace-args");
    let env = root.join("trace-env");
    let body = format!(
        "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\n/usr/bin/env > '{}'\nexit 0\n",
        args.display(),
        env.display()
    );
    fs::write(dir.join("git"), body).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(dir.join("git"), fs::Permissions::from_mode(0o755)).unwrap();
    (dir.into_os_string(), args, env)
}

fn lines(path: &Path) -> Vec<String> {
    fs::read_to_string(path)
        .unwrap()
        .lines()
        .map(str::to_string)
        .collect()
}

const URL: &str = "https://example.invalid/o/r";

fn traced_fetch(root: &TempRoot, env: FetchEnv) -> (Vec<String>, Vec<String>) {
    let (path, args, envfile) = tracing_git(root);
    let mirror = mirror_in(root);
    new_bare(root, &mirror);
    let hosts = KnownHosts::for_home(root.path());
    hosts.record("example.invalid").unwrap();
    GitFetch::new(FetchEnv { path, ..env })
        .fetch(&mirror, URL, &hosts)
        .unwrap();
    (lines(&args), lines(&envfile))
}

#[test]
fn the_url_comes_after_a_double_dash_and_the_refspecs_after_the_url() {
    let root = TempRoot::new();
    let (args, _) = traced_fetch(&root, env_of(&root, real_path()));
    let dashes = args.iter().position(|arg| arg == "--").expect("a `--`");
    assert_eq!(args[dashes + 1], URL);
    assert!(args[..dashes].contains(&"fetch".to_string()), "{args:?}");
    assert_eq!(
        &args[dashes + 2..dashes + 5],
        [
            "+refs/heads/*:refs/heads/*",
            "+refs/tags/*:refs/tags/*",
            "+HEAD:refs/collide/head"
        ]
    );
    assert!(!args.contains(&"--prune".to_string()), "{args:?}");
}

#[test]
fn git_is_told_never_to_ask_on_a_terminal_and_to_use_only_https_ssh_and_file() {
    let root = TempRoot::new();
    let (_, env) = traced_fetch(&root, env_of(&root, real_path()));
    assert!(
        env.contains(&"GIT_TERMINAL_PROMPT=0".to_string()),
        "{env:?}"
    );
    assert!(
        env.contains(&"GIT_ALLOW_PROTOCOL=https:ssh:file".to_string()),
        "{env:?}"
    );
}

#[test]
fn git_gets_the_fetchs_own_home_and_path_and_the_ssh_agent_and_config_home_only_when_given() {
    let root = TempRoot::new();
    let given = FetchEnv {
        ssh_auth_sock: Some("/tmp/agent.sock".into()),
        xdg_config_home: Some("/tmp/xdg".into()),
        ..env_of(&root, real_path())
    };
    let (_, env) = traced_fetch(&root, given);
    assert!(
        env.contains(&format!("HOME={}", root.path().display())),
        "{env:?}"
    );
    assert!(
        env.contains(&"SSH_AUTH_SOCK=/tmp/agent.sock".to_string()),
        "{env:?}"
    );
    assert!(
        env.contains(&"XDG_CONFIG_HOME=/tmp/xdg".to_string()),
        "{env:?}"
    );

    let root = TempRoot::new();
    let (_, env) = traced_fetch(&root, env_of(&root, real_path()));
    assert!(
        !env.iter().any(|line| line.starts_with("SSH_AUTH_SOCK=")),
        "{env:?}"
    );
    assert!(
        !env.iter().any(|line| line.starts_with("XDG_CONFIG_HOME=")),
        "{env:?}"
    );
}

#[test]
fn nothing_else_of_the_callers_environment_reaches_git() {
    let root = TempRoot::new();
    let (_, env) = traced_fetch(&root, env_of(&root, real_path()));
    const ALLOWED: [&str; 12] = [
        "PATH",
        "HOME",
        "SSH_AUTH_SOCK",
        "XDG_CONFIG_HOME",
        "GIT_TERMINAL_PROMPT",
        "GIT_ALLOW_PROTOCOL",
        "GIT_DIR",
        "LC_ALL",
        "GIT_CONFIG_NOSYSTEM",
        "PWD",
        "SHLVL",
        "_",
    ];
    for line in &env {
        let name = line.split('=').next().unwrap();
        assert!(ALLOWED.contains(&name), "{name} reached git: {env:?}");
    }
    for never in [
        "GIT_SSH_COMMAND",
        "GIT_ASKPASS",
        "GIT_EXEC_PATH",
        "SSH_ASKPASS",
        "GIT_PROXY_COMMAND",
    ] {
        assert!(
            !env.iter()
                .any(|line| line.starts_with(&format!("{never}="))),
            "{never}"
        );
    }
}

#[test]
fn a_tilde_in_the_url_is_the_fetchs_home() {
    let root = TempRoot::new();
    let remote = Remote::new();
    remote.work.write("a.txt", b"a");
    let commit = remote.work.commit_all("first");
    remote.publish();
    // The remote is moved under the fetch's home, so `~/` finds it there and nowhere else.
    fs::rename(remote.path(), root.join("remote.git")).unwrap();
    let mirror = mirror_in(&root);
    real_fetch(&root)
        .fetch(&mirror, "~/remote.git", &KnownHosts::for_home(root.path()))
        .unwrap();
    assert_eq!(kind(&mirror, &commit), Some(ObjectKind::Commit));
}

#[test]
fn a_hung_fetch_is_ended_at_the_timeout_and_the_error_names_the_url_and_the_time() {
    let root = TempRoot::new();
    let dir = root.join("hang");
    fs::create_dir(&dir).unwrap();
    // `exec`, so the kill reaches the process that waits and no child of the script is left.
    root.script("hang/git", "#!/bin/sh\nexec /bin/sleep 30\n");
    let mirror = mirror_in(&root);
    new_bare(&root, &mirror);
    let hosts = KnownHosts::for_home(root.path());
    hosts.record("example.invalid").unwrap();
    let fetch =
        GitFetch::new(env_of(&root, dir.into_os_string())).with_timeout(Duration::from_millis(300));
    let started = Instant::now();
    let error = fetch.fetch(&mirror, URL, &hosts).unwrap_err();
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "{:?}",
        started.elapsed()
    );
    assert!(matches!(error, FetchError::Timeout { .. }), "{error:?}");
    let message = error.to_string();
    assert!(message.contains(URL), "{message}");
    assert!(message.contains("0.3"), "{message}");
}

#[test]
fn the_timeout_is_120_seconds() {
    assert_eq!(FETCH_TIMEOUT, Duration::from_secs(120));
}

#[test]
fn a_fetch_that_would_ask_for_a_password_fails_and_says_prompts_are_off() {
    let root = TempRoot::new();
    let dir = root.join("prompt");
    fs::create_dir(&dir).unwrap();
    root.script(
        "prompt/git",
        concat!(
            "#!/bin/sh\n",
            "echo \"fatal: could not read Username for 'https://example.invalid': terminal prompts disabled\" >&2\n",
            "exit 128\n"
        ),
    );
    let mirror = mirror_in(&root);
    new_bare(&root, &mirror);
    let hosts = KnownHosts::for_home(root.path());
    hosts.record("example.invalid").unwrap();
    let error = GitFetch::new(env_of(&root, dir.into_os_string()))
        .fetch(&mirror, URL, &hosts)
        .unwrap_err();
    assert!(matches!(error, FetchError::Failed { .. }), "{error:?}");
    assert!(
        error.to_string().contains("terminal prompts disabled"),
        "{error}"
    );
}

#[test]
fn a_remote_helper_url_is_refused_by_gits_allowlist_even_when_the_users_config_allows_it() {
    let root = TempRoot::new();
    // The test's own home allows `ext::`, as a user's might. The allowlist the fetch sets wins over it.
    fs::write(
        root.join(".gitconfig"),
        "[protocol \"ext\"]\n\tallow = always\n",
    )
    .unwrap();
    let marker = root.join("marker");
    let url = format!("ext::sh -c \"touch {}\"", marker.display());
    let error = real_fetch(&root)
        .fetch(&mirror_in(&root), &url, &KnownHosts::for_home(root.path()))
        .unwrap_err();
    assert!(matches!(error, FetchError::Failed { .. }), "{error:?}");
    assert!(error.to_string().contains("not allowed"), "{error}");
    assert!(!marker.exists(), "the helper ran");
}

#[test]
fn a_url_that_starts_with_a_dash_or_holds_a_nul_or_newline_is_refused_before_git_runs() {
    let root = TempRoot::new();
    let (path, ran) = recording_git(&root, "fake");
    let fetch = GitFetch::new(env_of(&root, path));
    let hosts = KnownHosts::for_home(root.path());
    for url in [
        "--upload-pack=touch /x",
        "-x",
        "https://h/a\nb",
        "https://h/a\0b",
        "",
    ] {
        let error = fetch.fetch(&mirror_in(&root), url, &hosts).unwrap_err();
        assert!(
            matches!(error, FetchError::BadUrl { .. }),
            "{url:?}: {error:?}"
        );
    }
    assert!(!ran.exists());
}

#[test]
fn text_that_is_no_url_is_refused_with_a_hint_and_git_does_not_run() {
    let root = TempRoot::new();
    let (path, ran) = recording_git(&root, "fake");
    let error = GitFetch::new(env_of(&root, path))
        .fetch(
            &mirror_in(&root),
            "github.com/org/ferry-pack",
            &KnownHosts::for_home(root.path()),
        )
        .unwrap_err();
    assert!(matches!(error, FetchError::BadUrl { .. }), "{error:?}");
    assert!(error.to_string().contains("https://"), "{error}");
    assert!(!ran.exists());
}

#[test]
fn git_is_found_on_the_fetchs_path_and_not_the_processs() {
    let root = TempRoot::new();
    let empty = root.join("nothing");
    fs::create_dir(&empty).unwrap();
    assert!(
        find_on_path("git", &real_path()).is_some(),
        "the test needs git"
    );
    let remote = Remote::new();
    remote.work.write("a", b"a");
    remote.work.commit_all("first");
    remote.publish();
    let error = GitFetch::new(env_of(&root, empty.into_os_string()))
        .fetch(
            &mirror_in(&root),
            remote.path().to_str().unwrap(),
            &KnownHosts::for_home(root.path()),
        )
        .unwrap_err();
    assert!(matches!(error, FetchError::Io { .. }), "{error:?}");
}
