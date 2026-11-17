//! Working copies that an integrate changed (ts-0101-bd; 0.7, Working copies; 0.9, Trust, per pack source).
//!
//! A working copy is a folder the person edits and has linked as a `dev:` pack. `trust.yaml` keeps, by the folder's
//! real path, the tree id they last saw (`seen`) and, once an integrate has changed the folder, the tree id the
//! integrate left (`integrated`). A load that finds an `integrated` tree returns a problem that shows the diff from
//! what they saw, and recording the acknowledgement of that tree clears it. An integrate that comes after the diff was
//! shown leaves a later tree, which the earlier acknowledgement does not clear.
//!
//! The objects of both trees are kept in a bare repository of Collide's own (`<cache>/working-copies.git`), because
//! the old version of a folder is in no other repository. A cache that was cleared leaves the diff unavailable, and
//! the problem says so; it still asks.
//!
//! The Queue item that puts this in front of the person is a later stage's, and so is the integrate that calls
//! [`WorkingCopies::note_integrated`]. This module is what both will call.

use crate::git_objects::GitObjects;
use crate::pack_update::tree_changes;
use crate::trust::{Trust, TrustError, TrustStore};
use collide_core::owner::{PackFileChange, PackFileChangeKind};
use std::fs;
use std::path::{Path, PathBuf};

/// A working copy that waits for an acknowledgement.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Waiting {
    /// The tree id the person last saw.
    pub seen: String,
    /// The tree id the integrate left: the one an acknowledgement names.
    pub integrated: String,
    pub changes: Vec<PackFileChange>,
    /// Why the diff is incomplete, when it is.
    pub note: Option<String>,
}

impl Waiting {
    /// The text a load problem holds: the folder, the two trees, each changed file and its changed lines.
    pub fn render(&self, folder: &Path) -> String {
        let short = |id: &str| id.get(..7).unwrap_or(id).to_string();
        let mut text = format!(
            "the working copy {} was changed by an integrate (tree {} to {}) and waits for your acknowledgement:",
            folder.display(),
            short(&self.seen),
            short(&self.integrated)
        );
        for change in &self.changes {
            let kind = match change.kind {
                PackFileChangeKind::Added => "added",
                PackFileChangeKind::Deleted => "deleted",
                PackFileChangeKind::Modified => "modified",
                PackFileChangeKind::TypeChanged => "changed type of",
            };
            text.push_str(&format!("\n  {kind} {}", change.path));
            if let Some(note) = &change.note {
                text.push_str(&format!(" ({note})"));
            }
            for line in &change.lines {
                text.push_str(&format!("\n    {line}"));
            }
        }
        if let Some(note) = &self.note {
            text.push_str(&format!("\n  {note}"));
        }
        text
    }
}

pub struct WorkingCopies {
    store: TrustStore,
    git: GitObjects,
    objects: PathBuf,
}

impl WorkingCopies {
    /// `store` holds `trust.yaml`; `objects` is the bare repository the folders' trees are written to.
    pub fn new(store: TrustStore, git: GitObjects, objects: PathBuf) -> WorkingCopies {
        WorkingCopies {
            store,
            git,
            objects,
        }
    }

    fn key(folder: &Path) -> Result<String, TrustError> {
        let refused = |reason: String| TrustError::DevFolder {
            folder: folder.to_path_buf(),
            reason,
        };
        let real = fs::canonicalize(folder).map_err(|error| refused(error.to_string()))?;
        real.to_str()
            .map(str::to_string)
            .ok_or_else(|| refused("its path is not UTF-8".to_string()))
    }

    /// The person has seen the folder as it is now (they linked it): nothing waits. Returns its tree id.
    pub fn see(&self, folder: &Path) -> Result<String, TrustError> {
        let key = Self::key(folder)?;
        let tree = self.git.tree_of_folder_into(folder, &self.objects)?;
        let mut file = self.store.load()?;
        file.set_working_copy_seen(&key, &tree);
        self.store.save(&file)?;
        Ok(tree)
    }

    /// An integrate changed the folder: its tree now waits for the person. A folder that was never seen is shown as
    /// changed from nothing.
    pub fn note_integrated(&self, folder: &Path) -> Result<(), TrustError> {
        let key = Self::key(folder)?;
        let tree = self.git.tree_of_folder_into(folder, &self.objects)?;
        let mut file = self.store.load()?;
        file.note_integrated(&key, &tree);
        Ok(self.store.save(&file)?)
    }

    /// What waits for the person in `folder`, with the diff, or `None`.
    pub fn waiting(&self, folder: &Path) -> Result<Option<Waiting>, TrustError> {
        let key = Self::key(folder)?;
        let file = self.store.load()?;
        let Some((seen, integrated)) = file.working_copy_waiting(&key) else {
            return Ok(None);
        };
        let trust = Trust::new(self.store.clone()).with_git(self.git.clone());
        let (changes, note) = tree_changes(&trust, &self.objects, "working copy", seen, integrated);
        Ok(Some(Waiting {
            seen: seen.to_string(),
            integrated: integrated.to_string(),
            changes,
            note,
        }))
    }

    /// The person acknowledged the tree `tree`. It clears the wait only when that is the tree the integrate left, and
    /// then the tree is the one seen. `true` when it cleared one.
    pub fn acknowledge(&self, folder: &Path, tree: &str) -> Result<bool, TrustError> {
        let key = Self::key(folder)?;
        let mut file = self.store.load()?;
        if !file.acknowledge_working_copy(&key, tree) {
            return Ok(false);
        }
        self.store.save(&file)?;
        Ok(true)
    }
}
