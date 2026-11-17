//! Tests for the board loader (ts-0096-ph). Every repository is built by `git` in a temp root with a clean environment
//! (`common::git_repo`). The cache, the SDK stand-in and the fake tools for `requires.tools` sit in the same root, and
//! the loader gets a `PATH` that holds only that root's `bin` folder. The loader's own git and tool calls clear their
//! environment, so no test reads the real home, the real git config or a real daemon.

use collide_host::board_load::{
    LoadContext, LoadProblem, LoadedLine, LoadedPack, PackOrigin, load_line, pack_data_dir,
};
use collide_host::builtin_packs::{BuiltinPacks, Environment};
use collide_host::git_fetch::{FetchEnv, GitFetch};
use collide_host::hosts::KnownHosts;
use collide_host::pack_cache::{entry_name, materialize};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

mod common;
use common::git_repo::GitRepo;
use common::remote::Remote;
use common::{TempRoot, repo_root};

/// A line's repository with the cache, an SDK stand-in and a `bin` folder for fake tools beside it.
struct Line {
    repo: GitRepo,
    cache: PathBuf,
    sdk: PathBuf,
    bin: PathBuf,
}

fn make_sdk(dir: &Path, version: &str) {
    fs::create_dir_all(dir.join("dist")).unwrap();
    fs::write(
        dir.join("package.json"),
        format!(
            "{{\"name\":\"@collide/pack-sdk\",\"version\":\"{version}\",\"type\":\"module\"}}\n"
        ),
    )
    .unwrap();
    fs::write(
        dir.join("dist/index.js"),
        "export const name = 'stand-in'\n",
    )
    .unwrap();
}

fn fixture(rel: &str) -> String {
    fs::read_to_string(repo_root().join("fixtures").join(rel)).unwrap()
}

fn manifest(name: &str, extra: &str) -> String {
    format!("apiVersion: collide/v0\nname: {name}\n{extra}")
}

const FERRY_BOARD: &str = "packs:\n  - source: repo:.collide/packs/ferry\n";

fn texts(problems: &[LoadProblem]) -> Vec<String> {
    problems.iter().map(ToString::to_string).collect()
}

/// Panics, listing every problem, unless one of them holds all of `parts`.
fn assert_problem(problems: &[String], parts: &[&str]) {
    assert!(
        problems
            .iter()
            .any(|text| parts.iter().all(|part| text.contains(part))),
        "no problem holds {parts:?}; the problems are:\n{}",
        problems.join("\n")
    );
}

impl Line {
    fn new() -> Line {
        let repo = GitRepo::new();
        let cache = repo.root().join("cache");
        let sdk = repo.root().join("sdk-copy");
        let bin = repo.root().join("bin");
        make_sdk(&sdk, "0.1.0");
        fs::create_dir(&bin).unwrap();
        Line {
            repo,
            cache,
            sdk,
            bin,
        }
    }

    /// A board with the `ferry` repo pack at `.collide/packs/ferry`, committed.
    fn basic() -> Line {
        let line = Line::new();
        line.write(".collide/board.yaml", FERRY_BOARD);
        line.write(
            ".collide/packs/ferry/pack.yaml",
            &manifest("ferry", "version: 1.0.0\n"),
        );
        line.write(".collide/packs/ferry/hooks/run.sh", "#!/bin/sh\necho one\n");
        line.commit();
        line
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

    fn load_at(&self, rev: &str) -> Result<LoadedLine, Vec<LoadProblem>> {
        load_line(&self.repo.git_dir(), rev, &self.context())
    }

    fn load(&self) -> Result<LoadedLine, Vec<LoadProblem>> {
        self.load_at("main")
    }

    fn ok(&self) -> LoadedLine {
        self.load().unwrap_or_else(|problems| {
            panic!(
                "the load should work, and had problems:\n{}",
                texts(&problems).join("\n")
            )
        })
    }

    fn problems(&self) -> Vec<String> {
        match self.load() {
            Ok(_) => panic!("the load should have failed"),
            Err(problems) => texts(&problems),
        }
    }

    /// A tool on the loader's `PATH`: a script that runs `body`.
    fn tool(&self, name: &str, body: &str) {
        let path = self.bin.join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    }

    fn rev_parse(&self, spec: &str) -> String {
        self.repo.git(&["rev-parse", spec])
    }
}

fn pack<'a>(line: &'a LoadedLine, name: &str) -> &'a LoadedPack {
    line.packs
        .iter()
        .find(|pack| pack.name == name)
        .unwrap_or_else(|| panic!("no pack named {name}"))
}

fn names(line: &LoadedLine) -> Vec<&str> {
    line.packs.iter().map(|pack| pack.name.as_str()).collect()
}

// ---- snapshot, not workspace ----

#[test]
fn an_uncommitted_edit_to_pack_yaml_is_not_loaded_until_it_is_committed() {
    let line = Line::basic();
    let before = line.ok();
    assert_eq!(
        pack(&before, "ferry").manifest.version.as_deref(),
        Some("1.0.0")
    );

    line.write(
        ".collide/packs/ferry/pack.yaml",
        &manifest("ferry", "version: 2.0.0\n"),
    );
    let edited = line.ok();
    assert_eq!(
        pack(&edited, "ferry").manifest.version.as_deref(),
        Some("1.0.0")
    );
    assert_eq!(edited.trust_inputs(), before.trust_inputs());

    line.commit();
    let committed = line.ok();
    assert_eq!(
        pack(&committed, "ferry").manifest.version.as_deref(),
        Some("2.0.0")
    );
    assert_ne!(committed.trust_inputs(), before.trust_inputs());
}

#[test]
fn an_uncommitted_edit_to_a_hook_is_not_in_the_loaded_pack_until_it_is_committed() {
    let line = Line::basic();
    let hook = |loaded: &LoadedLine| {
        fs::read_to_string(pack(loaded, "ferry").dir.join("hooks/run.sh")).unwrap()
    };
    let before = line.ok();
    assert_eq!(hook(&before), "#!/bin/sh\necho one\n");

    line.write(".collide/packs/ferry/hooks/run.sh", "#!/bin/sh\necho two\n");
    let edited = line.ok();
    assert_eq!(hook(&edited), "#!/bin/sh\necho one\n");
    assert_eq!(edited.trust_inputs(), before.trust_inputs());

    line.commit();
    let committed = line.ok();
    assert_eq!(hook(&committed), "#!/bin/sh\necho two\n");
    assert_ne!(committed.trust_inputs(), before.trust_inputs());
    assert_ne!(
        pack(&committed, "ferry").entry,
        pack(&before, "ferry").entry
    );
}

#[test]
fn an_uncommitted_edit_to_board_yaml_is_not_loaded_until_it_is_committed() {
    let line = Line::basic();
    let before = line.ok();
    assert_eq!(before.board.workflow, None);

    line.write(
        ".collide/board.yaml",
        &format!("{FERRY_BOARD}workflow: ferry/loop\n"),
    );
    let edited = line.ok();
    assert_eq!(edited.board.workflow, None);
    assert_eq!(edited.trust_inputs(), before.trust_inputs());

    line.commit();
    let committed = line.ok();
    assert_eq!(committed.board.workflow.as_deref(), Some("ferry/loop"));
    assert_ne!(committed.trust_inputs(), before.trust_inputs());
}

#[test]
fn an_uncommitted_edit_to_config_yaml_is_not_loaded_until_it_is_committed() {
    let line = Line::basic();
    line.write(".collide/config.yaml", "ask: { budget_usd: 0.5 }\n");
    line.commit();
    let before = line.ok();
    assert_eq!(before.config.ask.budget_usd, Some(0.5));

    line.write(".collide/config.yaml", "ask: { budget_usd: 0.9 }\n");
    let edited = line.ok();
    assert_eq!(edited.config.ask.budget_usd, Some(0.5));
    assert_eq!(edited.trust_inputs(), before.trust_inputs());

    line.commit();
    let committed = line.ok();
    assert_eq!(committed.config.ask.budget_usd, Some(0.9));
    assert_ne!(committed.trust_inputs(), before.trust_inputs());
}

