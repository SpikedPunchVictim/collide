//! User packs, working copies and `collide pack` (ts-0101-bd; 0.7, Repo packs and user packs, Working copies,
//! Switching packs). A daemon runs in the test's own process over a fixture checkout, every remote is a bare repository
//! on disk in a temp root, and the temp root is the home, so `~/.config/collide` is the test's own. Two users are two
//! fixtures (two homes); two boards are two checkouts under one daemon.

mod common;

use collide_core::owner::{
    BoardOpenParams, BoardOpenResult, ConfigGetParams, ConfigGetResult, ConfigPackAddParams,
    ConfigPackEnableParams, ConfigPackLinkParams, ConfigPackListParams, ConfigPackListResult,
    ConfigPackResult, ConfigPackUpdateParams, PackEntry, PackOrigin, RpcError, TrustState, code,
};
use collide_core::owner::{
    BoardTrustParams, BoardTrustResult, BoardTrustReviewParams, BoardTrustReviewResult,
};
use collide_host::git_objects::GitObjects;
use collide_host::owner_client::ClientError;
use collide_host::settings_files::user_config_dir;
use collide_host::trust::{Trust, TrustStore};
use collide_host::working_copy::WorkingCopies;
use common::config_fixture::Fixture;
use common::git_repo::GitRepo;
use common::remote::Remote;
use serde::Serialize;
use serde::de::DeserializeOwned;
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

const REMOTE_MANIFEST: &str = "apiVersion: collide/v0\nname: remote\nversion: 1.0.0\n\
    requires:\n  agents: [claude]\n\
    gates:\n  lint: { script: hooks/lint.sh }\n\
    providers:\n  tasks: { script: providers/tasks.ts }\n\
    observers:\n  audit: { script: observers/audit.sh, events: [\"run.*\"] }\n";

fn real_path() -> OsString {
    std::env::var_os("PATH").unwrap_or_default()
}

fn manifest(name: &str, extra: &str) -> String {
    format!("apiVersion: collide/v0\nname: {name}\n{extra}")
}

struct World {
    fixture: Fixture,
    board: String,
    remote: Remote,
    /// The source text of the remote's pack, and the commit and tree of its first version.
    source: String,
    commit: String,
    tree: String,
}

/// A remote holding the pack `remote` at `pack/` (tagged `v1`, then changed and tagged `v2`, both published), and a
/// board that lists the repo pack `ferry`.
fn world() -> World {
    let remote = Remote::new();
    remote
        .work
        .write("pack/pack.yaml", REMOTE_MANIFEST.as_bytes());
    remote
        .work
        .write("pack/hooks/lint.sh", b"#!/bin/sh\necho lint\n");
    remote.work.write("pack/providers/tasks.ts", b"export {}\n");
    remote
        .work
        .write("pack/observers/audit.sh", b"#!/bin/sh\ntrue\n");
    let commit = remote.work.commit_all("one");
    let tree = remote.work.git(&["rev-parse", "HEAD:pack"]);
    remote.work.git(&["tag", "v1"]);
    remote.publish();
    let fixture = Fixture::fetching(real_path());
    fixture.write(
        ".collide/board.yaml",
        "packs:\n  - source: repo:.collide/packs/ferry\n",
    );
    fixture.write(
        ".collide/packs/ferry/pack.yaml",
        &manifest("ferry", "version: 1.0.0\n"),
    );
    fixture.write(".collide/packs/ferry/hooks/run.sh", "#!/bin/sh\necho one\n");
    fixture.commit();
    let board = fixture.board();
    let source = format!("git:{}//pack@main", remote.file_url());
    World {
        fixture,
        board,
        remote,
        source,
        commit,
        tree,
    }
}

/// Publishes a second version of the remote's pack, tagged `v2`, and returns its commit and tree.
fn second_version(w: &World) -> (String, String) {
    w.remote.work.write(
        "pack/pack.yaml",
        REMOTE_MANIFEST.replace("1.0.0", "2.0.0").as_bytes(),
    );
    let commit = w.remote.work.commit_all("two");
    let tree = w.remote.work.git(&["rev-parse", "HEAD:pack"]);
    w.remote.work.git(&["tag", "v2"]);
    w.remote.publish();
    (commit, tree)
}

fn rpc<P: Serialize, R: DeserializeOwned>(
    w: &World,
    method: &str,
    params: &P,
) -> Result<R, RpcError> {
    match w
        .fixture
        .client()
        .call(method, params, Some(Duration::from_secs(60)))
    {
        Ok(result) => Ok(result),
        Err(ClientError::Rpc(error)) => Err(error),
        Err(other) => panic!("{other}"),
    }
}

fn add_params(source: &str, confirm: bool) -> ConfigPackAddParams {
    ConfigPackAddParams {
        source: source.to_string(),
        confirm,
        approve_hosts: Vec::new(),
    }
}

fn add(w: &World, source: &str, confirm: bool) -> Result<ConfigPackResult, RpcError> {
    rpc(w, "config.pack_add", &add_params(source, confirm))
}

fn list_on(w: &World, board: &str) -> Result<Vec<PackEntry>, RpcError> {
    let params = ConfigPackListParams {
        board: board.to_string(),
    };
    rpc::<_, ConfigPackListResult>(w, "config.pack_list", &params).map(|result| result.packs)
}

