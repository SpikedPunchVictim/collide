//! Which launch line loads the worktree's own instructions, with fake Claude (ts-0103-sa; 0.3, The tool's config paths
//! in a repo; revision 21). This is Part 1 of that item: the candidate lines as data, the fixture repository they run
//! in, the rule a line must keep, and what fake Claude saw of each. Nothing here runs `claude` or spends anything.
//! `repo_instructions_live.rs` is Part 2: it reuses [`Fixture`], [`Candidate`] and [`Line`] against the real binary.
//!
//! **The rule.** Settings, hooks, MCP config and agent definitions come from the line's snapshot. So a candidate keeps
//! the four snapshot flags (`--setting-sources user`, `--settings`, `--mcp-config`, `--strict-mcp-config`) and names
//! no path under the worktree's `.claude/` ([`keeps_the_snapshot_line`]).
//!
//! **The candidates.**
//!
//! | n | line |
//! |---|---|
//! | 1 | the current line, started in the worktree |
//! | 2 | 1 plus `--add-dir <worktree>` and the variable that makes `claude` read `CLAUDE.md` of added directories |
//! | 3 | 1 plus the text of the worktree's `CLAUDE.md` files as an appended system prompt (a file of the snapshot) |
//! | 3b | the same text on the line itself (`--append-system-prompt`), which puts it in argv and in `ps` |
//! | 4 | `--setting-sources user,project`: loads project settings and hooks; here only to record it fails the rule |
//!
//! **The fixture.** A git repository in a temp root, with stand-in files only (`fixtures/repo-instructions/`): a root
//! `CLAUDE.md` with an `@docs/guide.md` import, a nested `CLAUDE.md`, a skill, an agent, project settings with a hook,
//! and, untracked, local settings with a hook and an `.mcp.json` with a server. Every hook and the server write a
//! marker file in the root's `markers/` folder, so a marker that appears is proof that the file loaded.

use super::common::TempRoot;
use super::common::git_repo::GitRepo;
use super::fake::FakeClaude;
use super::{plain_role, request, value_of};
use collide_core::adapter::{AgentAdapter, LaunchRequest, LaunchSpec};
use collide_host::adapters::claude::ClaudeAdapter;
use collide_host::adapters::claude::launch::{MCP_FILE, SETTINGS_FILE};
use collide_host::adapters::claude::snapshot::snapshot_files;
use collide_host::builtin_roles::BUILDER_DEFINITION;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// The words the fixture's files carry. A reply that holds one shows that its file loaded.
pub const ROOT_WORD: &str = "plum-4417";
pub const NESTED_WORD: &str = "quince-8302";
pub const IMPORT_WORD: &str = "fig-6159";
pub const SKILL_WORD: &str = "lime-2284";
pub const AGENT_WORD: &str = "kiwi-7730";

/// The names the fixture gives its skill, agent and MCP server.
pub const SKILL_NAME: &str = "repo-canary-skill";
pub const AGENT_NAME: &str = "repo-canary-agent";
pub const MCP_NAME: &str = "marker-server";

/// What the project and local hooks write to stderr, and the files all three markers are.
pub const PROJECT_HOOK_TEXT: &str = "project-hook-ran";
pub const LOCAL_HOOK_TEXT: &str = "local-hook-ran";
pub const PROJECT_HOOK_MARKER: &str = "project-hook";
pub const LOCAL_HOOK_MARKER: &str = "local-hook";
pub const MCP_MARKER: &str = "mcp-started";
pub const MARKERS: [&str; 3] = [PROJECT_HOOK_MARKER, LOCAL_HOOK_MARKER, MCP_MARKER];

/// The variable that makes `claude` read `CLAUDE.md` from the directories `--add-dir` adds. It is in the binary's
/// strings, not in `claude --help` (measured on 2.1.326); Part 2 finds out whether it does what its name says.
pub const ADD_DIR_VAR: &str = "CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD";

/// The file of the snapshot that holds candidate 3's text.
pub const INSTRUCTIONS_FILE: &str = "instructions.md";

