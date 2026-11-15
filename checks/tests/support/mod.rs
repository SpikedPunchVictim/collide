//! A scratch tree under cargo's per-target temp directory, never under the repository's own sources. Seeded strings
//! live only here, and each tree is removed when its owner drops.

#![allow(dead_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub struct Tree {
    root: PathBuf,
}

impl Tree {
    pub fn new() -> Tree {
        let n = NEXT.fetch_add(1, Ordering::SeqCst);
        let root = Path::new(env!("CARGO_TARGET_TMPDIR"))
            .join(format!("checks-{}-{n}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Tree { root }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn write(&self, rel: &str, text: &str) -> &Tree {
        let path = self.root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
        self
    }
}

impl Drop for Tree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

/// The reference pack's name, joined at run time so that this file is clean under the no-names check.
pub fn reference_pack() -> String {
    ["orch", "ard"].concat()
}