fn list(w: &World) -> Result<Vec<PackEntry>, RpcError> {
    list_on(w, &w.board)
}

fn link_on(
    w: &World,
    board: &str,
    name: &str,
    folder: Option<&Path>,
) -> Result<ConfigPackResult, RpcError> {
    let params = ConfigPackLinkParams {
        board: board.to_string(),
        name: name.to_string(),
        folder: folder.map(|folder| folder.display().to_string()),
    };
    rpc(w, "config.pack_link", &params)
}

fn link(w: &World, name: &str, folder: Option<&Path>) -> Result<ConfigPackResult, RpcError> {
    link_on(w, &w.board, name, folder)
}

fn enable(w: &World, name: &str, enabled: bool) -> Result<ConfigPackResult, RpcError> {
    let params = ConfigPackEnableParams {
        board: w.board.clone(),
        name: name.to_string(),
        enabled,
    };
    rpc(w, "config.pack_enable", &params)
}

fn names(packs: &[PackEntry]) -> Vec<&str> {
    packs.iter().map(|pack| pack.name.as_str()).collect()
}

fn entry<'a>(packs: &'a [PackEntry], name: &str) -> &'a PackEntry {
    packs
        .iter()
        .find(|pack| pack.name == name)
        .unwrap_or_else(|| panic!("no pack {name} in {:?}", names(packs)))
}

fn config_dir(w: &World) -> PathBuf {
    user_config_dir(w.fixture.home())
}

fn board_folder(w: &World) -> PathBuf {
    config_dir(w).join("boards").join(&w.board)
}

fn read(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// A folder with a pack in it, outside the checkout and outside every workspace.
fn pack_folder(w: &World, rel: &str, name: &str) -> PathBuf {
    let folder = w.fixture.home().join(rel);
    fs::create_dir_all(folder.join("hooks")).unwrap();
    fs::write(folder.join("pack.yaml"), manifest(name, "version: 9.0.0\n")).unwrap();
    fs::write(folder.join("hooks/run.sh"), "#!/bin/sh\necho dev\n").unwrap();
    folder
}

fn workspace_folder(w: &World) -> PathBuf {
    w.fixture.home().join("data/workspaces/run-1")
}

fn config_value(w: &World, key: &str) -> Result<Option<serde_json::Value>, RpcError> {
    let params = ConfigGetParams {
        board: Some(w.board.clone()),
        key: Some(key.to_string()),
        profile: None,
        with_problems: false,
    };
    rpc::<_, ConfigGetResult>(w, "config.get", &params).map(|result| {
        result
            .entries
            .into_iter()
            .next()
            .and_then(|entry| entry.value)
    })
}

fn git_write_tree(folder: &Path) -> String {
    let repo = GitRepo::new();
    for entry in walk(folder) {
        let rel = entry.strip_prefix(folder).unwrap();
        repo.write(
            &format!("pack/{}", rel.display()),
            &fs::read(&entry).unwrap(),
        );
    }
    repo.commit_all("same files");
    repo.git(&["rev-parse", "HEAD:pack"])
}

fn walk(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for found in fs::read_dir(folder).unwrap() {
        let path = found.unwrap().path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }
    files.sort();
    files
}

// ---- pack add ----

#[test]
fn add_without_confirm_shows_the_review_and_writes_nothing() {
    let w = world();
    let result = add(&w, &w.source, false).unwrap();
    assert!(!result.written);
    assert_eq!(result.pack.name, "remote");
    assert_eq!(result.pack.origin, PackOrigin::User);
    let review = result.pack.review.expect("the review is shown");
    assert_eq!(review.name, "remote");
    assert_eq!(review.version.as_deref(), Some("1.0.0"));
    assert_eq!(review.commit.as_deref(), Some(w.commit.as_str()));
    assert_eq!(review.tree, w.tree);
    assert_eq!(review.hooks[0].script, "hooks/lint.sh");
    assert_eq!(review.providers[0].script, "providers/tasks.ts");
    assert_eq!(review.observers[0].script, "observers/audit.sh");
    assert_eq!(review.requires, vec!["agent claude".to_string()]);
    assert!(!config_dir(&w).join("packs.yaml").exists());
    assert!(!config_dir(&w).join("packs.lock").exists());
    assert!(!config_dir(&w).join("trust.yaml").exists());
}

#[test]
fn add_with_confirm_writes_packs_yaml_the_lock_and_trust_at_the_tree() {
    let w = world();
    let result = add(&w, &w.source, true).unwrap();
    assert!(result.written);
    let packs = read(&config_dir(&w).join("packs.yaml"));
    assert!(packs.contains(&w.source), "{packs}");
    let lock_file = config_dir(&w).join("packs.lock");
    let lock = read(&lock_file);
    assert!(lock.contains(&w.commit) && lock.contains(&w.tree), "{lock}");
    assert_eq!(
        fs::metadata(&lock_file).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let trust = Trust::new(TrustStore::new(config_dir(&w)));
    assert_eq!(
        trust.pack_state(&w.source, &w.tree),
        TrustState::Trusted {
            hash: w.tree.clone()
        }
    );
}

#[test]
fn a_second_add_of_the_same_source_changes_nothing_and_says_so() {
    let w = world();
    add(&w, &w.source, true).unwrap();
    let before = (
        read(&config_dir(&w).join("packs.yaml")),
        read(&config_dir(&w).join("packs.lock")),
        read(&config_dir(&w).join("trust.yaml")),
    );
    let again = add(&w, &w.source, true).unwrap();
    assert!(!again.written);
    assert!(again.note.unwrap().contains("already"));
    let after = (
        read(&config_dir(&w).join("packs.yaml")),
        read(&config_dir(&w).join("packs.lock")),
        read(&config_dir(&w).join("trust.yaml")),
    );
    assert_eq!(before, after);
}

#[test]
fn a_user_pack_is_off_until_a_board_switches_it_on() {
    let w = world();
    add(&w, &w.source, true).unwrap();
    let packs = list(&w).unwrap();
    assert!(!entry(&packs, "remote").enabled);
    assert_eq!(entry(&packs, "remote").origin, PackOrigin::User);
}

#[test]
fn add_of_a_dev_folder_trusts_the_folder_and_shows_its_review() {
    let w = world();
    let folder = pack_folder(&w, "devs/mine", "mine");
    let source = format!("dev:{}", folder.display());
    let result = add(&w, &source, true).unwrap();
    assert!(result.written);
    assert!(result.pack.dev);
    assert_eq!(result.pack.review.unwrap().commit, None);
    let trust = read(&config_dir(&w).join("trust.yaml"));
    let real = fs::canonicalize(&folder).unwrap();
    assert!(trust.contains(real.to_str().unwrap()), "{trust}");
    assert!(
        !config_dir(&w).join("packs.lock").exists(),
        "a dev: pack is never locked"
    );
}

#[test]
fn add_of_a_pack_with_a_bad_manifest_is_refused_and_writes_nothing() {
    let w = world();
    let bad = Remote::new();
    bad.work.write(
        "pack/pack.yaml",
        b"apiVersion: collide/v0\nname: Not A Token!\n",
    );
    bad.work.commit_all("bad");
    bad.publish();
    let source = format!("git:{}//pack@main", bad.file_url());
    let error = add(&w, &source, true).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("pack.yaml"), "{}", error.message);
    assert!(!config_dir(&w).join("packs.yaml").exists());
}

#[test]
fn add_of_a_pack_named_like_a_built_in_is_refused() {
    let w = world();
    let other = Remote::new();
    other
        .work
        .write("pack/pack.yaml", manifest("collide", "").as_bytes());
    other.work.commit_all("clash");
    other.publish();
    let source = format!("git:{}//pack@main", other.file_url());
    let error = add(&w, &source, true).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("built-in"), "{}", error.message);
    assert!(!config_dir(&w).join("packs.yaml").exists());
}