/// Where the fixture's text comes from, and the path each file has in the repository.
const COMMITTED: [(&str, &str); 7] = [
    (
        "CLAUDE.md",
        include_str!("fixtures/repo-instructions/root-instructions.md"),
    ),
    (
        "docs/guide.md",
        include_str!("fixtures/repo-instructions/guide.md"),
    ),
    (
        "sub/CLAUDE.md",
        include_str!("fixtures/repo-instructions/nested-instructions.md"),
    ),
    (
        "sub/notes.txt",
        include_str!("fixtures/repo-instructions/notes.txt"),
    ),
    (
        "README.md",
        include_str!("fixtures/repo-instructions/readme.md"),
    ),
    (
        ".claude/skills/repo-canary-skill/SKILL.md",
        include_str!("fixtures/repo-instructions/skill.md"),
    ),
    (
        ".claude/agents/repo-canary-agent.md",
        include_str!("fixtures/repo-instructions/agent.md"),
    ),
];

/// The stand-in MCP server, copied into the root so its path is the root's own.
const MARKER_MCP: &str = include_str!("fixtures/repo-instructions/marker_mcp.py");

/// A fixture repository, its snapshot and its markers, all in one temp root.
pub struct Fixture {
    pub repo: GitRepo,
    pub snapshot: PathBuf,
    pub markers: PathBuf,
    mcp_script: PathBuf,
}

impl Fixture {
    /// The repository with its committed files, the snapshot a default role gets, an empty `markers/` folder and the
    /// MCP stand-in. The person's checkout is `repo.work()`; it has the committed files and nothing untracked.
    pub fn new() -> Fixture {
        let repo = GitRepo::new();
        let root = repo.root();
        let markers = root.join("markers");
        fs::create_dir(&markers).unwrap();
        let mcp_script = root.script("marker_mcp.py", MARKER_MCP);
        for (path, text) in COMMITTED {
            repo.write(path, text.as_bytes());
        }
        repo.write(
            ".claude/settings.json",
            hook_settings(PROJECT_HOOK_TEXT, &markers.join(PROJECT_HOOK_MARKER)).as_bytes(),
        );
        repo.commit_all("The stand-in files");

        let snapshot = root.join("snapshot");
        fs::create_dir(&snapshot).unwrap();
        let fixture = Fixture {
            repo,
            snapshot,
            markers,
            mcp_script,
        };
        let request = fixture.request(plain_role(), fixture.repo.work());
        let files =
            snapshot_files(&request, BUILDER_DEFINITION, &Map::new()).expect("the snapshot");
        for (file, text) in files.named() {
            fs::write(fixture.snapshot.join(file), text).unwrap();
        }
        fixture
    }

    pub fn root(&self) -> &TempRoot {
        self.repo.root()
    }

    /// The launch request of the role `role` that starts in `cwd` and has this fixture's snapshot.
    pub fn request(&self, role: Value, cwd: &Path) -> LaunchRequest {
        let mut request = request(role);
        request.cwd = cwd.to_path_buf();
        request.snapshot_dir = self.snapshot.clone();
        request
    }

    /// A worktree of the repository, made with `git worktree add` on a branch of the same name below the root, with
    /// the untracked local files ([`Fixture::add_local_files`]) in it.
    pub fn worktree(&self, name: &str) -> PathBuf {
        let path = self.root().join(name);
        self.repo.git(&[
            "worktree",
            "add",
            "--quiet",
            "-b",
            name,
            path.to_str().unwrap(),
        ]);
        self.add_local_files(&path);
        path
    }

    /// Untracked files that a person or an agent leaves in a checkout: local settings with a hook, and an `.mcp.json`
    /// with a server. Each writes its marker if `claude` loads it.
    pub fn add_local_files(&self, dir: &Path) {
        fs::create_dir_all(dir.join(".claude")).unwrap();
        fs::write(
            dir.join(".claude/settings.local.json"),
            hook_settings(LOCAL_HOOK_TEXT, &self.markers.join(LOCAL_HOOK_MARKER)),
        )
        .unwrap();
        let server = json!({"mcpServers": {MCP_NAME: {
            "command": super::common::PYTHON,
            "args": [self.mcp_script, self.markers.join(MCP_MARKER)],
        }}});
        fs::write(dir.join(".mcp.json"), server.to_string()).unwrap();
    }

