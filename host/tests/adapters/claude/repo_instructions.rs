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
//! | 5 | 2 plus `--plugin-dir <snapshot>/plugins/repo-skills`, holding only a link to the skills (ts-0106-nr) |
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

/// The plugin of candidate 5: its name (skills of a plugin may be listed as `<name>:<skill>`), and the folder of the
/// snapshot that holds it (0.3, the launch line: `--plugin-dir <snapshot>/plugins/<name>`).
pub const PLUGIN_NAME: &str = "repo-skills";
pub const PLUGINS_DIR: &str = "plugins";

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
    /// 2 plus the snapshot plugin whose `skills` is a link to the worktree's `.claude/skills` (ts-0106-nr).
    AddDirSkillsPlugin,
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
        Candidate::AddDirSkillsPlugin => {
            let plugin = write_skills_plugin(&fixture.snapshot, cwd);
            line.argv
                .extend(["--add-dir".into(), cwd.display().to_string()]);
            line.argv
                .extend(["--plugin-dir".into(), plugin.display().to_string()]);
            line.env.insert(ADD_DIR_VAR.into(), "1".into());
        }
    }
    line
}

/// Writes the plugin of candidate 5 in `snapshot` and returns its folder: a manifest with a name and a description,
/// and `skills`, a link to `<start_dir>/.claude/skills`, so the skills are read from the folder the phase starts in
/// when `claude` loads them and are not frozen at the line's commit. It holds nothing else: no hooks, MCP servers,
/// agents or commands.
pub fn write_skills_plugin(snapshot: &Path, start_dir: &Path) -> PathBuf {
    let folder = snapshot.join(PLUGINS_DIR).join(PLUGIN_NAME);
    fs::create_dir_all(folder.join(".claude-plugin")).unwrap();
    let manifest = json!({
        "name": PLUGIN_NAME,
        "description": "The repository's own skills, read from the folder a phase starts in.",
    });
    fs::write(
        folder.join(".claude-plugin/plugin.json"),
        manifest.to_string(),
    )
    .unwrap();
    std::os::unix::fs::symlink(start_dir.join(".claude/skills"), folder.join("skills")).unwrap();
    folder
}

/// The names a folder holds, sorted.
fn names_in(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = fs::read_dir(dir)
        .unwrap()
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    names
}

/// Whether the plugin folder `folder` holds the manifest and the link to `<start_dir>/.claude/skills` and nothing
/// else: no other file, a manifest with only a name, a description and a version (a key such as `hooks`, `mcpServers`,
/// `agents` or `commands` would add code), and a `skills` that is a link that resolves to the start folder's skills.
/// The error says which part it breaks.
pub fn holds_only_the_skills_link(folder: &Path, start_dir: &Path) -> Result<(), String> {
    let top = names_in(folder);
    if top != [".claude-plugin", "skills"] {
        return Err(format!(
            "the plugin folder holds {top:?}, not the manifest folder and `skills`"
        ));
    }
    let manifest_folder = names_in(&folder.join(".claude-plugin"));
    if manifest_folder != ["plugin.json"] {
        return Err(format!(
            "the manifest folder holds {manifest_folder:?}, not `plugin.json`"
        ));
    }
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(folder.join(".claude-plugin/plugin.json")).unwrap(),
    )
    .map_err(|error| format!("the manifest is not JSON: {error}"))?;
    let keys = manifest
        .as_object()
        .ok_or("the manifest is not an object")?;
    if let Some(key) = keys
        .keys()
        .find(|key| !["name", "description", "version"].contains(&key.as_str()))
    {
        return Err(format!(
            "the manifest has the key `{key}`, which is more than a name"
        ));
    }
    let skills = folder.join("skills");
    if !fs::symlink_metadata(&skills)
        .unwrap()
        .file_type()
        .is_symlink()
    {
        return Err("`skills` is not a link".into());
    }
    let wanted = start_dir.join(".claude/skills");
    match (fs::canonicalize(&skills), fs::canonicalize(&wanted)) {
        (Ok(linked), Ok(wanted)) if linked == wanted => Ok(()),
        (Ok(linked), _) => Err(format!(
            "`skills` resolves to {}, not the start folder's skills",
            linked.display()
        )),
        (Err(error), _) => Err(format!("`skills` does not resolve: {error}")),
    }
}