#[test]
fn add_of_a_repo_source_is_refused_naming_board_yaml() {
    let w = world();
    let error = add(&w, "repo:.collide/packs/ferry", true).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("board.yaml"), "{}", error.message);
}

#[test]
fn a_pack_update_to_a_new_tree_is_not_trusted_until_asked_again() {
    let w = world();
    add(&w, &w.source, true).unwrap();
    let (_, new_tree) = second_version(&w);
    let params = ConfigPackUpdateParams {
        board: w.board.clone(),
        name: "remote".into(),
        to: Some("v2".into()),
        confirm: true,
        approve_hosts: Vec::new(),
    };
    let done: collide_core::owner::ConfigPackUpdateResult =
        rpc(&w, "config.pack_update", &params).unwrap();
    assert!(done.written);
    let trust = Trust::new(TrustStore::new(config_dir(&w)));
    assert!(matches!(
        trust.pack_state(&w.source, &new_tree),
        TrustState::Untrusted { .. }
    ));
    assert!(matches!(
        trust.pack_state(&w.source, &w.tree),
        TrustState::Trusted { .. }
    ));
}

// ---- the load: order, clash, workspaces ----

#[test]
fn the_load_order_is_the_boards_packs_then_the_users_enabled_ones() {
    let w = world();
    add(&w, &w.source, true).unwrap();
    enable(&w, "remote", true).unwrap();
    let packs = list(&w).unwrap();
    let order = names(&packs);
    let ferry = order.iter().position(|name| *name == "ferry").unwrap();
    let remote = order.iter().position(|name| *name == "remote").unwrap();
    assert!(ferry < remote, "{order:?}");
    assert_eq!(entry(&packs, "ferry").origin, PackOrigin::Repo);
    assert!(entry(&packs, "remote").enabled);
    assert_eq!(entry(&packs, "remote").source, w.source);
    assert_eq!(entry(&packs, "collide").origin, PackOrigin::Builtin);
}

#[test]
fn a_user_pack_named_like_a_repo_pack_is_a_load_error_naming_both_sources() {
    let w = world();
    let clash = Remote::new();
    clash
        .work
        .write("pack/pack.yaml", manifest("ferry", "").as_bytes());
    clash.work.commit_all("clash");
    clash.publish();
    let source = format!("git:{}//pack@main", clash.file_url());
    add(&w, &source, true).unwrap();
    assert!(
        list(&w).is_ok(),
        "a pack that is off is not loaded, so it does not clash"
    );
    fs::create_dir_all(board_folder(&w)).unwrap();
    fs::write(board_folder(&w).join("packs.yaml"), "on: [ferry]\n").unwrap();
    let error = list(&w).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(
        error.message.contains("repo:.collide/packs/ferry") && error.message.contains(&source),
        "{}",
        error.message
    );
}