#[test]
fn a_file_deleted_or_added_in_the_working_tree_changes_nothing() {
    let line = Line::basic();
    let before = line.ok();
    fs::remove_file(line.repo.work().join(".collide/board.yaml")).unwrap();
    line.write(".collide/config.yaml", "ask: { budget_usd: 5 }\n");
    let after = line.ok();
    assert_eq!(names(&after), names(&before));
    assert_eq!(after.config.ask.budget_usd, None);
    assert_eq!(after.trust_inputs(), before.trust_inputs());
}

#[test]
fn the_load_reads_the_ref_asked_for_and_not_the_branch_that_is_checked_out() {
    let line = Line::basic();
    line.repo.git(&["checkout", "--quiet", "-b", "other"]);
    line.write(
        ".collide/board.yaml",
        &format!("{FERRY_BOARD}workflow: ferry/other\n"),
    );
    let other_commit = line.commit();
    line.repo.git(&["checkout", "--quiet", "main"]);
    let main = line.ok();
    let other = line.load_at("other").unwrap();
    assert_eq!(main.board.workflow, None);
    assert_eq!(other.board.workflow.as_deref(), Some("ferry/other"));
    assert_eq!(other.commit, other_commit);
    assert_eq!(main.commit, line.rev_parse("main"));
}

#[test]
fn a_ref_that_is_not_there_is_a_problem_naming_the_ref() {
    let line = Line::basic();
    let problems = texts(&line.load_at("no-such-branch").unwrap_err());
    assert_problem(&problems, &["no-such-branch"]);
}

#[test]
fn a_ref_that_starts_with_a_dash_is_refused() {
    let line = Line::basic();
    let problems = texts(&line.load_at("--all").unwrap_err());
    assert_problem(&problems, &["--all"]);
}

// ---- trust inputs ----

#[test]
fn trust_inputs_name_each_file_read_and_each_repo_pack_with_a_git_id() {
    let line = Line::basic();
    line.write(".collide/config.yaml", "ask: { budget_usd: 0.5 }\n");
    line.write(
        ".collide/profiles/small-fix.yaml",
        &fixture("profiles/small-fix.yaml"),
    );
    line.write(".collide/views/mine.yaml", "# a view\n");
    line.commit();
    let loaded = line.ok();
    let inputs = loaded.trust_inputs();
    let id_of = |name: &str| {
        inputs
            .iter()
            .find(|(input, _)| input == name)
            .unwrap_or_else(|| panic!("no input {name} in {inputs:?}"))
            .1
            .clone()
    };
    assert_eq!(
        id_of("board.yaml"),
        line.rev_parse("main:.collide/board.yaml")
    );
    assert_eq!(
        id_of("config.yaml"),
        line.rev_parse("main:.collide/config.yaml")
    );
    assert_eq!(
        id_of("profiles/small-fix.yaml"),
        line.rev_parse("main:.collide/profiles/small-fix.yaml")
    );
    assert_eq!(
        id_of("views/mine.yaml"),
        line.rev_parse("main:.collide/views/mine.yaml")
    );
    assert_eq!(
        id_of("pack repo:.collide/packs/ferry"),
        line.rev_parse("main:.collide/packs/ferry")
    );
    assert!(
        inputs
            .iter()
            .all(|(name, _)| name != "packs.lock" && name != "pointer")
    );
    let mut sorted = inputs.clone();
    sorted.sort();
    assert_eq!(
        inputs, sorted,
        "the inputs are sorted, so the same line gives the same list"
    );
}

#[test]
fn the_same_commit_gives_the_same_trust_inputs_every_time() {
    let line = Line::basic();
    assert_eq!(line.ok().trust_inputs(), line.ok().trust_inputs());
}

#[test]
fn a_committed_change_to_the_lock_or_the_pointer_changes_the_trust_inputs() {
    let line = Line::new();
    line.write("tools/collide/board.yaml", "packs: []\n");
    line.write(".collide", "dir: tools/collide\n");
    line.commit();
    let before = line.ok().trust_inputs();
    assert!(before.iter().any(|(name, _)| name == "pointer"));
    line.write("tools/collide/packs.lock", "packs: []\n");
    line.commit();
    let with_lock = line.ok().trust_inputs();
    assert_ne!(with_lock, before);
    assert!(with_lock.iter().any(|(name, _)| name == "packs.lock"));
    line.write(".collide", "dir: tools/collide\n# a comment\n");
    line.commit();
    assert_ne!(line.ok().trust_inputs(), with_lock);
}

// ---- the Collide directory and the pointer file ----

#[test]
fn a_pointer_file_moves_the_board_profiles_views_and_config_to_its_dir() {
    let line = Line::new();
    line.write(".collide", &fixture("pointer/dot-collide.yaml"));
    line.write(
        "tools/collide/board.yaml",
        "packs:\n  - source: repo:packs/ferry\nworkflow: ferry/loop\n",
    );
    line.write(
        "tools/collide/config.yaml",
        &fixture("config/repo-config.yaml"),
    );
    line.write(
        "tools/collide/profiles/small-fix.yaml",
        &fixture("profiles/small-fix.yaml"),
    );
    line.write("tools/collide/views/mine.yaml", "# a view\n");
    line.write("packs/ferry/pack.yaml", &manifest("ferry", ""));
    line.commit();
    let loaded = line.ok();
    assert_eq!(loaded.collide_dir.as_deref(), Some("tools/collide"));
    assert_eq!(loaded.board.workflow.as_deref(), Some("ferry/loop"));
    assert_eq!(loaded.config.ask.budget_usd, Some(0.5));
    assert!(loaded.profiles.contains_key("small-fix"));
    assert_eq!(loaded.views.len(), 1);
    assert_eq!(loaded.views[0].name, "mine.yaml");
    assert_eq!(
        pack(&loaded, "ferry").source.as_deref(),
        Some("repo:packs/ferry")
    );
}

#[test]
fn a_collide_directory_is_found_at_dot_collide() {
    let line = Line::basic();
    assert_eq!(line.ok().collide_dir.as_deref(), Some(".collide"));
}

#[test]
fn a_pointer_that_cannot_be_followed_is_refused_naming_the_pointer_and_the_dir() {
    // (the pointer file, what the problem says about it)
    let cases = [
        ("dir: ../elsewhere/collide\n", "../elsewhere/collide"),
        ("dir: /etc/collide\n", "/etc/collide"),
        ("dir: tools/not-there\n", "not in the repo"),
        ("dir: tools/a-file.txt\n", "is a file"),
        ("dir: tools/a-link\n", "symlink"),
        ("dir: tools//collide\n", "tools//collide"),
        ("dir: ./tools\n", "./tools"),
        ("dir: \"\"\n", "empty"),
    ];
    for (pointer, says) in cases {
        let line = Line::new();
        line.write(".collide", pointer);
        line.write("tools/a-file.txt", "not a folder\n");
        line.write("tools/collide/board.yaml", "packs: []\n");
        line.repo.symlink("tools/a-link", "collide");
        line.commit();
        let problems = line.problems();
        let dir = pointer.trim_start_matches("dir: ").trim();
        assert_problem(&problems, &[".collide", says]);
        if dir != "\"\"" {
            assert_problem(&problems, &[".collide", dir]);
        }
        assert_eq!(problems.len(), 1, "{pointer}: {problems:?}");
    }
}

#[test]
fn a_pointer_file_that_does_not_parse_is_refused_naming_the_file() {
    for (name, key) in [
        ("pointer/invalid/unknown-key.yaml", "ref"),
        ("pointer/invalid/missing-dir.yaml", "dir"),
    ] {
        let line = Line::new();
        line.write(".collide", &fixture(name));
        line.commit();
        let problems = line.problems();
        assert_problem(&problems, &[".collide", key]);
    }
}

#[test]
fn a_pointer_file_with_an_anchor_is_refused() {
    let line = Line::new();
    line.write(".collide", "dir: &a tools/collide\n");
    line.commit();
    assert_problem(&line.problems(), &[".collide", "anchors"]);
}

#[test]
fn a_dot_collide_that_is_a_symlink_is_refused_naming_it() {
    let line = Line::new();
    line.write("tools/collide/board.yaml", "packs: []\n");
    line.repo.symlink(".collide", "tools/collide");
    line.commit();
    let problems = line.problems();
    assert_problem(&problems, &[".collide", "symlink"]);
    assert_eq!(problems.len(), 1, "{problems:?}");
}