/// Whether `line` leaves settings, hooks and MCP config to the snapshot (the rule in the module's documentation). The
/// error says which part it breaks. `--setting-sources` may be `user` or empty (config none), never `project` or
/// `local`. A `--plugin-dir` is a folder of the snapshot, never one of the repository.
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
    for at in line
        .argv
        .iter()
        .enumerate()
        .filter_map(|(at, word)| (word == "--plugin-dir").then_some(at))
    {
        let plugin = line.argv.get(at + 1).map(Path::new);
        if !plugin.is_some_and(|plugin| plugin.starts_with(snapshot)) {
            return Err(format!(
                "--plugin-dir {plugin:?} is not a folder of the snapshot"
            ));
        }
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

#[test]
fn candidate_5_adds_the_skills_plugin_to_candidate_2_and_keeps_the_snapshot_flags() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let request = fixture.request(plain_role(), &worktree);
    let line = candidate_line(&fixture, Candidate::AddDirSkillsPlugin, &request);
    let candidate_2 = candidate_line(&fixture, Candidate::AddDir, &request);

    let plugin = fixture.snapshot.join("plugins").join(PLUGIN_NAME);
    assert_eq!(
        line.value_of("--plugin-dir"),
        Some(plugin.to_str().unwrap())
    );
    assert_eq!(line.argv[..candidate_2.argv.len()], candidate_2.argv[..]);
    assert_eq!(line.argv.len(), candidate_2.argv.len() + 2);
    assert_eq!(line.value_of("--add-dir"), Some(worktree.to_str().unwrap()));
    assert_eq!(line.env, candidate_2.env);
    assert_eq!(line.cwd, worktree);
    assert_eq!(
        keeps_the_snapshot_line(&line, &fixture.snapshot, &worktree),
        Ok(())
    );
    assert!(
        line.argv.iter().any(|word| word == "--strict-mcp-config"),
        "the four snapshot flags are still present"
    );
    assert!(
        line.argv
            .iter()
            .all(|word| !Path::new(word).starts_with(worktree.join(".claude"))),
        "no word of the line is a path under the worktree's .claude"
    );

    let seen = the_fake_saw(&fixture, &line);
    assert_eq!(seen.env.get(ADD_DIR_VAR).map(String::as_str), Some("1"));
}