#[test]
fn a_dev_folder_inside_a_workspace_is_refused_at_add_naming_both() {
    let w = world();
    let folder = workspace_folder(&w).join("mine");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("pack.yaml"), manifest("mine", "")).unwrap();
    let error = add(&w, &format!("dev:{}", folder.display()), true).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("workspace"), "{}", error.message);
    assert!(error.message.contains("run-1"), "{}", error.message);
    assert!(!config_dir(&w).join("packs.yaml").exists());
}

#[test]
fn a_dev_folder_inside_a_workspace_is_refused_at_load_too() {
    let w = world();
    let folder = workspace_folder(&w).join("mine");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("pack.yaml"), manifest("mine", "")).unwrap();
    w.fixture.write_user(
        "packs.yaml",
        &format!("packs:\n  - source: dev:{}\n", folder.display()),
    );
    fs::create_dir_all(board_folder(&w)).unwrap();
    fs::write(board_folder(&w).join("packs.yaml"), "on: [mine]\n").unwrap();
    let error = list(&w).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("workspace"), "{}", error.message);
    assert!(error.message.contains("mine"), "{}", error.message);
}

/// A user pack `mine` from a `dev:` source written into `packs.yaml` by hand, switched on for the board.
fn hand_written_dev_pack(w: &World) -> PathBuf {
    let folder = pack_folder(w, "devs/mine", "mine");
    w.fixture.write_user(
        "packs.yaml",
        &format!("packs:\n  - source: dev:{}\n", folder.display()),
    );
    fs::create_dir_all(board_folder(w)).unwrap();
    fs::write(board_folder(w).join("packs.yaml"), "on: [mine]\n").unwrap();
    fs::canonicalize(folder).unwrap()
}

fn approve_dev(w: &World, folders: &[&Path]) -> Result<BoardTrustResult, RpcError> {
    let params = BoardTrustParams {
        board: w.board.clone(),
        line: "main".to_string(),
        hash: String::new(),
        hosts: Vec::new(),
        dev_folders: folders.iter().map(|f| f.display().to_string()).collect(),
    };
    rpc(w, "board.trust", &params)
}

#[test]
fn a_dev_source_written_by_hand_is_refused_until_it_is_trusted() {
    let w = world();
    let real = hand_written_dev_pack(&w);
    let error = list(&w).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(
        error.message.contains("not been trusted"),
        "{}",
        error.message
    );
    assert!(error.message.contains("collide trust"), "{}", error.message);
    assert!(
        error.message.contains(real.to_str().unwrap()),
        "{}",
        error.message
    );
    approve_dev(&w, &[&real]).unwrap();
    assert!(names(&list(&w).unwrap()).contains(&"mine"));
}

#[test]
fn the_review_names_the_dev_folder_and_hashes_nothing_until_it_is_trusted() {
    let w = world();
    let real = hand_written_dev_pack(&w);
    let params = BoardTrustReviewParams {
        board: w.board.clone(),
        line: "main".to_string(),
    };
    let shown: BoardTrustReviewResult = rpc(&w, "board.trust_review", &params).unwrap();
    assert_eq!(shown.dev_folders, vec![real.display().to_string()]);
    assert_eq!(
        shown.trust,
        TrustState::Untrusted {
            hash: String::new()
        }
    );
    assert!(
        shown
            .problems
            .iter()
            .any(|problem| problem.contains("not been trusted"))
    );
    approve_dev(&w, &[&real]).unwrap();
    let shown: BoardTrustReviewResult = rpc(&w, "board.trust_review", &params).unwrap();
    assert!(shown.dev_folders.is_empty() && shown.problems.is_empty());
    assert!(matches!(shown.trust, TrustState::Untrusted { hash } if hash.len() == 40));
}

#[test]
fn approving_a_folder_the_line_does_not_need_trusts_nothing() {
    let w = world();
    let real = hand_written_dev_pack(&w);
    let other = pack_folder(&w, "devs/other", "other");
    approve_dev(&w, &[&fs::canonicalize(other).unwrap()]).unwrap();
    assert!(list(&w).is_err());
    let trust = fs::read_to_string(config_dir(&w).join("trust.yaml")).unwrap_or_default();
    assert!(!trust.contains(real.to_str().unwrap()), "{trust}");
}

#[test]
fn a_user_pack_that_cannot_be_read_does_not_break_a_board_that_has_it_off() {
    // A pack whose name cannot be read without loading it (here its folder is gone) is loaded only on a board that
    // switched on a name no other pack of the user's has: one broken pack must not fail every board.
    let w = world();
    let gone = w.fixture.home().join("devs/gone");
    w.fixture.write_user(
        "packs.yaml",
        &format!("packs:\n  - source: dev:{}\n", gone.display()),
    );
    let packs = list(&w).unwrap();
    assert!(names(&packs).contains(&"ferry"), "{packs:?}");
    fs::create_dir_all(board_folder(&w)).unwrap();
    fs::write(board_folder(&w).join("packs.yaml"), "on: [gone]\n").unwrap();
    let error = list(&w).unwrap_err();
    assert!(error.message.contains("devs/gone"), "{}", error.message);
}

