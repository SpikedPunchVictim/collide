//! Tests for the launch of a loaded pack's provider (ts-0108-dq): the script that runs is the one in the line's
//! snapshot, a `dev:` pack runs its folder, and a script that is not a plain path inside the pack is refused. The
//! providers here are real `node` children (the plan's `node <script>`, types stripped) that answer `initialize` with
//! a marker, so a test can tell which file ran. Every repository is built by `git` in a temp root with a clean
//! environment, and every child gets a cleared environment and a `HOME` of the test's own root.

use collide_core::pack::PackManifest;
use collide_host::board_load::{LoadContext, LoadedPack, PackOrigin, load_line};
use collide_host::builtin_packs::{BuiltinPacks, Environment, ProviderLaunch};
use collide_host::provider_client::ProviderClient;
use collide_host::provider_launch::{ProviderLaunchError, provider_launch};
use std::fs;
use std::path::{Path, PathBuf};

mod common;
use common::fake_provider::base_env;
use common::git_repo::GitRepo;
use common::{TempRoot, node_missing_message, repo_root};

/// A provider that answers every request with an `initialize` result whose `idPattern` is `MARK`.
const PROVIDER: &str = "import { createInterface } from 'node:readline'
const lines = createInterface({ input: process.stdin })
lines.on('line', (line) => {
  const request = JSON.parse(line)
  const result = { scope: 'global', idPattern: 'MARK', shortIdPattern: '^x$', statuses: [], capabilities: {} }
  process.stdout.write(JSON.stringify({ jsonrpc: '2.0', id: request.id, result }) + '\\n')
})
";

fn provider_source(mark: &str) -> String {
    PROVIDER.replace("MARK", mark)
}

fn real_path() -> std::ffi::OsString {
    let path = std::env::var_os("PATH").unwrap_or_default();
    if let Some(message) = node_missing_message(&path) {
        panic!("{message}");
    }
    path
}

fn make_sdk(dir: &Path) {
    fs::create_dir_all(dir.join("dist")).unwrap();
    fs::write(
        dir.join("package.json"),
        "{\"name\":\"@collide/pack-sdk\",\"version\":\"0.1.0\",\"type\":\"module\"}\n",
    )
    .unwrap();
    fs::write(
        dir.join("dist/index.js"),
        "export const name = 'stand-in'\n",
    )
    .unwrap();
}

fn context(root: &TempRoot, builtin: PathBuf) -> LoadContext {
    let sdk = root.join("sdk-copy");
    make_sdk(&sdk);
    LoadContext::new(
        root.join("cache"),
        sdk,
        Environment { path: real_path() },
        BuiltinPacks::new(builtin),
    )
}

fn manifest(name: &str, script: &str) -> String {
    format!(
        "apiVersion: collide/v0\nname: {name}\nproviders:\n  main: {{ script: \"{script}\" }}\n"
    )
}

/// Starts the launch's provider, the way the supervisor would, and returns its `idPattern`.
fn id_pattern_of(launch: ProviderLaunch, root: &TempRoot) -> String {
    let launch = launch.with_env(base_env(root));
    let mut client = ProviderClient::start(&launch, &launch.pack_dir).expect("the provider starts");
    let result = client
        .initialize(&collide_core::provider::InitializeParams {
            config: Default::default(),
            root: "/work".to_string(),
        })
        .expect("the provider answers");
    result.id_pattern
}

/// A repository whose `.collide/packs/mine` has a provider script `providers/main.ts`.
fn repo_with_pack(script: &str) -> GitRepo {
    let repo = GitRepo::new();
    repo.write(
        ".collide/board.yaml",
        b"packs:\n  - source: repo:.collide/packs/mine\n",
    );
    repo.write(
        ".collide/packs/mine/pack.yaml",
        manifest("mine", script).as_bytes(),
    );
    repo.write(
        ".collide/packs/mine/providers/main.ts",
        provider_source("committed").as_bytes(),
    );
    repo.commit_all("a pack with a provider");
    repo
}

/// A pack the test makes by hand, in `dir`, with the manifest text given.
fn pack_in(dir: &Path, manifest_text: &str, origin: PackOrigin, dev: bool) -> LoadedPack {
    let manifest: PackManifest =
        collide_core::yaml::load_yaml(manifest_text.as_bytes(), "pack.yaml").unwrap();
    LoadedPack {
        name: manifest.name.clone(),
        origin,
        source: None,
        pin: None,
        dir: dir.to_path_buf(),
        entry: None,
        manifest,
        dev,
        overrides: None,
    }
}

#[test]
fn a_repo_packs_provider_runs_from_the_snapshot_and_not_the_working_copy() {
    let repo = repo_with_pack("providers/main.ts");
    // Edited after the commit, and never committed.
    repo.write(
        ".collide/packs/mine/providers/main.ts",
        provider_source("edited").as_bytes(),
    );
    let context = context(repo.root(), repo_root().join("packs"));
    let line = load_line(&repo.git_dir(), "main", &context).expect("the line loads");
    let pack = line.packs.iter().find(|pack| pack.name == "mine").unwrap();
    let launch = provider_launch(&context, pack, "main").expect("a launch");
    assert_eq!(launch.pack_dir, pack.dir);
    assert!(
        launch.pack_dir.starts_with(repo.root().join("cache")),
        "the cache's copy"
    );
    assert_eq!(
        launch.args,
        vec![pack.dir.join("providers/main.ts").into_os_string()]
    );
    assert_eq!(launch.program.file_name().unwrap(), "node");
    assert_eq!(id_pattern_of(launch, repo.root()), "committed");
}