#[test]
fn a_pointer_dir_below_a_symlinked_folder_is_not_followed() {
    let line = Line::new();
    line.write(".collide", "dir: linked/collide\n");
    line.write("real/collide/board.yaml", "packs: []\n");
    line.repo.symlink("linked", "real");
    line.commit();
    assert_problem(
        &line.problems(),
        &[".collide", "linked/collide", "not in the repo"],
    );
}

#[test]
fn a_line_with_no_collide_directory_loads_the_two_built_in_packs_and_no_problems() {
    let line = Line::new();
    line.write("README.md", "# nothing here\n");
    line.commit();
    let loaded = line.ok();
    assert_eq!(loaded.collide_dir, None);
    assert_eq!(names(&loaded), ["collide", "collide/plan"]);
    assert_eq!(loaded.board.packs.len(), 0);
    assert!(loaded.trust_inputs().is_empty());
}

// ---- the files ----

#[test]
fn a_collide_directory_with_no_files_loads_with_the_built_in_packs() {
    let line = Line::new();
    line.write(".collide/README.md", "notes\n");
    line.commit();
    let loaded = line.ok();
    assert_eq!(loaded.collide_dir.as_deref(), Some(".collide"));
    assert_eq!(names(&loaded), ["collide", "collide/plan"]);
    assert!(loaded.trust_inputs().is_empty());
}

#[test]
fn a_file_over_the_yaml_cap_is_refused_naming_the_file() {
    let line = Line::basic();
    line.write(
        ".collide/config.yaml",
        &format!("# {}\n", "x".repeat(300_000)),
    );
    line.commit();
    assert_problem(&line.problems(), &[".collide/config.yaml", "cap"]);
}

#[test]
fn an_anchor_or_a_duplicate_key_in_a_file_is_the_loaders_own_refusal() {
    let line = Line::basic();
    line.write(".collide/board.yaml", "packs: &p []\n");
    line.write(".collide/packs.lock", "packs: []\npacks: []\n");
    line.commit();
    let problems = line.problems();
    assert_problem(&problems, &[".collide/board.yaml", "anchors"]);
    assert_problem(&problems, &[".collide/packs.lock", "duplicate"]);
}

#[test]
fn a_file_that_fails_its_check_is_refused_naming_the_file_and_the_key() {
    // (the file at the ref, its text, what the problem names besides the file)
    let cases: Vec<(&str, String, &str)> = vec![
        (
            ".collide/board.yaml",
            fixture("board/invalid/dev-source.yaml"),
            "dev:",
        ),
        (
            ".collide/board.yaml",
            fixture("board/invalid/unknown-key.yaml"),
            "unknown",
        ),
        (
            ".collide/packs.lock",
            fixture("locks/invalid/repo-source.lock"),
            "not pinned",
        ),
        (
            ".collide/packs.lock",
            fixture("locks/invalid/missing-tree.lock"),
            "tree",
        ),
        (
            ".collide/config.yaml",
            fixture("config/invalid/disk-floor-zero.yaml"),
            "disk.floor_gib",
        ),
        (
            ".collide/config.yaml",
            fixture("config/invalid/repo-sets-agent.yaml"),
            "agents.claude-haiku.model",
        ),
        (
            ".collide/config.yaml",
            fixture("config/invalid/repo-sets-agent.yaml"),
            "ask.agent",
        ),
        (
            ".collide/config.yaml",
            fixture("config/invalid/repo-sets-check-published.yaml"),
            "claude.check_published",
        ),
        (
            ".collide/profiles/bad.yaml",
            fixture("profiles/invalid/compact-at-below-floor.yaml"),
            "compact_at",
        ),
        (
            ".collide/profiles/bad.yaml",
            fixture("profiles/invalid/unknown-key.yaml"),
            "unknown",
        ),
    ];
    for (file, text, names) in cases {
        let line = Line::new();
        line.write(file, &text);
        line.commit();
        let problems = line.problems();
        assert_problem(&problems, &[file, names]);
    }
}

#[test]
fn a_file_that_is_a_symlink_or_a_folder_is_refused_and_not_read() {
    let line = Line::new();
    line.write("elsewhere/board.yaml", "packs: []\n");
    line.repo
        .symlink(".collide/board.yaml", "../elsewhere/board.yaml");
    line.write(
        ".collide/config.yaml/inner.txt",
        "a folder where a file belongs\n",
    );
    line.commit();
    let problems = line.problems();
    assert_problem(&problems, &[".collide/board.yaml", "symlink"]);
    assert_problem(&problems, &[".collide/config.yaml", "folder"]);
}

#[test]
fn profiles_are_loaded_by_file_name_and_views_are_listed_by_name_and_blob_id() {
    let line = Line::basic();
    line.write(
        ".collide/profiles/small-fix.yaml",
        &fixture("profiles/small-fix.yaml"),
    );
    line.write(".collide/profiles/README.md", "not a profile\n");
    line.write(".collide/views/a.yaml", "# first\n");
    line.write(".collide/views/b.yaml", "# second\n");
    line.write(".collide/views/notes.txt", "not a view\n");
    line.commit();
    let loaded = line.ok();
    assert_eq!(loaded.profiles.keys().collect::<Vec<_>>(), ["small-fix"]);
    let views: Vec<(&str, &str)> = loaded
        .views
        .iter()
        .map(|view| (view.name.as_str(), view.id.as_str()))
        .collect();
    assert_eq!(
        views,
        [
            (
                "a.yaml",
                line.rev_parse("main:.collide/views/a.yaml").as_str()
            ),
            (
                "b.yaml",
                line.rev_parse("main:.collide/views/b.yaml").as_str()
            ),
        ]
    );
}

#[test]
fn a_view_is_listed_and_not_parsed() {
    let line = Line::basic();
    line.write(".collide/views/broken.yaml", "this: [is not: a view\n");
    line.commit();
    assert_eq!(line.ok().views.len(), 1);
}

// ---- the built-in packs ----

#[test]
fn the_built_in_packs_come_first_marked_builtin_with_no_pin_and_no_cache_entry() {
    let line = Line::basic();
    let loaded = line.ok();
    assert_eq!(names(&loaded), ["collide", "collide/plan", "ferry"]);
    for builtin in &loaded.packs[..2] {
        assert_eq!(builtin.origin, PackOrigin::Builtin);
        assert_eq!(builtin.source, None);
        assert_eq!(builtin.pin, None);
        assert_eq!(builtin.entry, None);
        assert_eq!(
            builtin.dir,
            repo_root()
                .join("packs")
                .join(if builtin.name == "collide" {
                    "collide"
                } else {
                    "plan"
                })
        );
    }
    assert!(
        !loaded
            .trust_inputs()
            .iter()
            .any(|(name, _)| name.contains("collide/plan") || name == "pack collide"),
        "a built-in pack is trusted with the app"
    );
}

#[test]
fn loading_only_the_built_in_packs_makes_no_cache_entry() {
    let line = Line::new();
    line.write("README.md", "x\n");
    line.commit();
    line.ok();
    assert!(!line.cache.exists(), "the cache folder was made");
}

#[test]
fn the_requires_of_a_built_in_pack_are_not_checked_at_load() {
    // `collide/plan` needs `node`, and the loader's PATH has none: its provider's start says so (ts-0029-ea).
    let line = Line::new();
    line.write("README.md", "x\n");
    line.commit();
    assert!(
        line.ok()
            .packs
            .iter()
            .any(|pack| pack.name == "collide/plan")
    );
}

#[test]
fn a_bundle_with_no_built_in_packs_is_a_problem_naming_the_pack() {
    let line = Line::new();
    line.write("README.md", "x\n");
    line.commit();
    let context = LoadContext::new(
        line.cache.clone(),
        line.sdk.clone(),
        Environment {
            path: line.bin.clone().into_os_string(),
        },
        BuiltinPacks::new(line.repo.root().join("no-bundle")),
    );
    let problems = texts(&load_line(&line.repo.git_dir(), "main", &context).unwrap_err());
    assert_problem(&problems, &["built-in", "collide"]);
}

// ---- repo packs ----

