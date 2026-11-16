//! A fixture git repository in a temp root (ts-0094-av). Every git command a test runs here clears the environment and
//! sets `HOME` and `COLLIDE_DAEMON_ROOT` to the temp root, so none reads the real home or the real git config. The
//! author, the committer and the dates are fixed, so the same files give the same ids in every run.

use super::TempRoot;
use std::fs;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// `git` with a clean environment, run in `dir`. `root` is the temp root that stands in for the home.
pub fn git_command(root: &Path, dir: &Path) -> Command {
    let mut command = Command::new("git");
    command
        .env_clear()
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("HOME", root)
        .env("COLLIDE_DAEMON_ROOT", root)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("LC_ALL", "C")
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_AUTHOR_DATE", "2020-01-01T00:00:00Z")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00Z")
        .current_dir(dir);
    command
}

/// What git printed, as bytes. Panics with git's message when it fails.
pub fn git_bytes(root: &Path, dir: &Path, args: &[&str], stdin: Option<&[u8]>) -> Vec<u8> {
    let mut command = git_command(root, dir);
    command
        .args(args)
        .stdin(if stdin.is_some() {
            Stdio::piped()
        } else {
            Stdio::null()
        })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().expect("git should run");
    if let (Some(bytes), Some(mut pipe)) = (stdin, child.stdin.take()) {
        let bytes = bytes.to_vec();
        // On its own thread, so a long input cannot fill both pipes at once.
        std::thread::spawn(move || {
            let _ = pipe.write_all(&bytes);
        });
    }
    let output = child.wait_with_output().expect("git should end");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    output.stdout
}

/// What git printed, as trimmed text.
pub fn git_text(root: &Path, dir: &Path, args: &[&str]) -> String {
    String::from_utf8_lossy(&git_bytes(root, dir, args, None))
        .trim()
        .to_string()
}

/// A checkout on `main` in `<root>/work`. Its git directory is `<root>/work/.git`.
pub struct GitRepo {
    root: TempRoot,
    work: PathBuf,
}

impl GitRepo {
    pub fn new() -> GitRepo {
        let root = TempRoot::new();
        let work = root.join("work");
        fs::create_dir(&work).unwrap();
        git_text(
            root.path(),
            &work,
            &["init", "--quiet", "--initial-branch=main"],
        );
        GitRepo { root, work }
    }

    /// The temp root, for files a test makes beside the checkout.
    pub fn root(&self) -> &TempRoot {
        &self.root
    }

    pub fn work(&self) -> &Path {
        &self.work
    }

    pub fn git_dir(&self) -> PathBuf {
        self.work.join(".git")
    }

    pub fn git(&self, args: &[&str]) -> String {
        git_text(self.root.path(), &self.work, args)
    }

    pub fn git_with_input(&self, args: &[&str], input: &[u8]) -> String {
        let out = git_bytes(self.root.path(), &self.work, args, Some(input));
        String::from_utf8_lossy(&out).trim().to_string()
    }

    pub fn write(&self, rel: &str, bytes: &[u8]) {
        let path = self.work.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    pub fn write_executable(&self, rel: &str, bytes: &[u8]) {
        self.write(rel, bytes);
        fs::set_permissions(self.work.join(rel), fs::Permissions::from_mode(0o755)).unwrap();
    }

    pub fn symlink(&self, rel: &str, target: &str) {
        let path = self.work.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(target, path).unwrap();
    }

    pub fn remove(&self, rel: &str) {
        fs::remove_file(self.work.join(rel)).unwrap();
    }

    /// Adds every file, commits, and returns the commit id.
    pub fn commit_all(&self, message: &str) -> String {
        self.git(&["add", "--all"]);
        self.git(&["commit", "--quiet", "--allow-empty", "--message", message]);
        self.git(&["rev-parse", "HEAD"])
    }

    /// Writes `bytes` as a blob and returns its id.
    pub fn blob(&self, bytes: &[u8]) -> String {
        self.git_with_input(&["hash-object", "-w", "--stdin"], bytes)
    }

    /// Writes a tree of `(mode, id, name)` entries with names as raw bytes, which no file system need accept, and
    /// returns its id.
    pub fn tree_of(&self, entries: &[(&str, &str, &[u8])]) -> String {
        let mut input = Vec::new();
        for (mode, id, name) in entries {
            input.extend_from_slice(format!("{mode} blob {id}\t").as_bytes());
            input.extend_from_slice(name);
            input.push(0);
        }
        let out = git_bytes(
            self.root.path(),
            &self.work,
            &["mktree", "-z"],
            Some(&input),
        );
        String::from_utf8_lossy(&out).trim().to_string()
    }
}