#[test]
fn a_dev_pack_is_hashed_at_every_load_and_its_tree_id_is_git_write_tree() {
    use collide_host::board_load::{LoadContext, load_line};
    use collide_host::builtin_packs::{BuiltinPacks, Environment};
    use collide_host::user_packs::UserScope;
    let w = world();
    let folder = pack_folder(&w, "devs/mine", "mine");
    w.fixture.write_user(
        "packs.yaml",
        &format!("packs:\n  - source: dev:{}\n", folder.display()),
    );
    fs::create_dir_all(board_folder(&w)).unwrap();
    fs::write(board_folder(&w).join("packs.yaml"), "on: [mine]\n").unwrap();
    let home = w.fixture.home();
    let context = LoadContext::new(
        home.join("cache"),
        home.join("sdk-copy"),
        Environment { path: real_path() },
        BuiltinPacks::new(common::repo_root().join("packs")),
    )
    .with_user(UserScope::new(
        home.to_path_buf(),
        w.board.clone(),
        Vec::new(),
    ));
    // The folder is the person's own and has been trusted (an edit after that needs no new trust).
    approve_dev(&w, &[&fs::canonicalize(&folder).unwrap()]).unwrap();
    let git_dir = w.fixture.repo.git_dir();
    let first = load_line(&git_dir, "HEAD", &context).unwrap();
    let mine = first.packs.iter().find(|pack| pack.name == "mine").unwrap();
    assert!(mine.dev);
    assert_eq!(mine.pin.as_deref(), Some(git_write_tree(&folder).as_str()));
    // An edit is seen at the next load, with no commit and no cache.
    fs::write(folder.join("hooks/run.sh"), "#!/bin/sh\necho edited\n").unwrap();
    let second = load_line(&git_dir, "HEAD", &context).unwrap();
    let mine = second
        .packs
        .iter()
        .find(|pack| pack.name == "mine")
        .unwrap();
    assert_eq!(mine.pin.as_deref(), Some(git_write_tree(&folder).as_str()));
    assert_ne!(
        first.packs.last().unwrap().pin,
        second.packs.last().unwrap().pin
    );
    // A user's pack is not an input of the repo's trust hash.
    assert_eq!(first.trust_inputs(), second.trust_inputs());
    assert!(
        !first
            .trust_inputs()
            .iter()
            .any(|(name, _)| name.contains("dev:"))
    );
}

// ---- pack link ----

#[test]
fn link_with_a_folder_overrides_the_repo_pack_in_place_and_marks_it_dev() {
    let w = world();
    let before = list(&w).unwrap();
    let folder = pack_folder(&w, "devs/ferry", "ferry");
    let result = link(&w, "ferry", Some(&folder)).unwrap();
    assert!(result.written);
    let after = list(&w).unwrap();
    assert_eq!(names(&before), names(&after), "replaced in place");
    let ferry = entry(&after, "ferry");
    assert!(ferry.dev);
    assert!(ferry.source.starts_with("dev:"), "{}", ferry.source);
    assert!(!entry(&before, "ferry").dev);
    let links = read(&board_folder(&w).join("packs.yaml"));
    assert!(links.contains("ferry"), "{links}");
}

#[test]
fn a_link_is_this_boards_and_this_users_alone() {
    let w = world();
    let folder = pack_folder(&w, "devs/ferry", "ferry");
    link(&w, "ferry", Some(&folder)).unwrap();
    // The same user, another board: a second checkout that lists the same repo pack.
    let other = GitRepo::new();
    other.write(
        ".collide/board.yaml",
        b"packs:\n  - source: repo:.collide/packs/ferry\n",
    );
    other.write(
        ".collide/packs/ferry/pack.yaml",
        manifest("ferry", "").as_bytes(),
    );
    other.commit_all("board");
    let opened: BoardOpenResult = rpc(
        &w,
        "board.open",
        &BoardOpenParams {
            root: fs::canonicalize(other.work())
                .unwrap()
                .to_str()
                .unwrap()
                .to_string(),
            init_git: false,
        },
    )
    .unwrap();
    let other_board = opened.board.unwrap().id;
    assert_ne!(other_board, w.board);
    let there = list_on(&w, &other_board).unwrap();
    assert!(!entry(&there, "ferry").dev, "another board");
    // Another user: a second home, a second daemon.
    let someone = world();
    assert!(
        !entry(&list(&someone).unwrap(), "ferry").dev,
        "another user"
    );
    assert!(entry(&list(&w).unwrap(), "ferry").dev);
}

#[test]
fn link_with_no_folder_runs_the_uncommitted_folder_of_the_checkout() {
    let w = world();
    w.fixture.write(
        ".collide/packs/ferry/pack.yaml",
        &manifest(
            "ferry",
            "version: 1.0.0\nconfig:\n  knob: { type: string, default: edited }\n",
        ),
    );
    assert!(
        config_value(&w, "ferry.knob").is_err(),
        "the commit has no such setting"
    );
    let result = link(&w, "ferry", None).unwrap();
    assert!(result.written);
    assert!(entry(&list(&w).unwrap(), "ferry").dev);
    assert_eq!(
        config_value(&w, "ferry.knob").unwrap(),
        Some(serde_json::json!("edited"))
    );
}