#[test]
fn a_repo_pack_loads_with_its_name_origin_source_pin_and_cache_entry() {
    let line = Line::basic();
    let loaded = line.ok();
    let ferry = pack(&loaded, "ferry");
    let tree = line.rev_parse("main:.collide/packs/ferry");
    assert_eq!(ferry.origin, PackOrigin::Repo);
    assert_eq!(ferry.source.as_deref(), Some("repo:.collide/packs/ferry"));
    assert_eq!(ferry.pin.as_deref(), Some(tree.as_str()));
    let entry = line.cache.join("packs").join(entry_name(&tree, "0.1.0"));
    assert_eq!(ferry.entry.as_deref(), Some(entry.as_path()));
    assert_eq!(ferry.dir, entry.join("pack"));
    assert_eq!(ferry.manifest.version.as_deref(), Some("1.0.0"));
    assert!(ferry.dir.join("hooks/run.sh").is_file());
    assert_eq!(
        loaded
            .trust_inputs()
            .iter()
            .filter(|(name, _)| name.starts_with("pack "))
            .count(),
        1
    );
}

#[test]
fn a_repo_pack_is_loaded_again_from_the_cache_without_writing() {
    let line = Line::basic();
    let first = line.ok();
    let entry = pack(&first, "ferry").entry.clone().unwrap();
    let marker = fs::metadata(entry.join(".collide-verified"))
        .unwrap()
        .modified()
        .unwrap();
    let second = line.ok();
    assert_eq!(
        pack(&second, "ferry").entry.as_deref(),
        Some(entry.as_path())
    );
    assert_eq!(
        fs::metadata(entry.join(".collide-verified"))
            .unwrap()
            .modified()
            .unwrap(),
        marker
    );
}

#[test]
fn packs_load_in_the_order_the_board_lists_them() {
    let line = Line::basic();
    line.write(
        ".collide/board.yaml",
        "packs:\n  - source: repo:packs/zeta\n  - source: repo:.collide/packs/ferry\n",
    );
    line.write("packs/zeta/pack.yaml", &manifest("zeta", ""));
    line.commit();
    assert_eq!(
        names(&line.ok()),
        ["collide", "collide/plan", "zeta", "ferry"]
    );
}

#[test]
fn a_repo_pack_path_that_is_missing_a_file_or_a_symlink_is_a_problem_naming_the_source() {
    let cases = [
        ("repo:packs/not-there", "not in the repo"),
        ("repo:packs/a-file", "file"),
        ("repo:packs/a-link", "symlink"),
        ("repo:packs/../outside", "`..`"),
        ("repo:/abs/packs", "`/`"),
    ];
    for (source, says) in cases {
        let line = Line::new();
        line.write(
            ".collide/board.yaml",
            &format!("packs:\n  - source: \"{source}\"\n"),
        );
        line.write("packs/a-file", "x\n");
        line.write("packs/real/pack.yaml", &manifest("real", ""));
        line.repo.symlink("packs/a-link", "real");
        line.commit();
        let problems = line.problems();
        assert_problem(&problems, &[source, says]);
        assert_eq!(problems.len(), 1, "{source}: {problems:?}");
    }
}

#[test]
fn a_materializer_refusal_is_a_problem_naming_the_pack_and_the_path() {
    // (a file the pack commits, the word the problem has for it)
    let cases = [
        ("node_modules/@collide/pack-sdk/index.js", "node_modules"),
        ("package.json", "package.json"),
    ];
    for (path, says) in cases {
        let line = Line::basic();
        line.write(&format!(".collide/packs/ferry/{path}"), "x\n");
        line.commit();
        assert_problem(&line.problems(), &["repo:.collide/packs/ferry", path, says]);
    }
    let line = Line::basic();
    line.repo
        .symlink(".collide/packs/ferry/hooks/link", "run.sh");
    line.commit();
    assert_problem(
        &line.problems(),
        &["repo:.collide/packs/ferry", "hooks/link", "symlink"],
    );
}

#[test]
fn a_pack_with_no_pack_yaml_is_a_problem_naming_the_pack() {
    let line = Line::basic();
    line.repo.remove(".collide/packs/ferry/pack.yaml");
    line.commit();
    assert_problem(
        &line.problems(),
        &["repo:.collide/packs/ferry", "pack.yaml", "no"],
    );
}

#[test]
fn a_manifest_that_does_not_parse_or_fails_its_check_is_refused_naming_the_pack_file_and_key() {
    // (the manifest, what the problem names besides the pack and `pack.yaml`)
    let cases = [
        (
            "apiVersion: collide/v0\nname: ferry\nflavor: mint\n",
            "flavor",
        ),
        (
            "apiVersion: collide/v0\nname: ferry\nintegrations:\n  m: { script: ../outside/m.ts }\n",
            "integrations.m.script",
        ),
        (
            "apiVersion: collide/v0\nname: ferry\nroles:\n  builder: { agent: \"\" }\n",
            "agent",
        ),
        (
            "apiVersion: collide/v0\nname: ferry\nconfig:\n  a: &a { type: string }\n  b: *a\n",
            "anchors",
        ),
        ("apiVersion: collide/v0\n", "name"),
    ];
    for (text, names) in cases {
        let line = Line::basic();
        line.write(".collide/packs/ferry/pack.yaml", text);
        line.commit();
        assert_problem(
            &line.problems(),
            &["repo:.collide/packs/ferry", "pack.yaml", names],
        );
    }
}

#[test]
fn a_role_workflow_or_prompt_name_that_is_not_a_safe_token_is_refused_naming_the_pack_file_and_key()
{
    let human = "human: Read it.\n        on: { approve: { end: approved } }\n";
    let cases = [
        (
            "roles:\n  \"bad;id\": { agent: claude }\n".to_string(),
            "roles.bad;id",
        ),
        ("roles:\n  \"..\": { agent: claude }\n".to_string(), "roles"),
        (
            format!("workflows:\n  \"-flag\":\n    phases:\n      review:\n        {human}"),
            "workflows.-flag",
        ),
        (
            "prompts:\n  \"a;b\": prompts/a.md\n".to_string(),
            "prompts.a;b",
        ),
    ];
    for (extra, key) in cases {
        let line = Line::basic();
        line.write(".collide/packs/ferry/pack.yaml", &manifest("ferry", &extra));
        line.commit();
        let problems = line.problems();
        assert_problem(
            &problems,
            &["repo:.collide/packs/ferry", "pack.yaml", key, "safe token"],
        );
    }
}

#[test]
fn a_template_field_that_is_not_a_safe_token_is_refused_naming_the_pack_file_and_key() {
    let extra = "workflows:\n  loop:\n    accepts: { fields: [labelId, \"bad;field\"] }\n    phases:\n      review:\n\
                 \x20       human: Read it.\n        on: { approve: { end: approved } }\n";
    let line = Line::basic();
    line.write(".collide/packs/ferry/pack.yaml", &manifest("ferry", extra));
    line.commit();
    let problems = line.problems();
    assert_problem(
        &problems,
        &[
            "repo:.collide/packs/ferry",
            "pack.yaml",
            "workflows.loop.accepts.fields[1]",
            "bad;field",
        ],
    );
    assert_eq!(problems.len(), 1, "{problems:?}");
}

#[test]
fn safe_names_and_a_name_with_a_dot_or_dash_inside_load() {
    let extra = "roles:\n  builder-2.x: { agent: claude }\nprompts:\n  build_1: prompts/build.md\n";
    let line = Line::basic();
    line.write(".collide/packs/ferry/pack.yaml", &manifest("ferry", extra));
    line.write(".collide/packs/ferry/prompts/build.md", "build\n");
    line.commit();
    line.ok();
}

#[test]
fn a_board_source_with_a_scheme_that_is_not_supported_is_a_problem_naming_it() {
    let line = Line::basic();
    line.write(
        ".collide/board.yaml",
        "packs:\n  - source: npm:ferry-pack\n",
    );
    line.commit();
    assert_problem(&line.problems(), &["npm:ferry-pack", "not supported"]);
}

#[test]
fn a_dev_source_in_board_yaml_is_refused_with_the_boards_message_and_no_pack_is_loaded_for_it() {
    let line = Line::basic();
    line.write(
        ".collide/board.yaml",
        &format!("{FERRY_BOARD}  - source: dev:/somewhere/ferry\n"),
    );
    line.commit();
    let problems = line.problems();
    assert_problem(&problems, &[".collide/board.yaml", "refuses `dev:`"]);
    assert_eq!(problems.len(), 1, "{problems:?}");
}