    /// The markers that exist, in the order of [`MARKERS`].
    pub fn markers_present(&self) -> Vec<&'static str> {
        MARKERS
            .into_iter()
            .filter(|name| self.markers.join(name).exists())
            .collect()
    }

    /// Removes the three marker files, each by its own path.
    pub fn clear_markers(&self) {
        for name in MARKERS {
            let _ = fs::remove_file(self.markers.join(name));
        }
    }
}

/// A settings file with one `SessionStart` hook that says `text` on stderr and creates `marker`.
fn hook_settings(text: &str, marker: &Path) -> String {
    let command = format!("echo {text} >&2; touch '{}'", marker.display());
    json!({"hooks": {"SessionStart": [{"hooks": [{"type": "command", "command": command}]}]}})
        .to_string()
}

/// The text of every `CLAUDE.md` below `dir` (any depth, not below `.git`), in path order, each under a header with its
/// path. It is what Collide would read to pass the instructions as a system prompt: `@` imports are not followed and
/// skills are not included.
pub fn instruction_text(dir: &Path) -> String {
    fn find(root: &Path, dir: &Path, found: &mut Vec<String>) {
        for entry in fs::read_dir(dir).unwrap().flatten() {
            let path = entry.path();
            if entry.file_name() == ".git" {
                continue;
            }
            if path.is_dir() {
                find(root, &path, found);
            } else if entry.file_name() == "CLAUDE.md" {
                found.push(path.strip_prefix(root).unwrap().display().to_string());
            }
        }
    }
    let mut files = Vec::new();
    find(dir, dir, &mut files);
    files.sort();
    files
        .iter()
        .map(|path| {
            format!(
                "# {path}\n{}\n",
                fs::read_to_string(dir.join(path)).unwrap()
            )
        })
        .collect()
}

/// What Collide would start `claude` with: the words, the variables it sets and the folder it starts in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Line {
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: PathBuf,
}

impl Line {
    pub fn of(spec: LaunchSpec) -> Line {
        Line {
            argv: spec.argv,
            env: spec.env,
            cwd: spec.cwd,
        }
    }

    pub fn value_of(&self, flag: &str) -> Option<&str> {
        value_of(&self.argv, flag)
    }

    /// Sets the value of `--setting-sources`, which the line has.
    fn set_setting_sources(&mut self, value: &str) {
        let at = self
            .argv
            .iter()
            .position(|word| word == "--setting-sources");
        self.argv[at.expect("the line has --setting-sources") + 1] = value.to_string();
    }
}

/// The lines under measurement (see the table in the module's documentation).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Candidate {
    Baseline,
    AddDir,
    AppendedPromptFile,
    AppendedPromptInline,
    ProjectSource,
}

/// The line of `candidate` for `request`, started in the request's folder. Candidate 3 writes its file in the fixture's
/// snapshot.
pub fn candidate_line(fixture: &Fixture, candidate: Candidate, request: &LaunchRequest) -> Line {
    let cwd = request.cwd.as_path();
    let mut line = Line::of(ClaudeAdapter.launch(request).expect("the launch line"));
    match candidate {
        Candidate::Baseline => {}
        Candidate::AddDir => {
            line.argv
                .extend(["--add-dir".into(), cwd.display().to_string()]);
            line.env.insert(ADD_DIR_VAR.into(), "1".into());
        }
        Candidate::AppendedPromptFile => {
            let file = fixture.snapshot.join(INSTRUCTIONS_FILE);
            fs::write(&file, instruction_text(cwd)).unwrap();
            line.argv.extend([
                "--append-system-prompt-file".into(),
                file.display().to_string(),
            ]);
        }
        Candidate::AppendedPromptInline => {
            line.argv
                .extend(["--append-system-prompt".into(), instruction_text(cwd)]);
        }
        Candidate::ProjectSource => line.set_setting_sources("user,project"),
    }
    line
}

