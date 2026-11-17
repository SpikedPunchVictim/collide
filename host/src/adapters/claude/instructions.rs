//! The repository's own instructions on the Claude line (ts-0104-gx; 0.3, The repository's own instructions).
//!
//! With `repo_instructions: worktree` (the default) the line adds the folder the phase starts in with `--add-dir`, and
//! [`ADD_DIR_VAR`], which makes `claude` read the `CLAUDE.md` of an added folder. The skills of that folder come
//! through a plugin in the phase's snapshot ([`write_skills_plugin`]): a manifest and a `skills` link to
//! `<folder>/.claude/skills`, so the skills are read when `claude` loads them and are not copied. Settings, hooks and
//! MCP config still come from the snapshot. A skill's own frontmatter `hooks` and `allowed-tools` take effect through
//! the plugin (measured `ts-0107-rj`), as they would in plain `claude` in that folder; the owner accepted that
//! (2026-11-17). Each `Skill` call still asks the host first.
//!
//! [`is_instruction_path`] is the one list of paths the run's warning reads: a file `claude` loads as instructions.
//!
//! The plugin is a seam: [`PLUGIN_ON_THE_LINE`] says whether the line names it.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// The variable that makes `claude` read `CLAUDE.md` from the folders `--add-dir` adds. It is not in `claude --help`;
/// the contract test pins it (`ts-0103-sa`).
pub const ADD_DIR_VAR: &str = "CLAUDE_CODE_ADDITIONAL_DIRECTORIES_CLAUDE_MD";

/// The folder of the snapshot that holds plugins, and the one plugin this item writes.
pub const PLUGINS_DIR: &str = "plugins";
pub const PLUGIN_NAME: &str = "repo-skills";

/// Whether the `worktree` line names the skills plugin. On since `ts-0107-rj` (owner, 2026-11-17: a skill's own
/// `hooks` and `allowed-tools` are accepted as plain `claude` takes them). The code that writes the plugin, and its
/// tests, stay whatever this says.
pub const PLUGIN_ON_THE_LINE: bool = true;

const MANIFEST_DIR: &str = ".claude-plugin";
const MANIFEST_FILE: &str = "plugin.json";
const SKILLS_LINK: &str = "skills";
const DESCRIPTION: &str = "The repository's own skills, read from the folder a phase starts in.";

/// The plugin's folder in a phase's snapshot.
pub fn skills_plugin_dir(snapshot_dir: &Path) -> PathBuf {
    snapshot_dir.join(PLUGINS_DIR).join(PLUGIN_NAME)
}

/// Whether `path`, relative to the workspace and written with `/`, is a file `claude` loads as instructions:
/// `CLAUDE.md` at any depth, and anything below the root's `.claude/skills` folder (the one the plugin links).
/// `.claude/commands` is not here, because commands do not load on the line; `AGENTS.md` is not, because the line
/// does not read it. An `@`-imported file is not followed.
pub fn is_instruction_path(path: &str) -> bool {
    path.rsplit('/').next() == Some("CLAUDE.md") || path.starts_with(".claude/skills/")
}

/// Writes the skills plugin in `snapshot_dir` when `<start_dir>/.claude/skills` is a folder, and says whether it did.
/// With no such folder it writes nothing and takes out a plugin an earlier attempt of the phase wrote, so the line
/// never names a link that points at nothing. Writing again replaces the plugin: the link follows `start_dir`.
pub fn write_skills_plugin(snapshot_dir: &Path, start_dir: &Path) -> io::Result<bool> {
    let folder = skills_plugin_dir(snapshot_dir);
    remove_if_there(&folder)?;
    let skills = start_dir.join(".claude").join("skills");
    if !skills.is_dir() {
        return Ok(false);
    }
    fs::create_dir_all(folder.join(MANIFEST_DIR))?;
    let manifest = format!("{{\"name\":\"{PLUGIN_NAME}\",\"description\":\"{DESCRIPTION}\"}}");
    fs::write(folder.join(MANIFEST_DIR).join(MANIFEST_FILE), manifest)?;
    std::os::unix::fs::symlink(&skills, folder.join(SKILLS_LINK))?;
    Ok(true)
}

/// Removes the plugin folder this module wrote, by its exact path. A folder that is not there is not an error.
fn remove_if_there(folder: &Path) -> io::Result<()> {
    match fs::remove_dir_all(folder) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