#[test]
fn the_same_source_listed_twice_is_one_board_problem_and_is_loaded_once() {
    let line = Line::basic();
    line.write(
        ".collide/board.yaml",
        &format!("{FERRY_BOARD}{}", &FERRY_BOARD["packs:\n".len()..]),
    );
    line.commit();
    let problems = line.problems();
    assert_problem(&problems, &[".collide/board.yaml", "both list"]);
    assert_eq!(problems.len(), 1, "{problems:?}");
}

// ---- git packs ----

const GIT_SOURCE: &str = "git:example.invalid/remote//pack@main";

/// A pack in a repository of its own, materialized into the line's cache, as the mirror will leave it (ts-0100-pu).
/// Returns its commit and tree.
fn prime_cache(line: &Line, name: &str) -> (String, String) {
    let source = GitRepo::new();
    source.write("pack.yaml", manifest(name, "version: 3.0.0\n").as_bytes());
    source.write("hooks/run.sh", b"#!/bin/sh\necho remote\n");
    let commit = source.commit_all("remote");
    let tree = source.git(&["rev-parse", "HEAD^{tree}"]);
    materialize(&line.cache, &line.sdk, &source.git_dir(), &tree).unwrap();
    (commit, tree)
}

fn git_line(source_in_lock: &str, commit: &str, tree: &str) -> Line {
    let line = Line::new();
    line.write(
        ".collide/board.yaml",
        &format!("packs:\n  - source: {GIT_SOURCE}\n"),
    );
    line.write(
        ".collide/packs.lock",
        &format!("packs:\n  - source: {source_in_lock}\n    commit: {commit}\n    tree: {tree}\n"),
    );
    line.commit();
    line
}

/// A line whose lock pins a `git:` pack, with the pack primed in that line's own cache.
fn primed_git_line() -> (Line, String, String) {
    let line = git_line(GIT_SOURCE, &"a".repeat(40), &"b".repeat(40));
    let (commit, tree) = prime_cache(&line, "remote");
    line.write(
        ".collide/packs.lock",
        &format!("packs:\n  - source: {GIT_SOURCE}\n    commit: {commit}\n    tree: {tree}\n"),
    );
    line.commit();
    (line, commit, tree)
}

#[test]
fn a_git_pack_with_a_lock_entry_loads_from_the_cache_by_its_pinned_tree() {
    let (line, commit, tree) = primed_git_line();
    let loaded = line.ok();
    let remote = pack(&loaded, "remote");
    assert_eq!(remote.origin, PackOrigin::Git);
    assert_eq!(remote.source.as_deref(), Some(GIT_SOURCE));
    assert_eq!(remote.pin.as_deref(), Some(tree.as_str()));
    let entry = line.cache.join("packs").join(entry_name(&tree, "0.1.0"));
    assert_eq!(remote.entry.as_deref(), Some(entry.as_path()));
    assert_eq!(remote.manifest.version.as_deref(), Some("3.0.0"));
    assert!(remote.dir.join("hooks/run.sh").is_file());
    assert_eq!(loaded.lock.packs[0].commit, commit);
    assert!(
        loaded
            .trust_inputs()
            .contains(&(format!("pack {GIT_SOURCE}"), tree))
    );
    assert!(
        loaded
            .trust_inputs()
            .iter()
            .any(|(name, _)| name == "packs.lock")
    );
}

#[test]
fn a_git_pack_is_loaded_by_the_lock_and_never_by_the_remotes_ref() {
    let (line, _, tree) = primed_git_line();
    // The source names `@main` at a host that does not exist: nothing is fetched, the lock's tree is the pin.
    let loaded = line.ok();
    assert_eq!(pack(&loaded, "remote").pin.as_deref(), Some(tree.as_str()));
}

#[test]
fn a_git_pack_with_no_lock_entry_fails_naming_it() {
    let (line, _, _) = primed_git_line();
    line.write(".collide/packs.lock", "packs: []\n");
    line.commit();
    let problems = line.problems();
    assert_problem(&problems, &[GIT_SOURCE, "no entry", "packs.lock"]);
    assert_eq!(problems.len(), 1, "{problems:?}");
}

#[test]
fn a_lock_entry_whose_source_text_differs_fails_and_lists_what_the_lock_has() {
    let (line, commit, tree) = primed_git_line();
    let other = "git:example.invalid/remote//pack@v2";
    line.write(
        ".collide/packs.lock",
        &format!("packs:\n  - source: {other}\n    commit: {commit}\n    tree: {tree}\n"),
    );
    line.commit();
    let problems = line.problems();
    assert_problem(&problems, &[GIT_SOURCE, "no entry", other]);
}

#[test]
fn a_git_pack_whose_entry_is_not_in_the_cache_is_a_problem_naming_the_pack_and_the_commit() {
    let line = git_line(GIT_SOURCE, &"c".repeat(40), &"d".repeat(40));
    let problems = line.problems();
    assert_problem(
        &problems,
        &[GIT_SOURCE, &"c".repeat(40), "not in the cache"],
    );
    assert_eq!(problems.len(), 1, "{problems:?}");
}

#[test]
fn a_git_pack_whose_cache_entry_is_for_another_sdk_is_not_in_the_cache() {
    let (line, _, _) = primed_git_line();
    make_sdk(&line.sdk, "0.2.0");
    assert_problem(&line.problems(), &[GIT_SOURCE, "not in the cache"]);
}

#[test]
fn a_git_pack_with_a_bad_lock_tree_id_is_a_problem_and_nothing_is_read_from_the_cache() {
    let line = git_line(GIT_SOURCE, &"c".repeat(40), "../../etc");
    let problems = line.problems();
    assert_problem(&problems, &[".collide/packs.lock", "tree"]);
}

// ---- git packs: the mirror and the fetch (ts-0100-pu) ----

/// A remote with the pack `name` at `pack/` on `main`: the remote, its commit and the pack's tree.
fn remote_pack(name: &str) -> (Remote, String, String) {
    let remote = Remote::new();
    remote.work.write(
        "pack/pack.yaml",
        manifest(name, "version: 3.0.0\n").as_bytes(),
    );
    remote
        .work
        .write("pack/hooks/run.sh", b"#!/bin/sh\necho remote\n");
    let commit = remote.work.commit_all("remote");
    let tree = remote.work.git(&["rev-parse", "HEAD:pack"]);
    remote.publish();
    (remote, commit, tree)
}

/// A fetch with real git from the test's own home (`root`), reading no system config.
fn real_fetch(root: &TempRoot) -> GitFetch {
    GitFetch::new(FetchEnv {
        path: std::env::var_os("PATH").unwrap_or_default(),
        home: root.path().to_path_buf(),
        ssh_auth_sock: None,
        xdg_config_home: None,
    })
    .without_system_config()
}

/// A `git` on its own `PATH` that records each run in a file and fails. Returns the `PATH` and the file.
fn recording_git(root: &TempRoot) -> (std::ffi::OsString, PathBuf) {
    let dir = root.join("fake-git");
    fs::create_dir(&dir).unwrap();
    let ran = root.join("fake-git-ran");
    root.script(
        "fake-git/git",
        &format!("#!/bin/sh\necho ran >> '{}'\nexit 1\n", ran.display()),
    );
    (dir.into_os_string(), ran)
}

/// A line that lists and pins a pack at `source`, in a cache of its own. The context fetches with `fetch`.
fn pinned_line(source: &str, commit: &str, tree: &str) -> Line {
    let line = Line::new();
    line.write(
        ".collide/board.yaml",
        &format!("packs:\n  - source: {source}\n"),
    );
    line.write(
        ".collide/packs.lock",
        &format!("packs:\n  - source: {source}\n    commit: {commit}\n    tree: {tree}\n"),
    );
    line.commit();
    line
}

fn load_with(
    line: &Line,
    fetch: GitFetch,
    home: &TempRoot,
) -> Result<LoadedLine, Vec<LoadProblem>> {
    let context = line
        .context()
        .with_fetch(fetch, KnownHosts::for_home(home.path()));
    load_line(&line.repo.git_dir(), "main", &context)
}

fn mirrors(line: &Line) -> Vec<String> {
    let Ok(entries) = fs::read_dir(line.cache.join("git")) else {
        return Vec::new();
    };
    entries
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect()
}