/// Whether `line` leaves settings, hooks and MCP config to the snapshot (the rule in the module's documentation). The
/// error says which part it breaks. `--setting-sources` may be `user` or empty (config none), never `project` or
/// `local`.
pub fn keeps_the_snapshot_line(
    line: &Line,
    snapshot: &Path,
    worktree: &Path,
) -> Result<(), String> {
    match line.value_of("--setting-sources") {
        Some("user" | "") => {}
        Some(other) => {
            return Err(format!(
                "--setting-sources `{other}` loads settings from the repository: only `user` or none may be named"
            ));
        }
        None => return Err("the line has no --setting-sources".into()),
    }
    for (flag, file) in [("--settings", SETTINGS_FILE), ("--mcp-config", MCP_FILE)] {
        let want = snapshot.join(file);
        if line.value_of(flag).map(Path::new) != Some(want.as_path()) {
            return Err(format!("{flag} is not the snapshot's {}", want.display()));
        }
    }
    if !line.argv.iter().any(|word| word == "--strict-mcp-config") {
        return Err("--strict-mcp-config is missing".into());
    }
    let project = worktree.join(".claude");
    if let Some(word) = line
        .argv
        .iter()
        .find(|word| Path::new(word).starts_with(&project))
    {
        return Err(format!("the line names a project settings path: {word}"));
    }
    Ok(())
}

/// What fake Claude recorded of its own start.
#[derive(Debug)]
pub struct Seen {
    pub argv: Vec<String>,
    pub env: BTreeMap<String, String>,
    pub cwd: PathBuf,
}

/// Starts fake Claude on `line` (its words, variables and folder), has it record what it was started with, and returns
/// that. Its environment is its own (`env_clear()`, then `PATH`, `HOME` in the root and the line's variables).
pub fn through_the_fake(root: &TempRoot, line: &Line) -> Seen {
    let recorded = root.join("launch-seen.json");
    let recording = root.join("launch.jsonl");
    fs::write(&recording, json!({ "dump_launch": recorded }).to_string()).unwrap();
    let words: Vec<&str> = line.argv.iter().map(String::as_str).collect();
    let mut fake = FakeClaude::start_on_line(root, &recording, false, &words, &line.cwd, &line.env);
    fake.close_stdin();
    let ended = fake.finish();
    assert_eq!(ended.code, Some(0), "{}", ended.stderr);
    let seen: Value = serde_json::from_str(&fs::read_to_string(&recorded).unwrap()).unwrap();
    let text = |value: &Value| value.as_str().unwrap().to_string();
    Seen {
        argv: seen["argv"].as_array().unwrap().iter().map(text).collect(),
        env: seen["env"]
            .as_object()
            .unwrap()
            .iter()
            .map(|(key, value)| (key.clone(), text(value)))
            .collect(),
        cwd: PathBuf::from(text(&seen["cwd"])),
    }
}

fn same_folder(a: &Path, b: &Path) -> bool {
    fs::canonicalize(a).unwrap() == fs::canonicalize(b).unwrap()
}

/// What every candidate's test checks of what fake Claude saw: the words, the variables and the folder the line
/// names, a home in the temp root, and no variable of `claude`'s that the line did not set.
fn the_fake_saw(fixture: &Fixture, line: &Line) -> Seen {
    let seen = through_the_fake(fixture.root(), line);
    assert_eq!(seen.argv, line.argv);
    assert!(same_folder(&seen.cwd, &line.cwd), "{:?}", seen.cwd);
    for (name, value) in &line.env {
        assert_eq!(seen.env.get(name), Some(value), "{name}");
    }
    assert_eq!(
        seen.env.get("HOME").map(PathBuf::from),
        Some(fixture.root().path().to_path_buf())
    );
    let unset_by_the_line = seen
        .env
        .keys()
        .filter(|name| name.starts_with("CLAUDE") && !line.env.contains_key(*name));
    assert_eq!(unset_by_the_line.count(), 0);
    seen
}

