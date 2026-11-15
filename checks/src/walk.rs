//! The file walk the checks share: every file under `root` with a wanted extension, skipping what is not source.

use std::io;
use std::path::{Path, PathBuf};

/// Directories that are never source, by name, wherever they are: tool and build output, and `private/` (which no
/// check reads).
const SKIPPED_NAMES: [&str; 6] = [
    ".git",
    "node_modules",
    "target",
    "dist",
    "private",
    ".pnpm-store",
];

/// Directories that are skipped only at these paths below the root: records and prose that name things freely
/// (`.collide/` holds the planning items and spike records, `docs/` the research notes), and staged or generated
/// build output.
const SKIPPED_PATHS: [&str; 4] = [
    ".collide",
    "docs",
    "app/src-tauri/resources",
    "app/src-tauri/gen",
];

/// A file under `root`: its path relative to `root` with `/` separators, and its path on disk.
pub struct Entry {
    pub rel: String,
    pub path: PathBuf,
}

/// The files under `root` whose extension is in `extensions`, in path order. A directory is skipped when its name is
/// in `SKIPPED_NAMES` or in `extra_skipped_names`, or its path below `root` is in `SKIPPED_PATHS`. Symlinks are not
/// followed.
pub fn files(
    root: &Path,
    extensions: &[&str],
    extra_skipped_names: &[&str],
) -> io::Result<Vec<Entry>> {
    let mut found = Vec::new();
    visit(root, "", extensions, extra_skipped_names, &mut found)?;
    Ok(found)
}

fn visit(
    dir: &Path,
    rel_dir: &str,
    extensions: &[&str],
    extra: &[&str],
    found: &mut Vec<Entry>,
) -> io::Result<()> {
    let mut children: Vec<_> = std::fs::read_dir(dir)?.collect::<Result<_, _>>()?;
    children.sort_by_key(|c| c.file_name());
    for child in children {
        let name = child.file_name().to_string_lossy().into_owned();
        let rel = if rel_dir.is_empty() {
            name.clone()
        } else {
            format!("{rel_dir}/{name}")
        };
        let kind = child.file_type()?;
        if kind.is_symlink() {
            continue;
        }
        if kind.is_dir() {
            if SKIPPED_NAMES.contains(&name.as_str())
                || extra.contains(&name.as_str())
                || SKIPPED_PATHS.contains(&rel.as_str())
            {
                continue;
            }
            visit(&child.path(), &rel, extensions, extra, found)?;
        } else if Path::new(&name)
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| extensions.contains(&e))
        {
            found.push(Entry {
                rel,
                path: child.path(),
            });
        }
    }
    Ok(())
}