#[test]
fn a_cold_cache_fetches_into_the_mirror_and_loads_the_pack_at_its_locked_tree() {
    let (remote, commit, tree) = remote_pack("remote");
    let source = format!("git:{}//pack@main", remote.file_url());
    let line = pinned_line(&source, &commit, &tree);
    let home = TempRoot::new();
    let loaded = load_with(&line, real_fetch(&home), &home).unwrap_or_else(|p| panic!("{p:?}"));
    let found = pack(&loaded, "remote");
    assert_eq!(found.origin, PackOrigin::Git);
    assert_eq!(found.source.as_deref(), Some(source.as_str()));
    assert_eq!(found.pin.as_deref(), Some(tree.as_str()));
    assert_eq!(found.manifest.version.as_deref(), Some("3.0.0"));
    assert!(found.dir.join("hooks/run.sh").is_file());
    assert_eq!(
        mirrors(&line).len(),
        1,
        "one mirror, in the cache's git folder"
    );
}

#[test]
fn the_loaded_line_shows_the_packs_name_source_commit_and_tree() {
    let (remote, commit, tree) = remote_pack("remote");
    let source = format!("git:{}//pack@main", remote.file_url());
    let line = pinned_line(&source, &commit, &tree);
    let home = TempRoot::new();
    let loaded = load_with(&line, real_fetch(&home), &home).unwrap();
    let locked = &loaded.lock.packs[0];
    assert_eq!(
        (
            locked.source.as_str(),
            locked.commit.as_str(),
            locked.tree.as_str()
        ),
        (source.as_str(), commit.as_str(), tree.as_str())
    );
    let found = pack(&loaded, "remote");
    assert_eq!(
        (found.name.as_str(), found.pin.as_deref()),
        ("remote", Some(tree.as_str()))
    );
}

#[test]
fn a_warm_cache_runs_no_git_and_makes_no_mirror() {
    let (line, _, tree) = primed_git_line();
    let home = TempRoot::new();
    let (path, ran) = recording_git(&home);
    let fetch = GitFetch::new(FetchEnv {
        path,
        home: home.path().to_path_buf(),
        ssh_auth_sock: None,
        xdg_config_home: None,
    });
    let loaded = load_with(&line, fetch, &home).unwrap();
    assert_eq!(pack(&loaded, "remote").pin.as_deref(), Some(tree.as_str()));
    assert!(!ran.exists(), "git ran");
    assert!(mirrors(&line).is_empty());
}

#[test]
fn a_cold_cache_and_a_host_never_fetched_from_is_a_problem_that_names_the_host_and_runs_nothing() {
    let source = "git:https://example.invalid/o/r//pack@v1";
    let line = pinned_line(source, &"c".repeat(40), &"d".repeat(40));
    let home = TempRoot::new();
    let (path, ran) = recording_git(&home);
    let fetch = GitFetch::new(FetchEnv {
        path,
        home: home.path().to_path_buf(),
        ssh_auth_sock: None,
        xdg_config_home: None,
    });
    let problems = load_with(&line, fetch, &home).unwrap_err();
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert_eq!(problems[0].needs_host.as_deref(), Some("example.invalid"));
    assert_eq!(problems[0].pack.as_deref(), Some(source));
    assert!(
        problems[0].message.contains("example.invalid"),
        "{}",
        problems[0]
    );
    assert!(!ran.exists(), "git ran");
    assert!(mirrors(&line).is_empty());
}

#[test]
fn a_host_that_has_been_approved_is_fetched_from_on_a_cold_cache() {
    let source = "git:https://example.invalid/o/r//pack@v1";
    let line = pinned_line(source, &"c".repeat(40), &"d".repeat(40));
    let home = TempRoot::new();
    KnownHosts::for_home(home.path())
        .record("example.invalid")
        .unwrap();
    let (path, ran) = recording_git(&home);
    let fetch = GitFetch::new(FetchEnv {
        path,
        home: home.path().to_path_buf(),
        ssh_auth_sock: None,
        xdg_config_home: None,
    });
    let problems = load_with(&line, fetch, &home).unwrap_err();
    assert!(ran.exists(), "git ran");
    assert_eq!(problems[0].needs_host, None);
    assert_problem(&texts(&problems), &[source, "getting it failed"]);
}

#[test]
fn a_fetch_that_hangs_fails_the_load_naming_the_pack_and_the_time() {
    let source = "git:https://example.invalid/o/r//pack@v1";
    let line = pinned_line(source, &"c".repeat(40), &"d".repeat(40));
    let home = TempRoot::new();
    KnownHosts::for_home(home.path())
        .record("example.invalid")
        .unwrap();
    fs::create_dir(home.join("hang")).unwrap();
    home.script("hang/git", "#!/bin/sh\nexec /bin/sleep 30\n");
    let fetch = GitFetch::new(FetchEnv {
        path: home.join("hang").into_os_string(),
        home: home.path().to_path_buf(),
        ssh_auth_sock: None,
        xdg_config_home: None,
    })
    .with_timeout(Duration::from_millis(300));
    let started = Instant::now();
    let problems = texts(&load_with(&line, fetch, &home).unwrap_err());
    assert!(started.elapsed() < Duration::from_secs(20));
    assert_problem(&problems, &[source, "did not end within 0.3"]);
}

#[test]
fn a_locked_commit_the_remote_no_longer_has_fails_naming_the_pack_and_the_commit() {
    let (remote, _, tree) = remote_pack("remote");
    let source = format!("git:{}//pack@main", remote.file_url());
    let gone = "e".repeat(40);
    let line = pinned_line(&source, &gone, &tree);
    let home = TempRoot::new();
    let problems = texts(&load_with(&line, real_fetch(&home), &home).unwrap_err());
    assert_problem(&problems, &[&source, &gone, "no longer"]);
    assert_eq!(problems.len(), 1, "{problems:?}");
}

#[test]
fn a_lock_whose_tree_is_not_the_one_at_its_commit_fails_naming_both_ids() {
    let (remote, commit, tree) = remote_pack("remote");
    let source = format!("git:{}//pack@main", remote.file_url());
    let wrong = "f".repeat(40);
    let line = pinned_line(&source, &commit, &wrong);
    let home = TempRoot::new();
    let problems = texts(&load_with(&line, real_fetch(&home), &home).unwrap_err());
    assert_problem(&problems, &[&source, &wrong, &tree]);
}

#[test]
fn a_locked_path_that_is_not_in_the_commit_fails_naming_the_pack() {
    let (remote, commit, tree) = remote_pack("remote");
    let source = format!("git:{}//nowhere@main", remote.file_url());
    let line = pinned_line(&source, &commit, &tree);
    let home = TempRoot::new();
    let problems = texts(&load_with(&line, real_fetch(&home), &home).unwrap_err());
    assert_problem(&problems, &[&source, "nowhere"]);
}

#[test]
fn two_lines_that_pin_one_tree_share_one_mirror_and_one_cache_entry() {
    let (remote, commit, tree) = remote_pack("remote");
    let source = format!("git:{}//pack@main", remote.file_url());
    let first = pinned_line(&source, &commit, &tree);
    let second = pinned_line(&source, &commit, &tree);
    let home = TempRoot::new();
    let fetch = real_fetch(&home);
    let hosts = KnownHosts::for_home(home.path());
    // Both lines load through the first one's cache.
    let context = first.context().with_fetch(fetch, hosts);
    let one = load_line(&first.repo.git_dir(), "main", &context).unwrap();
    let two = load_line(&second.repo.git_dir(), "main", &context).unwrap();
    assert_eq!(pack(&one, "remote").entry, pack(&two, "remote").entry);
    assert_eq!(mirrors(&first).len(), 1);
    let entries = fs::read_dir(first.cache.join("packs")).unwrap().count();
    assert_eq!(entries, 1, "one cache entry");
}