#[test]
fn link_with_no_folder_uses_the_repo_packs_own_path_when_the_collide_dir_is_a_pointer() {
    let w = world();
    w.fixture.repo.git(&["rm", "-r", "-q", ".collide"]);
    w.fixture.write(".collide", "dir: config/collide\n");
    w.fixture.write(
        "config/collide/board.yaml",
        "packs:\n  - source: repo:config/collide/packs/ferry\n",
    );
    w.fixture.write(
        "config/collide/packs/ferry/pack.yaml",
        &manifest("ferry", "version: 1.0.0\n"),
    );
    w.fixture.commit();
    w.fixture.write(
        "config/collide/packs/ferry/pack.yaml",
        &manifest("ferry", "version: 1.1.0\n"),
    );
    link(&w, "ferry", None).unwrap();
    let links = read(&board_folder(&w).join("packs.yaml"));
    assert!(links.contains("config/collide/packs/ferry"), "{links}");
}

#[test]
fn link_with_no_folder_offers_a_working_copy_found_through_the_pointer() {
    let w = world();
    w.fixture.repo.git(&["rm", "-r", "-q", ".collide"]);
    w.fixture.write(".collide", "dir: config/collide\n");
    w.fixture
        .write("config/collide/board.yaml", "workflow: x/y\n");
    w.fixture.commit();
    w.fixture.write(
        "config/collide/packs/scratch/pack.yaml",
        &manifest("scratch", ""),
    );
    let packs = list(&w).unwrap();
    assert_eq!(entry(&packs, "scratch").origin, PackOrigin::Offered);
    let linked = link(&w, "scratch", None).unwrap();
    assert!(linked.pack.dev);
}

#[test]
fn link_refuses_a_folder_that_is_not_a_pack() {
    let w = world();
    let folder = w.fixture.home().join("devs/empty");
    fs::create_dir_all(&folder).unwrap();
    let error = link(&w, "ferry", Some(&folder)).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("pack.yaml"), "{}", error.message);
    assert!(!board_folder(&w).join("packs.yaml").exists());
}

#[test]
fn link_refuses_a_folder_whose_manifest_name_differs() {
    let w = world();
    let folder = pack_folder(&w, "devs/other", "other");
    let error = link(&w, "ferry", Some(&folder)).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(
        error.message.contains("other") && error.message.contains("ferry"),
        "{}",
        error.message
    );
    assert!(!board_folder(&w).join("packs.yaml").exists());
}

#[test]
fn link_refuses_a_folder_inside_a_workspace() {
    let w = world();
    let folder = workspace_folder(&w).join("ferry");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("pack.yaml"), manifest("ferry", "")).unwrap();
    let error = link(&w, "ferry", Some(&folder)).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("workspace"), "{}", error.message);
    assert!(!board_folder(&w).join("packs.yaml").exists());
}

#[test]
fn link_of_a_name_the_board_does_not_have_and_no_working_copy_offers_is_not_found() {
    let w = world();
    let error = link(&w, "nothing-by-this-name", None).unwrap_err();
    assert_eq!(error.code, code::NOT_FOUND);
    assert!(
        error.message.contains("nothing-by-this-name"),
        "{}",
        error.message
    );
}

// ---- working copies ----

fn offer_scratch(w: &World) -> PathBuf {
    let folder = w.fixture.repo.work().join(".collide/packs/scratch");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("pack.yaml"), manifest("scratch", "")).unwrap();
    folder
}

#[test]
fn a_new_packs_folder_is_offered_and_is_in_no_other_list_until_linked() {
    let w = world();
    let folder = offer_scratch(&w);
    let packs = list(&w).unwrap();
    let scratch = entry(&packs, "scratch");
    assert_eq!(scratch.origin, PackOrigin::Offered);
    assert!(!scratch.enabled && !scratch.dev);
    assert!(scratch.source.contains("scratch"), "{}", scratch.source);
    assert_eq!(
        packs.iter().filter(|pack| pack.name == "scratch").count(),
        1
    );
    assert_eq!(config_value(&w, "scratch.knob").ok().flatten(), None);

    let linked = link(&w, "scratch", None).unwrap();
    assert!(linked.pack.dev);
    let after = list(&w).unwrap();
    let scratch = entry(&after, "scratch");
    assert_eq!(scratch.origin, PackOrigin::User);
    assert!(scratch.dev);
    assert_eq!(
        after.iter().filter(|pack| pack.name == "scratch").count(),
        1
    );
    assert!(folder.exists());
}

#[test]
fn a_packs_folder_that_board_yaml_lists_is_not_offered() {
    let w = world();
    let packs = list(&w).unwrap();
    assert_eq!(entry(&packs, "ferry").origin, PackOrigin::Repo);
    assert!(packs.iter().all(|pack| pack.origin != PackOrigin::Offered));
}