// ---- the fixture -------------------------------------------------------------------------------------------------

#[test]
fn the_worktree_holds_the_canary_files_and_the_untracked_local_ones() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    for (path, _) in COMMITTED {
        assert!(worktree.join(path).is_file(), "{path}");
    }
    let has = |path: &str, word: &str| {
        fs::read_to_string(worktree.join(path))
            .unwrap()
            .contains(word)
    };
    assert!(has("CLAUDE.md", ROOT_WORD) && has("CLAUDE.md", "@docs/guide.md"));
    assert!(has("docs/guide.md", IMPORT_WORD));
    assert!(has("sub/CLAUDE.md", NESTED_WORD));
    assert!(has(".claude/skills/repo-canary-skill/SKILL.md", SKILL_WORD));
    assert!(has(".claude/agents/repo-canary-agent.md", AGENT_WORD));
    assert!(has(".claude/settings.json", PROJECT_HOOK_TEXT));
    assert!(has(".claude/settings.local.json", LOCAL_HOOK_TEXT));
    assert!(has(".mcp.json", MCP_NAME));
}

#[test]
fn the_worktree_is_a_git_worktree_of_the_checkout_and_the_local_files_are_untracked_in_it() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let listed = fixture.repo.git(&["worktree", "list", "--porcelain"]);
    assert_eq!(listed.matches("worktree ").count(), 2, "{listed}");
    assert!(
        worktree.join(".git").is_file(),
        "a worktree's .git is a file"
    );
    let status = super::common::git_repo::git_text(
        fixture.root().path(),
        &worktree,
        &["status", "--porcelain", "--untracked-files=all"],
    );
    assert_eq!(
        status.lines().collect::<Vec<_>>(),
        ["?? .claude/settings.local.json", "?? .mcp.json"]
    );
}

#[test]
fn the_persons_checkout_starts_with_no_local_files_and_takes_them_untracked() {
    let fixture = Fixture::new();
    let checkout = fixture.repo.work();
    assert!(!checkout.join(".claude/settings.local.json").exists());
    assert!(!checkout.join(".mcp.json").exists());
    fixture.add_local_files(checkout);
    assert_eq!(
        fixture
            .repo
            .git(&["status", "--porcelain", "--untracked-files=all"])
            .lines()
            .collect::<Vec<_>>(),
        ["?? .claude/settings.local.json", "?? .mcp.json"]
    );
}

#[test]
fn the_markers_are_made_by_the_hooks_and_the_server_and_cleared_one_by_one() {
    let fixture = Fixture::new();
    assert_eq!(fixture.markers_present(), Vec::<&str>::new());
    let worktree = fixture.worktree("wt");
    for (file, marker) in [
        (".claude/settings.json", PROJECT_HOOK_MARKER),
        (".claude/settings.local.json", LOCAL_HOOK_MARKER),
    ] {
        let settings: Value =
            serde_json::from_str(&fs::read_to_string(worktree.join(file)).unwrap()).unwrap();
        let command = settings["hooks"]["SessionStart"][0]["hooks"][0]["command"]
            .as_str()
            .unwrap();
        let ran = std::process::Command::new("/bin/sh")
            .args(["-c", command])
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .output()
            .unwrap();
        assert!(ran.status.success());
        assert!(fixture.markers.join(marker).exists(), "{marker}");
    }
    let server: Value =
        serde_json::from_str(&fs::read_to_string(worktree.join(".mcp.json")).unwrap()).unwrap();
    let command = &server["mcpServers"][MCP_NAME];
    let args: Vec<&str> = command["args"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let started = std::process::Command::new(command["command"].as_str().unwrap())
        .args(&args)
        .env_clear()
        .stdin(std::process::Stdio::null())
        .status()
        .unwrap();
    assert!(started.success());
    assert_eq!(fixture.markers_present(), MARKERS);
    fixture.clear_markers();
    assert_eq!(fixture.markers_present(), Vec::<&str>::new());
}

// ---- the candidates, each through fake Claude --------------------------------------------------------------------

#[test]
fn candidate_1_is_the_current_line_started_in_the_worktree() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let line = candidate_line(
        &fixture,
        Candidate::Baseline,
        &fixture.request(plain_role(), &worktree),
    );

    let own = ClaudeAdapter
        .launch(&fixture.request(plain_role(), &worktree))
        .unwrap();
    assert_eq!(line, Line::of(own));
    assert_eq!(line.cwd, worktree);
    assert!(line.env.is_empty());
    assert_eq!(
        keeps_the_snapshot_line(&line, &fixture.snapshot, &worktree),
        Ok(())
    );
    assert_eq!(line.value_of("--setting-sources"), Some("user"));
    assert!(
        !line
            .argv
            .iter()
            .any(|word| word.contains("--add-dir") || word.contains("system-prompt"))
    );

    let seen = the_fake_saw(&fixture, &line);
    assert!(
        !same_folder(&seen.cwd, fixture.repo.work()),
        "the worktree, not the checkout"
    );
}