#[test]
fn a_remote_branch_that_moves_changes_nothing_until_the_pin_is_moved() {
    let (remote, commit, tree) = remote_pack("remote");
    let source = format!("git:{}//pack@main", remote.file_url());
    let line = pinned_line(&source, &commit, &tree);
    let home = TempRoot::new();
    let context = line
        .context()
        .with_fetch(real_fetch(&home), KnownHosts::for_home(home.path()));
    let before = load_line(&line.repo.git_dir(), "main", &context).unwrap();

    remote.work.write(
        "pack/pack.yaml",
        manifest("remote", "version: 4.0.0\n").as_bytes(),
    );
    remote.work.commit_all("moved");
    remote.publish();
    let after = load_line(&line.repo.git_dir(), "main", &context).unwrap();

    assert_eq!(pack(&after, "remote").pin, pack(&before, "remote").pin);
    assert_eq!(
        pack(&after, "remote").manifest.version.as_deref(),
        Some("3.0.0")
    );
}

// ---- names ----

#[test]
fn two_packs_with_one_name_fail_naming_both_sources() {
    let line = Line::basic();
    line.write(
        ".collide/board.yaml",
        &format!("{FERRY_BOARD}  - source: repo:packs/twin\n"),
    );
    line.write("packs/twin/pack.yaml", &manifest("ferry", ""));
    line.commit();
    let problems = line.problems();
    assert_problem(
        &problems,
        &["ferry", "repo:.collide/packs/ferry", "repo:packs/twin"],
    );
    assert_eq!(problems.len(), 1, "{problems:?}");
}

#[test]
fn a_pack_named_like_a_built_in_fails_naming_the_built_in() {
    let line = Line::basic();
    line.write(".collide/packs/ferry/pack.yaml", &manifest("collide", ""));
    line.commit();
    let problems = line.problems();
    assert_problem(
        &problems,
        &["collide", "built-in", "repo:.collide/packs/ferry"],
    );
    assert_eq!(problems.len(), 1, "{problems:?}");
}

#[test]
fn a_pack_named_like_a_root_of_collides_own_settings_fails_naming_both() {
    for root in ["agents", "ask", "disk", "claude"] {
        let line = Line::basic();
        line.write(".collide/packs/ferry/pack.yaml", &manifest(root, ""));
        line.commit();
        let problems = line.problems();
        assert_problem(
            &problems,
            &[
                &format!("`{root}`"),
                "Collide's own",
                "repo:.collide/packs/ferry",
            ],
        );
        assert_eq!(problems.len(), 1, "{root}: {problems:?}");
    }
}

#[test]
fn a_pack_whose_name_only_starts_like_a_root_loads() {
    for name in ["asking", "disks", "agent", "claude-extras"] {
        let line = Line::basic();
        line.write(".collide/packs/ferry/pack.yaml", &manifest(name, ""));
        line.commit();
        assert!(names(&line.ok()).contains(&name), "{name}");
    }
}

#[test]
fn a_pack_name_that_is_not_a_safe_token_is_refused() {
    for name in ["bad;name", "x..y", "-flag", "collide/plan", "a/b"] {
        let line = Line::basic();
        line.write(
            ".collide/packs/ferry/pack.yaml",
            &manifest(&format!("\"{name}\""), ""),
        );
        line.commit();
        let problems = line.problems();
        assert!(
            problems
                .iter()
                .any(|text| text.contains("pack.yaml") && text.contains(name)),
            "{name}: {problems:?}"
        );
    }
}

// ---- requires ----

fn requires_line(requires: &str) -> Line {
    let line = Line::basic();
    line.write(
        ".collide/packs/ferry/pack.yaml",
        &manifest("ferry", &format!("requires:\n{requires}")),
    );
    line.commit();
    line
}

#[test]
fn a_missing_tool_fails_naming_the_pack_the_tool_and_the_hint() {
    let line =
        requires_line("  tools:\n    - { name: tool-that-is-missing, hint: \"install it\" }\n");
    let problems = line.problems();
    assert_problem(
        &problems,
        &["ferry", "tool-that-is-missing", "install it", "pack.yaml"],
    );
    assert_eq!(problems.len(), 1, "{problems:?}");
}

#[test]
fn a_missing_tool_with_no_hint_fails_naming_the_pack_and_the_tool() {
    let line = requires_line("  tools:\n    - { name: tool-that-is-missing }\n");
    assert_problem(&line.problems(), &["ferry", "tool-that-is-missing"]);
}

#[test]
fn a_tool_in_range_meets_the_requirement() {
    let line = requires_line("  tools:\n    - { name: node, version: \">=22.18\" }\n");
    line.tool("node", "echo v22.18.0");
    line.ok();
}

#[test]
fn a_tool_out_of_range_fails_naming_the_version_found_and_the_range() {
    let line = requires_line("  tools:\n    - { name: node, version: \">=999.0\" }\n");
    line.tool("node", "echo v22.18.0");
    let problems = line.problems();
    assert_problem(&problems, &["ferry", "node", "22.18.0", ">=999.0"]);
}

#[test]
fn a_tool_out_of_range_carries_the_hint() {
    let line = requires_line(
        "  tools:\n    - { name: node, version: \">=999.0\", hint: \"upgrade node\" }\n",
    );
    line.tool("node", "echo v22.18.0");
    assert_problem(&line.problems(), &["node", "upgrade node"]);
}

#[test]
fn a_tool_that_prints_its_version_on_stderr_is_read_too() {
    let line = requires_line("  tools:\n    - { name: jvm, version: \">=17.0\" }\n");
    line.tool("jvm", "echo 'jvm version 17.0.2' >&2");
    line.ok();
}

#[test]
fn a_tool_with_no_version_range_is_found_and_not_run() {
    let line = requires_line("  tools:\n    - { name: marker-tool }\n");
    let ran = line.repo.root().join("ran.txt");
    line.tool("marker-tool", &format!("echo ran >> {}", ran.display()));
    line.ok();
    assert!(!ran.exists(), "a tool with no version range was run");
}

#[test]
fn a_tool_that_two_packs_need_is_run_once() {
    let line = Line::basic();
    line.write(
        ".collide/board.yaml",
        &format!("{FERRY_BOARD}  - source: repo:packs/second\n"),
    );
    let needs = "requires:\n  tools:\n    - { name: node, version: \">=22.18\" }\n";
    line.write(".collide/packs/ferry/pack.yaml", &manifest("ferry", needs));
    line.write("packs/second/pack.yaml", &manifest("second", needs));
    line.commit();
    let ran = line.repo.root().join("ran.txt");
    line.tool(
        "node",
        &format!("echo ran >> {}\necho v22.18.0", ran.display()),
    );
    line.ok();
    assert_eq!(fs::read_to_string(&ran).unwrap().lines().count(), 1);
}

#[test]
fn a_tool_is_run_with_a_cleared_environment() {
    let line = requires_line("  tools:\n    - { name: node, version: \">=22.18\" }\n");
    let seen = line.repo.root().join("env.txt");
    line.tool(
        "node",
        &format!("/usr/bin/env > {}\necho v22.18.0", seen.display()),
    );
    line.ok();
    let env = fs::read_to_string(&seen).unwrap();
    assert!(env.lines().any(|entry| entry.starts_with("PATH=")), "{env}");
    assert!(
        !env.lines().any(|entry| entry.starts_with("HOME=")),
        "{env}"
    );
}

#[test]
fn a_tool_that_hangs_fails_within_the_timeout_naming_the_tool() {
    let line = requires_line("  tools:\n    - { name: slow, version: \">=1.0\" }\n");
    line.tool("slow", "exec /bin/sleep 30");
    let context = line.context().with_tool_timeout(Duration::from_millis(300));
    let started = Instant::now();
    let problems = texts(&load_line(&line.repo.git_dir(), "main", &context).unwrap_err());
    assert!(started.elapsed() < Duration::from_secs(10));
    assert_problem(&problems, &["ferry", "slow", "did not end"]);
}

#[test]
fn a_tool_that_prints_no_version_or_fails_is_a_problem_naming_the_tool() {
    let cases = [
        ("echo hello", "no version"),
        ("echo 'oops' >&2\nexit 3", "exit"),
    ];
    for (body, says) in cases {
        let line = requires_line("  tools:\n    - { name: odd, version: \">=1.0\" }\n");
        line.tool("odd", body);
        assert_problem(&line.problems(), &["ferry", "odd", says]);
    }
}

