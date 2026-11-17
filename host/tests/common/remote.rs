//! A remote for the fetch tests (ts-0100-pu): a bare repository on disk, in the temp root, beside the working
//! repository that is pushed to it. Nothing here reaches a network host. The bare repository is reached by its path or
//! by a `file://` URL.

use super::TempRoot;
use super::git_repo::{GitRepo, git_text};
use std::path::{Path, PathBuf};

pub struct Remote {
    pub work: GitRepo,
    bare: PathBuf,
}

impl Remote {
    pub fn new() -> Remote {
        let work = GitRepo::new();
        let bare = work.root().join("remote.git");
        std::fs::create_dir(&bare).unwrap();
        git_text(
            work.root().path(),
            &bare,
            &["init", "--quiet", "--bare", "--initial-branch=main"],
        );
        Remote { work, bare }
    }

    pub fn root(&self) -> &TempRoot {
        self.work.root()
    }

    /// The bare repository's path.
    pub fn path(&self) -> &Path {
        &self.bare
    }

    pub fn file_url(&self) -> String {
        format!("file://{}", self.bare.display())
    }

    /// Makes the bare repository's refs the working repository's: branches and tags, new, moved and removed.
    pub fn publish(&self) {
        self.work.git(&[
            "push",
            "--quiet",
            "--force",
            "--mirror",
            self.bare.to_str().unwrap(),
        ]);
    }
}