#[test]
fn a_repo_pack_whose_folder_differs_from_the_ref_is_marked_as_having_a_working_copy() {
    let w = world();
    assert!(!entry(&list(&w).unwrap(), "ferry").working_copy);
    w.fixture.write(
        ".collide/packs/ferry/hooks/run.sh",
        "#!/bin/sh\necho edited\n",
    );
    let packs = list(&w).unwrap();
    assert!(entry(&packs, "ferry").working_copy);
    assert!(!entry(&packs, "ferry").dev, "marked, not linked");
}

fn wc_for(w: &World) -> WorkingCopies {
    WorkingCopies::new(
        TrustStore::new(config_dir(w)),
        GitObjects::new(),
        w.fixture.home().join("cache/working-copies.git"),
    )
}

#[test]
fn an_integrate_that_changed_a_linked_folder_waits_for_an_acknowledgement_with_its_diff() {
    let w = world();
    let folder = pack_folder(&w, "devs/ferry", "ferry");
    link(&w, "ferry", Some(&folder)).unwrap();
    assert!(list(&w).is_ok(), "linking is seeing it");
    fs::write(folder.join("hooks/run.sh"), "#!/bin/sh\necho integrated\n").unwrap();
    let copies = wc_for(&w);
    copies.note_integrated(&folder).unwrap();
    let error = list(&w).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("hooks/run.sh"), "{}", error.message);
    assert!(
        error.message.contains("+echo integrated"),
        "{}",
        error.message
    );
    assert!(error.message.contains("acknowledg"), "{}", error.message);

    let waiting = copies.waiting(&folder).unwrap().expect("one is waiting");
    assert!(copies.acknowledge(&folder, &waiting.integrated).unwrap());
    assert!(list(&w).is_ok());
}

#[test]
fn an_acknowledgement_clears_the_tree_it_names_and_not_a_later_one() {
    let w = world();
    let folder = pack_folder(&w, "devs/ferry", "ferry");
    link(&w, "ferry", Some(&folder)).unwrap();
    let copies = wc_for(&w);
    fs::write(folder.join("hooks/run.sh"), "#!/bin/sh\necho two\n").unwrap();
    copies.note_integrated(&folder).unwrap();
    let first = copies.waiting(&folder).unwrap().unwrap().integrated;
    fs::write(folder.join("hooks/run.sh"), "#!/bin/sh\necho three\n").unwrap();
    copies.note_integrated(&folder).unwrap();
    assert!(!copies.acknowledge(&folder, &first).unwrap());
    assert!(list(&w).is_err(), "the later tree still waits");
    let second = copies.waiting(&folder).unwrap().unwrap().integrated;
    assert!(copies.acknowledge(&folder, &second).unwrap());
    assert!(list(&w).is_ok());
}

// ---- enable and disable ----

#[test]
fn enable_and_disable_change_the_boards_folder_only() {
    let w = world();
    add(&w, &w.source, true).unwrap();
    let global = (
        read(&config_dir(&w).join("packs.yaml")),
        read(&config_dir(&w).join("packs.lock")),
    );
    let on = enable(&w, "remote", true).unwrap();
    assert!(on.written && on.pack.enabled);
    assert!(read(&board_folder(&w).join("packs.yaml")).contains("remote"));
    let again = enable(&w, "remote", true).unwrap();
    assert!(!again.written);
    let off = enable(&w, "remote", false).unwrap();
    assert!(off.written && !off.pack.enabled);
    assert!(!read(&board_folder(&w).join("packs.yaml")).contains("remote"));
    assert_eq!(
        global,
        (
            read(&config_dir(&w).join("packs.yaml")),
            read(&config_dir(&w).join("packs.lock"))
        )
    );
}

#[test]
fn enable_of_a_repo_pack_is_refused_naming_board_yaml() {
    let w = world();
    for enabled in [true, false] {
        let error = enable(&w, "ferry", enabled).unwrap_err();
        assert_eq!(error.code, code::CONFLICT);
        assert!(error.message.contains("board.yaml"), "{}", error.message);
    }
    assert!(!board_folder(&w).join("packs.yaml").exists());
}

#[test]
fn enable_of_a_built_in_pack_is_refused_and_of_an_unknown_name_is_not_found() {
    let w = world();
    let error = enable(&w, "collide", true).unwrap_err();
    assert_eq!(error.code, code::CONFLICT);
    assert!(error.message.contains("built-in"), "{}", error.message);
    let error = enable(&w, "nothing-by-this-name", true).unwrap_err();
    assert_eq!(error.code, code::NOT_FOUND);
}

// ---- pack list ----