#[test]
fn a_range_form_that_is_not_known_fails_naming_it_and_the_tool_is_not_run() {
    for form in ["^1", "~1", "1.x", "*"] {
        let line = requires_line(&format!(
            "  tools:\n    - {{ name: marker-tool, version: \"{form}\" }}\n"
        ));
        let ran = line.repo.root().join("ran.txt");
        line.tool(
            "marker-tool",
            &format!("echo ran >> {}\necho 1.0.0", ran.display()),
        );
        assert_problem(
            &line.problems(),
            &["ferry", "marker-tool", &format!("`{form}`")],
        );
        assert!(!ran.exists());
    }
}

#[test]
fn a_tool_name_that_is_a_path_is_refused_and_nothing_is_run() {
    let line = requires_line("  tools:\n    - { name: \"../bin/node\", version: \">=1.0\" }\n");
    line.tool("node", "echo 1.0.0");
    assert_problem(
        &line.problems(),
        &["ferry", "../bin/node", "requires.tools[0].name"],
    );
}

#[test]
fn an_sdk_range_the_sdk_does_not_meet_fails_naming_both() {
    let line = requires_line("  sdk: \">=9.0\"\n");
    let problems = line.problems();
    assert_problem(&problems, &["ferry", "requires.sdk", ">=9.0", "0.1.0"]);
}

#[test]
fn an_sdk_range_the_sdk_meets_loads() {
    let line = requires_line("  sdk: \">=0.1 <0.2\"\n");
    line.ok();
}

#[test]
fn an_sdk_range_with_a_form_that_is_not_known_fails_naming_it() {
    let line = requires_line("  sdk: \"^0.1\"\n");
    assert_problem(&line.problems(), &["ferry", "requires.sdk", "`^0.1`"]);
}

#[test]
fn an_sdk_that_cannot_be_read_is_one_problem_for_the_line_and_loads_no_pack_from_the_cache() {
    let line = Line::basic();
    fs::remove_file(line.sdk.join("package.json")).unwrap();
    let problems = line.problems();
    assert_problem(&problems, &["SDK", "package.json"]);
    assert_eq!(problems.len(), 1, "{problems:?}");
    assert!(!line.cache.exists());
}

// ---- several problems at once ----

#[test]
fn a_bad_board_and_a_bad_pack_in_one_load_give_both() {
    let line = Line::basic();
    line.write(
        ".collide/board.yaml",
        "packs:\n  - source: dev:/x\n  - source: repo:.collide/packs/ferry\n",
    );
    line.write(
        ".collide/packs/ferry/pack.yaml",
        "apiVersion: collide/v0\nname: ferry\nflavor: mint\n",
    );
    line.commit();
    let problems = line.problems();
    assert_problem(&problems, &[".collide/board.yaml", "dev:"]);
    assert_problem(&problems, &["repo:.collide/packs/ferry", "flavor"]);
}

#[test]
fn problems_come_in_file_order_pointer_board_lock_config_profiles_then_packs_in_the_boards_order() {
    let line = Line::new();
    line.write(
        ".collide/board.yaml",
        "packs:\n  - source: dev:/x\n  - source: repo:packs/one\n  - source: repo:packs/two\n",
    );
    line.write(
        ".collide/packs.lock",
        &fixture("locks/invalid/repo-source.lock"),
    );
    line.write(
        ".collide/config.yaml",
        &fixture("config/invalid/disk-floor-zero.yaml"),
    );
    line.write(
        ".collide/profiles/bad.yaml",
        &fixture("profiles/invalid/compact-at-below-floor.yaml"),
    );
    line.write(
        "packs/one/pack.yaml",
        "apiVersion: collide/v0\nname: one\nflavor: mint\n",
    );
    line.write(
        "packs/two/pack.yaml",
        "apiVersion: collide/v0\nname: two\nflavor: mint\n",
    );
    line.commit();
    let problems = line.problems().join("\n");
    let at = |part: &str| {
        problems
            .find(part)
            .unwrap_or_else(|| panic!("no {part} in:\n{problems}"))
    };
    let order = [
        at(".collide/board.yaml"),
        at(".collide/packs.lock"),
        at(".collide/config.yaml"),
        at(".collide/profiles/bad.yaml"),
        at("repo:packs/one"),
        at("repo:packs/two"),
    ];
    let mut sorted = order;
    sorted.sort();
    assert_eq!(order, sorted, "{problems}");
}

#[test]
fn a_pointer_problem_and_a_built_in_pack_problem_are_both_returned() {
    let line = Line::new();
    line.write(".collide", "dir: ../up\n");
    line.commit();
    let context = LoadContext::new(
        line.cache.clone(),
        line.sdk.clone(),
        Environment {
            path: line.bin.clone().into_os_string(),
        },
        BuiltinPacks::new(line.repo.root().join("no-bundle")),
    );
    let problems = texts(&load_line(&line.repo.git_dir(), "main", &context).unwrap_err());
    assert_problem(&problems, &[".collide", "../up"]);
    assert_problem(&problems, &["built-in"]);
}

// ---- pack data ----

fn mode_of(path: &Path) -> u32 {
    fs::metadata(path).unwrap().permissions().mode() & 0o777
}

#[test]
fn pack_data_is_a_distinct_folder_per_pack_and_per_board_made_with_mode_0700() {
    let root = TempRoot::new();
    let data = root.join("data");
    let a = pack_data_dir(&data, "board-1", "ferry").unwrap();
    let b = pack_data_dir(&data, "board-1", "other").unwrap();
    let c = pack_data_dir(&data, "board-2", "ferry").unwrap();
    assert!(a != b && a != c && b != c);
    for dir in [&a, &b, &c] {
        assert!(dir.is_dir());
        assert!(dir.starts_with(&data));
        assert_eq!(mode_of(dir), 0o700);
        assert_eq!(mode_of(dir.parent().unwrap()), 0o700);
    }
    assert_eq!(mode_of(&data), 0o700);
}

#[test]
fn pack_data_is_made_once_and_a_second_call_keeps_what_is_in_it() {
    let root = TempRoot::new();
    let data = root.join("data");
    let first = pack_data_dir(&data, "board-1", "ferry").unwrap();
    fs::write(first.join("state.json"), "{}").unwrap();
    let second = pack_data_dir(&data, "board-1", "ferry").unwrap();
    assert_eq!(first, second);
    assert_eq!(fs::read_to_string(second.join("state.json")).unwrap(), "{}");
}

#[test]
fn the_built_in_plan_pack_gets_a_folder_that_no_user_pack_can_share() {
    let root = TempRoot::new();
    let data = root.join("data");
    let plan = pack_data_dir(&data, "board-1", "collide/plan").unwrap();
    let name = plan.file_name().unwrap().to_string_lossy().to_string();
    assert_eq!(name, "collide~plan");
    assert_eq!(
        plan.parent().unwrap(),
        pack_data_dir(&data, "board-1", "collide")
            .unwrap()
            .parent()
            .unwrap()
    );
    assert!(pack_data_dir(&data, "board-1", "collide~plan").is_err());
}

#[test]
fn a_board_or_pack_name_that_is_not_a_safe_token_is_refused_and_nothing_is_made() {
    let root = TempRoot::new();
    let data = root.join("data");
    let bad_packs = [
        "",
        "..",
        "a/../b",
        ".hidden",
        "-x",
        "a b",
        "collide//plan",
        "/collide",
        "a/",
        "x;y",
        "a~b",
    ];
    for pack in bad_packs {
        assert!(
            pack_data_dir(&data, "board-1", pack).is_err(),
            "pack {pack:?}"
        );
    }
    for board in ["", "..", "../x", "a/b", ".x", "-x", "a b"] {
        assert!(
            pack_data_dir(&data, board, "ferry").is_err(),
            "board {board:?}"
        );
    }
    assert!(!data.exists(), "a refused name made a folder");
}

#[test]
fn pack_data_where_a_symlink_or_a_file_stands_is_refused() {
    let root = TempRoot::new();
    let data = root.join("data");
    let elsewhere = root.join("elsewhere");
    fs::create_dir(&elsewhere).unwrap();
    let board = data.join("board-1");
    fs::create_dir_all(&board).unwrap();
    std::os::unix::fs::symlink(&elsewhere, board.join("linked")).unwrap();
    fs::write(board.join("a-file"), "x").unwrap();
    assert!(pack_data_dir(&data, "board-1", "linked").is_err());
    assert!(pack_data_dir(&data, "board-1", "a-file").is_err());
    assert_eq!(fs::read_dir(&elsewhere).unwrap().count(), 0);
}