#[test]
fn the_skills_plugin_folder_holds_only_the_manifest_and_the_link_to_the_worktrees_skills() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let plugin = write_skills_plugin(&fixture.snapshot, &worktree);

    assert_eq!(names_in(&plugin), [".claude-plugin", "skills"]);
    assert_eq!(names_in(&plugin.join(".claude-plugin")), ["plugin.json"]);
    let manifest: Value = serde_json::from_str(
        &fs::read_to_string(plugin.join(".claude-plugin/plugin.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(manifest["name"], PLUGIN_NAME);
    assert_eq!(holds_only_the_skills_link(&plugin, &worktree), Ok(()));

    let link = plugin.join("skills");
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert_eq!(
        fs::canonicalize(&link).unwrap(),
        fs::canonicalize(worktree.join(".claude/skills")).unwrap()
    );
    let skill = fs::read_to_string(link.join("repo-canary-skill/SKILL.md")).unwrap();
    assert!(
        skill.contains(SKILL_WORD),
        "the skill is read through the link"
    );
    assert!(
        !plugin.starts_with(&worktree),
        "the plugin is in the snapshot, not in the worktree"
    );
}

#[test]
fn the_skills_plugin_shows_the_worktrees_skills_as_they_are_now_not_as_committed() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let plugin = write_skills_plugin(&fixture.snapshot, &worktree);
    let skill = worktree.join(".claude/skills/repo-canary-skill/SKILL.md");

    fs::write(
        &skill,
        "---\nname: repo-canary-skill\ndescription: Edited after the plugin was made.\n---\n",
    )
    .unwrap();
    fs::create_dir(worktree.join(".claude/skills/later-skill")).unwrap();
    fs::write(
        worktree.join(".claude/skills/later-skill/SKILL.md"),
        "later",
    )
    .unwrap();

    let through = |path: &str| fs::read_to_string(plugin.join("skills").join(path)).unwrap();
    assert!(through("repo-canary-skill/SKILL.md").contains("Edited after the plugin was made"));
    assert_eq!(through("later-skill/SKILL.md"), "later");
    assert_eq!(
        names_in(&plugin.join("skills")),
        ["later-skill", "repo-canary-skill"]
    );
}

#[test]
fn the_plugin_check_refuses_more_than_the_manifest_and_the_link() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let fresh = |name: &str| {
        let snapshot = fixture.root().join(name);
        fs::create_dir(&snapshot).unwrap();
        (write_skills_plugin(&snapshot, &worktree), snapshot)
    };

    let (plugin, _) = fresh("a");
    fs::create_dir(plugin.join("hooks")).unwrap();
    fs::write(plugin.join("hooks/hooks.json"), "{}").unwrap();
    let refusal = holds_only_the_skills_link(&plugin, &worktree).unwrap_err();
    assert!(refusal.contains("hooks"), "{refusal}");

    let (plugin, _) = fresh("b");
    fs::write(plugin.join(".claude-plugin/extra.json"), "{}").unwrap();
    let refusal = holds_only_the_skills_link(&plugin, &worktree).unwrap_err();
    assert!(refusal.contains("extra.json"), "{refusal}");

    for key in ["hooks", "mcpServers", "agents", "commands"] {
        let (plugin, _) = fresh(&format!("key-{key}"));
        let manifest = json!({"name": PLUGIN_NAME, key: "./x"});
        fs::write(
            plugin.join(".claude-plugin/plugin.json"),
            manifest.to_string(),
        )
        .unwrap();
        let refusal = holds_only_the_skills_link(&plugin, &worktree).unwrap_err();
        assert!(refusal.contains(key), "{key}: {refusal}");
    }

    let (plugin, _) = fresh("c");
    fs::remove_file(plugin.join("skills")).unwrap();
    fs::create_dir(plugin.join("skills")).unwrap();
    let refusal = holds_only_the_skills_link(&plugin, &worktree).unwrap_err();
    assert!(refusal.contains("not a link"), "{refusal}");

    let (plugin, _) = fresh("d");
    fs::remove_file(plugin.join("skills")).unwrap();
    std::os::unix::fs::symlink(worktree.join("sub"), plugin.join("skills")).unwrap();
    let refusal = holds_only_the_skills_link(&plugin, &worktree).unwrap_err();
    assert!(
        refusal.contains("not the start folder's skills"),
        "{refusal}"
    );

    let (plugin, _) = fresh("e");
    let other = fixture.worktree("other");
    let refusal = holds_only_the_skills_link(&plugin, &other).unwrap_err();
    assert!(
        refusal.contains("not the start folder's skills"),
        "{refusal}"
    );
}

#[test]
fn the_rule_refuses_a_plugin_folder_that_is_not_in_the_snapshot() {
    let fixture = Fixture::new();
    let worktree = fixture.worktree("wt");
    let request = fixture.request(plain_role(), &worktree);
    let baseline = candidate_line(&fixture, Candidate::Baseline, &request);

    let mut line = baseline.clone();
    line.argv.extend([
        "--plugin-dir".into(),
        worktree.join("plugins/p").display().to_string(),
    ]);
    let refusal = keeps_the_snapshot_line(&line, &fixture.snapshot, &worktree).unwrap_err();
    assert!(
        refusal.contains("--plugin-dir") && refusal.contains("plugins/p"),
        "{refusal}"
    );

    let mut line = baseline.clone();
    line.argv.push("--plugin-dir".into());
    let refusal = keeps_the_snapshot_line(&line, &fixture.snapshot, &worktree).unwrap_err();
    assert!(refusal.contains("--plugin-dir"), "{refusal}");

    let mut line = baseline;
    line.argv.extend([
        "--plugin-dir".into(),
        fixture.snapshot.join("plugins/p").display().to_string(),
    ]);
    assert_eq!(
        keeps_the_snapshot_line(&line, &fixture.snapshot, &worktree),
        Ok(())
    );
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