#[test]
fn pack_list_gives_source_origin_enabled_and_the_dev_mark() {
    let w = world();
    add(&w, &w.source, true).unwrap();
    enable(&w, "remote", true).unwrap();
    let folder = pack_folder(&w, "devs/ferry", "ferry");
    link(&w, "ferry", Some(&folder)).unwrap();
    offer_scratch(&w);
    let packs = list(&w).unwrap();
    let ferry = entry(&packs, "ferry");
    assert_eq!(
        (ferry.origin, ferry.enabled, ferry.dev),
        (PackOrigin::Repo, true, true)
    );
    let remote = entry(&packs, "remote");
    assert_eq!(
        (remote.origin, remote.enabled, remote.dev),
        (PackOrigin::User, true, false)
    );
    assert_eq!(remote.source, w.source);
    assert_eq!(entry(&packs, "scratch").origin, PackOrigin::Offered);
    assert_eq!(entry(&packs, "collide").source, "builtin");
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

    /// Whether the line of `list` that starts with `name` has `word` as a column.
    fn has_word(out: &str, name: &str, word: &str) -> bool {
        out.lines()
            .find(|line| line.split_whitespace().next() == Some(name))
            .is_some_and(|line| line.split_whitespace().any(|column| column == word))
    }

    #[test]
    fn add_prints_the_review_asks_and_a_yes_writes() {
        let w = world();
        let (code, out, err) = pack(&w, &["add", &w.source], "y\n");
        assert_eq!((code, err.as_str()), (exit::OK, ""));
        assert!(out.contains("Pack remote 1.0.0"), "{out}");
        assert!(out.contains(&w.commit), "{out}");
        assert!(out.contains("gates.lint: hooks/lint.sh"), "{out}");
        assert!(out.contains("added"), "{out}");
        assert!(config_dir(&w).join("packs.yaml").exists());
    }

    #[test]
    fn add_with_a_no_writes_nothing() {
        let w = world();
        for input in ["n\n", "", "\n", "what\n"] {
            let (code, out, _) = pack(&w, &["add", &w.source], input);
            assert_eq!(code, exit::OK, "{input:?}");
            assert!(out.contains("nothing was written"), "{out}");
        }
        assert!(!config_dir(&w).join("packs.yaml").exists());
    }

    #[test]
    fn add_with_yes_writes_and_a_second_add_says_it_is_already_there() {
        let w = world();
        let (code, _, _) = pack(&w, &["add", &w.source, "--yes"], "");
        assert_eq!(code, exit::OK);
        let (code, out, _) = pack(&w, &["add", &w.source, "--yes"], "");
        assert_eq!(code, exit::OK);
        assert!(out.contains("already"), "{out}");
    }

    #[test]
    fn link_enable_disable_and_list_work_from_the_command_line() {
        let w = world();
        pack(&w, &["add", &w.source, "--yes"], "");
        let (code, out, err) = pack(&w, &["enable", "remote"], "");
        assert_eq!((code, err.as_str()), (exit::OK, ""));
        assert!(out.contains("remote"), "{out}");
        let folder = pack_folder(&w, "devs/ferry", "ferry");
        let (code, _, err) = pack(&w, &["link", "ferry", folder.to_str().unwrap()], "");
        assert_eq!((code, err.as_str()), (exit::OK, ""));
        let (code, out, _) = pack(&w, &["list"], "");
        assert_eq!(code, exit::OK);
        assert!(has_word(&out, "ferry", "dev"), "{out}");
        assert!(has_word(&out, "remote", "on"), "{out}");
        let (code, out, _) = pack(&w, &["disable", "remote"], "");
        assert_eq!(code, exit::OK);
        assert!(out.contains("remote"), "{out}");
        let (_, out, _) = pack(&w, &["list"], "");
        assert!(has_word(&out, "remote", "off"), "{out}");
    }

    #[test]
    fn a_refusal_exits_1_with_the_reason_and_a_missing_pack_exits_3() {
        let w = world();
        let (code, out, err) = pack(&w, &["enable", "ferry"], "");
        assert_eq!((code, out.as_str()), (exit::FAILED, ""));
        assert!(err.contains("board.yaml"), "{err}");
        let (code, _, err) = pack(&w, &["enable", "nothing-by-this-name"], "");
        assert_eq!(code, exit::ID);
        assert!(err.contains("nothing-by-this-name"), "{err}");
    }

    #[test]
    fn list_outside_a_checkout_is_refused() {
        let w = world();
        let elsewhere = common::TempRoot::new();
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let args = vec!["list".to_string()];
        let code = run_with(
            w.fixture.daemon.paths(),
            &args,
            elsewhere.path(),
            &mut "".as_bytes(),
            &mut out,
            &mut err,
        );
        assert_ne!(code, exit::OK);
        assert!(!String::from_utf8(err).unwrap().is_empty());
    }

    #[test]
    fn a_bad_command_line_exits_2_with_the_usage_and_starts_no_daemon() {
        let root = common::TempRoot::new();
        let paths = collide_host::daemon::DaemonPaths::under(root.path());
        for args in [
            vec!["add"],
            vec!["add", "a", "b"],
            vec!["add", "a", "--nope"],
            vec!["add", "a", "--allow-host"],
            vec!["add", ""],
            vec!["link"],
            vec!["link", "a", "b", "c"],
            vec!["link", ""],
            vec!["enable"],
            vec!["enable", "a", "b"],
            vec!["disable"],
            vec!["list", "x"],
            vec!["update"],
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

    #[test]
    fn pack_test_says_it_is_a_later_stage() {
        let root = common::TempRoot::new();
        let paths = collide_host::daemon::DaemonPaths::under(root.path());
        let args = vec!["test".to_string()];
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run_with(
            &paths,
            &args,
            root.path(),
            &mut "".as_bytes(),
            &mut out,
            &mut err,
        );
        assert_eq!(code, exit::NOT_BUILT);
        assert!(String::from_utf8(err).unwrap().contains("later stage"));
    }
}