#[test]
fn a_dev_pack_runs_its_folder_as_it_is_now() {
    let root = TempRoot::new();
    let folder = root.join("my-pack");
    fs::create_dir_all(folder.join("providers")).unwrap();
    fs::write(folder.join("providers/main.ts"), provider_source("one")).unwrap();
    let pack = pack_in(
        &folder,
        &manifest("mine", "providers/main.ts"),
        PackOrigin::Linked,
        true,
    );
    let context = context(&root, repo_root().join("packs"));
    let launch = provider_launch(&context, &pack, "main").unwrap();
    assert_eq!(launch.pack_dir, folder);
    assert_eq!(
        launch.args,
        vec![folder.join("providers/main.ts").into_os_string()]
    );
    assert_eq!(id_pattern_of(launch, &root), "one");
    fs::write(folder.join("providers/main.ts"), provider_source("two")).unwrap();
    let again = provider_launch(&context, &pack, "main").unwrap();
    assert_eq!(id_pattern_of(again, &root), "two");
}

#[test]
fn a_script_that_climbs_out_is_absolute_or_names_another_scheme_is_refused() {
    let root = TempRoot::new();
    let folder = root.join("pack");
    fs::create_dir_all(&folder).unwrap();
    let context = context(&root, repo_root().join("packs"));
    for script in [
        "../x.ts",
        "providers/../../x.ts",
        "/etc/x.ts",
        "repo:x.ts",
        "./x.ts",
        "",
    ] {
        let pack = pack_in(&folder, &manifest("mine", script), PackOrigin::Repo, false);
        match provider_launch(&context, &pack, "main") {
            Err(ProviderLaunchError::BadScript { pack, script: got }) => {
                assert_eq!((pack.as_str(), got.as_str()), ("mine", script));
            }
            other => panic!("`{script}` should be refused, got {other:?}"),
        }
    }
}

#[test]
fn a_script_that_is_not_in_the_pack_folder_is_refused_naming_the_file() {
    let root = TempRoot::new();
    let folder = root.join("pack");
    fs::create_dir_all(&folder).unwrap();
    let pack = pack_in(
        &folder,
        &manifest("mine", "providers/gone.ts"),
        PackOrigin::Repo,
        false,
    );
    let context = context(&root, repo_root().join("packs"));
    let error = provider_launch(&context, &pack, "main").unwrap_err();
    assert!(
        matches!(error, ProviderLaunchError::MissingScript { .. }),
        "{error:?}"
    );
    let text = error.to_string();
    assert!(
        text.contains("mine") && text.contains("providers/gone.ts"),
        "{text}"
    );
}

#[test]
fn a_provider_the_manifest_does_not_have_is_refused_naming_the_pack_and_the_provider() {
    let root = TempRoot::new();
    let folder = root.join("pack");
    fs::create_dir_all(&folder).unwrap();
    let pack = pack_in(
        &folder,
        &manifest("mine", "providers/main.ts"),
        PackOrigin::Repo,
        false,
    );
    let context = context(&root, repo_root().join("packs"));
    let text = provider_launch(&context, &pack, "other")
        .unwrap_err()
        .to_string();
    assert!(text.contains("mine") && text.contains("`other`"), "{text}");
}

#[test]
fn with_no_node_on_the_path_the_error_names_the_pack_the_provider_and_where_it_looked() {
    let root = TempRoot::new();
    let folder = root.join("pack");
    fs::create_dir_all(folder.join("providers")).unwrap();
    fs::write(folder.join("providers/main.ts"), provider_source("x")).unwrap();
    let pack = pack_in(
        &folder,
        &manifest("mine", "providers/main.ts"),
        PackOrigin::Repo,
        false,
    );
    let empty = root.join("empty-bin");
    fs::create_dir_all(&empty).unwrap();
    let mut context = context(&root, repo_root().join("packs"));
    context.env = Environment {
        path: empty.clone().into_os_string(),
    };
    let text = provider_launch(&context, &pack, "main")
        .unwrap_err()
        .to_string();
    assert_eq!(
        text,
        format!(
            "pack mine needs `node` to start its provider `main`, \
             and `node` was not found on PATH (searched: {})",
            empty.display()
        )
    );
}

#[test]
fn a_built_in_pack_keeps_the_launch_of_the_built_in_loader() {
    let root = TempRoot::new();
    let bundle = repo_root().join("host/tests/fixtures/fake-provider");
    let context = context(&root, bundle.clone());
    let pack = pack_in(
        &bundle.join("plan"),
        "apiVersion: collide/v0\nname: collide/plan\nproviders:\n  plan: { script: src/main.ts }\n",
        PackOrigin::Builtin,
        false,
    );
    let launch = provider_launch(&context, &pack, "plan").unwrap();
    let want = context
        .builtin
        .provider("collide/plan", "plan", &context.env)
        .unwrap();
    assert_eq!(launch, want);
    assert_eq!(
        launch.args,
        vec![bundle.join("plan/src/main.js").into_os_string()]
    );
}

#[test]
fn the_launch_runs_in_the_packs_folder_and_carries_no_environment_until_it_is_given_one() {
    let root = TempRoot::new();
    let folder = root.join("pack");
    fs::create_dir_all(folder.join("providers")).unwrap();
    fs::write(folder.join("providers/main.ts"), provider_source("x")).unwrap();
    let pack = pack_in(
        &folder,
        &manifest("mine", "providers/main.ts"),
        PackOrigin::Repo,
        false,
    );
    let context = context(&root, repo_root().join("packs"));
    let launch = provider_launch(&context, &pack, "main").unwrap();
    assert_eq!(launch.pack_dir, folder);
    assert_eq!(launch.env, None);
    assert_eq!(launch.data, None);
}
