//! The load checks of the agent entries that need the machine (ts-0099-bw; 0.7, Agent entries; Stage 4a).
//!
//! The checks that need one file only are `core`'s ([`collide_core::settings::check_agents`]). These two need the
//! disk and the keychain, so they are `host`'s:
//! - a `read` folder that does not exist is an error naming the key and the folder. `<board root>` stands for the
//!   board's root, the default line's root; with no board it has no meaning and is not checked;
//! - a `secrets` entry whose keychain item is missing is an error naming the item and the key. Only
//!   [`SecretStore::contains`] is asked: the value is read at launch (Stage 6b), never at load.

use crate::secret_store::SecretStore;
use collide_core::agent_layers::AgentView;
use collide_core::owner::SettingLayer;
use collide_core::settings_layers::SettingProblem;
use std::path::{Path, PathBuf};

/// What `read` writes for the board's root.
pub const BOARD_ROOT: &str = "<board root>";

/// The folder a `read` entry names: `<board root>` and `<board root>/...` replaced by `board_root`. `None` for an entry
/// that starts at the board's root when there is no board.
pub fn read_folder(entry: &str, board_root: Option<&Path>) -> Option<PathBuf> {
    match entry.strip_prefix(BOARD_ROOT) {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => {
            Some(board_root?.join(rest.trim_start_matches('/')))
        }
        _ => Some(PathBuf::from(entry)),
    }
}

/// One problem per `read` folder that is not a folder and per `secrets` item the keychain does not hold, in the order
/// of the entries and their fields. Each names its key, as the problem's key and in its message.
pub fn check_agent_environment(
    agents: &[AgentView],
    board_root: Option<&Path>,
    secret_store: &dyn SecretStore,
) -> Vec<SettingProblem> {
    let mut problems = Vec::new();
    for agent in agents {
        let id = &agent.id;
        for entry in agent.entry.read.iter().flatten() {
            if let Some(folder) = read_folder(entry, board_root)
                && !folder.is_dir()
            {
                problems.push(SettingProblem {
                    key: format!("agents.{id}.read"),
                    layer: SettingLayer::User,
                    message: format!(
                        "`agents.{id}.read`: the folder `{}` does not exist",
                        folder.display()
                    ),
                });
            }
        }
        for (name, item) in agent.entry.secrets.iter().flatten() {
            let key = format!("agents.{id}.secrets.{name}");
            let message = match secret_store.contains(item) {
                Ok(true) => continue,
                Ok(false) => format!(
                    "`{key}`: the keychain has no item `{item}`; set it with `collide config set --secret {key}`"
                ),
                Err(error) => format!("`{key}`: the item `{item}` cannot be looked up: {error}"),
            };
            problems.push(SettingProblem {
                key,
                layer: SettingLayer::User,
                message,
            });
        }
    }
    problems
}