#[test]
fn candidate_2_adds_the_worktree_and_the_variable_and_keeps_the_snapshot_flags() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let line = candidate_line(
        &fixture,
        Candidate::AddDir,
        &fixture.request(plain_role(), &worktree),
    );

    assert_eq!(line.value_of("--add-dir"), Some(worktree.to_str().unwrap()));
    assert_eq!(
        line.env,
        BTreeMap::from([(ADD_DIR_VAR.to_string(), "1".to_string())])
    );
    assert_eq!(
        keeps_the_snapshot_line(&line, &fixture.snapshot, &worktree),
        Ok(())
    );
    let baseline = candidate_line(
        &fixture,
        Candidate::Baseline,
        &fixture.request(plain_role(), &worktree),
    );
    assert_eq!(line.argv[..baseline.argv.len()], baseline.argv[..]);

    let seen = the_fake_saw(&fixture, &line);
    assert_eq!(seen.env.get(ADD_DIR_VAR).map(String::as_str), Some("1"));
}

#[test]
fn candidate_3_passes_the_claude_md_text_as_an_appended_prompt_file_and_keeps_the_snapshot_flags() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let line = candidate_line(
        &fixture,
        Candidate::AppendedPromptFile,
        &fixture.request(plain_role(), &worktree),
    );

    let file = fixture.snapshot.join(INSTRUCTIONS_FILE);
    assert_eq!(
        line.value_of("--append-system-prompt-file"),
        Some(file.to_str().unwrap())
    );
    assert!(line.env.is_empty());
    assert_eq!(
        keeps_the_snapshot_line(&line, &fixture.snapshot, &worktree),
        Ok(())
    );
    let text = fs::read_to_string(&file).unwrap();
    assert!(
        text.contains(ROOT_WORD) && text.contains(NESTED_WORD),
        "{text}"
    );
    assert!(
        line.argv.iter().all(|word| !word.contains(ROOT_WORD)),
        "the text is in the file, not in argv or in `ps`"
    );

    the_fake_saw(&fixture, &line);
}

#[test]
fn candidate_3_text_holds_every_claude_md_and_neither_the_imports_nor_the_skills() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let text = instruction_text(&worktree);
    assert!(text.contains(ROOT_WORD) && text.contains(NESTED_WORD));
    assert!(
        text.contains("# CLAUDE.md\n") && text.contains("# sub/CLAUDE.md\n"),
        "{text}"
    );
    assert!(
        text.contains("@docs/guide.md"),
        "the import is text, not followed"
    );
    assert!(!text.contains(IMPORT_WORD), "an @ import is not loaded");
    assert!(!text.contains(SKILL_WORD), "a skill is not loaded");
    assert!(!text.contains(AGENT_WORD), "an agent is not loaded");
}

