//! `collide` for a run's agent (ts-0140-gk; 0.2, the socket token bullet). Every run's launch puts a folder of its own
//! first on the agent's `PATH`. The folder holds one thing, a link named `collide` to the binary the daemon runs from,
//! so the agent runs the same `collide` as its owner and nothing else in the daemon's own folder reaches the `PATH`.
//! The folder is the run's `bin`, so it goes with the run.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::{Path, PathBuf};

/// The run folder's subfolder that holds the link.
pub const DIR_NAME: &str = "bin";

/// The name the agent types.
pub const LINK_NAME: &str = "collide";

/// Makes `<run_dir>/bin/collide`, a link to `exe`, and returns the folder. A folder or a link that is there from an
/// earlier attempt of the run is replaced, so a resumed phase finds the binary the daemon runs from now.
pub fn link_dir(run_dir: &Path, exe: &Path) -> Result<PathBuf, String> {
    if !exe.is_absolute() {
        return Err(format!(
            "the binary the daemon runs `{LINK_NAME}` from, {}, is not an absolute path, so it cannot be linked",
            exe.display()
        ));
    }
    let dir = run_dir.join(DIR_NAME);
    let link = dir.join(LINK_NAME);
    let failed = |what: &str, error: std::io::Error| format!("{what} {}: {error}", dir.display());
    fs::create_dir_all(&dir).map_err(|error| failed("making", error))?;
    match fs::remove_file(&link) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(failed("replacing the link in", error)),
    }
    symlink(exe, &link).map_err(|error| failed("linking `collide` in", error))?;
    Ok(dir)
}

/// `dir` first, then `path` as the base environment gave it. Without a `path` (or an empty one) it is `dir` alone.
pub fn path_with_first(dir: &Path, path: Option<&str>) -> Result<String, String> {
    let Some(dir) = dir.to_str() else {
        return Err(format!("the folder {} is not valid UTF-8", dir.display()));
    };
    Ok(match path {
        Some(path) if !path.is_empty() => format!("{dir}:{path}"),
        _ => dir.to_string(),
    })
}