#[test]
fn candidate_3b_puts_the_same_text_on_the_line_which_is_why_the_file_form_is_the_one() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let line = candidate_line(
        &fixture,
        Candidate::AppendedPromptInline,
        &fixture.request(plain_role(), &worktree),
    );

    assert_eq!(
        line.value_of("--append-system-prompt"),
        Some(instruction_text(&worktree).as_str())
    );
    assert!(
        line.argv.iter().any(|word| word.contains(ROOT_WORD)),
        "the text is in argv"
    );
    assert_eq!(
        keeps_the_snapshot_line(&line, &fixture.snapshot, &worktree),
        Ok(())
    );

    the_fake_saw(&fixture, &line);
}

#[test]
fn candidate_4_loads_project_settings_so_the_rule_refuses_it_and_the_message_names_user_project() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let line = candidate_line(
        &fixture,
        Candidate::ProjectSource,
        &fixture.request(plain_role(), &worktree),
    );

    assert_eq!(line.value_of("--setting-sources"), Some("user,project"));
    let refusal = keeps_the_snapshot_line(&line, &fixture.snapshot, &worktree).unwrap_err();
    assert!(refusal.contains("user,project"), "{refusal}");
    assert!(refusal.contains("--setting-sources"), "{refusal}");

    let seen = the_fake_saw(&fixture, &line);
    assert!(seen.argv.iter().any(|word| word == "user,project"));
}

// ---- the rule ----------------------------------------------------------------------------------------------------

#[test]
fn the_rule_names_each_snapshot_flag_a_line_loses() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let baseline = candidate_line(
        &fixture,
        Candidate::Baseline,
        &fixture.request(plain_role(), &worktree),
    );
    for (flag, name) in [
        ("--strict-mcp-config", "--strict-mcp-config"),
        ("--settings", "--settings"),
        ("--mcp-config", "--mcp-config"),
        ("--setting-sources", "--setting-sources"),
    ] {
        let mut line = baseline.clone();
        let at = line.argv.iter().position(|word| word == flag).unwrap();
        let width = if flag == "--strict-mcp-config" { 1 } else { 2 };
        line.argv.drain(at..at + width);
        let refusal = keeps_the_snapshot_line(&line, &fixture.snapshot, &worktree).unwrap_err();
        assert!(refusal.contains(name), "{flag}: {refusal}");
    }
}

#[test]
fn the_rule_refuses_a_line_that_names_a_project_settings_path_and_one_that_names_local_as_a_source()
{
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let baseline = candidate_line(
        &fixture,
        Candidate::Baseline,
        &fixture.request(plain_role(), &worktree),
    );

    let mut line = baseline.clone();
    line.argv.extend([
        "--settings".into(),
        worktree.join(".claude/settings.json").display().to_string(),
    ]);
    let refusal = keeps_the_snapshot_line(&line, &fixture.snapshot, &worktree).unwrap_err();
    assert!(refusal.contains(".claude/settings.json"), "{refusal}");

    let mut line = baseline.clone();
    line.argv.extend([
        "--add-dir".into(),
        worktree.join(".claude").display().to_string(),
    ]);
    let refusal = keeps_the_snapshot_line(&line, &fixture.snapshot, &worktree).unwrap_err();
    assert!(refusal.contains("project settings path"), "{refusal}");

    let mut line = baseline;
    line.set_setting_sources("user,local");
    let refusal = keeps_the_snapshot_line(&line, &fixture.snapshot, &worktree).unwrap_err();
    assert!(refusal.contains("user,local"), "{refusal}");
}

#[test]
fn the_rule_accepts_config_none_which_is_an_empty_setting_sources() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let none = json!({ "agent": "claude", "config": "none" });
    let line = candidate_line(
        &fixture,
        Candidate::Baseline,
        &fixture.request(none, &worktree),
    );
    assert_eq!(line.value_of("--setting-sources"), Some(""));
    assert_eq!(
        keeps_the_snapshot_line(&line, &fixture.snapshot, &worktree),
        Ok(())
    );
}
