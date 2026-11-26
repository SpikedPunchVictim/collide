//! The pack mirror and the fetch rules (ts-0100-pu; 0.7, Pack sources and Fetch rules).
//!
//! A `git:` pack is fetched into a bare mirror, `<cache>/git/<name>/`, one per URL ([`mirror_path`]). Every branch and
//! tag is fetched, so a ref the lock or a later `pack update` names is in it, and nothing is pruned, so a commit a
//! branch has left stays until a person clears the cache. A load reads the lock and never the remote: the mirror is
//! read only when the cache has no entry for the locked tree.
//!
//! Every fetch goes through [`GitFetch::fetch`], and the rules are in it and nowhere else:
//!
//! - A URL with a host that [`KnownHosts`] does not list is not fetched: the result is [`FetchError::NeedsApproval`]
//!   with the host, and nothing runs. A path on disk has no host and never asks.
//! - The URL is refused when it is empty, starts with `-`, or holds a NUL or a newline; it follows a `--`.
//! - Git runs with a cleared environment: only `PATH`, `HOME`, `SSH_AUTH_SOCK` and `XDG_CONFIG_HOME` (all four
//!   from [`FetchEnv`]), `GIT_TERMINAL_PROMPT=0` (a fetch that would ask fails), and
//!   `GIT_ALLOW_PROTOCOL=https:ssh:file` (no `ext::` or other helper runs, whatever the user's git config allows).
//!   `GIT_SSH_COMMAND`, `GIT_ASKPASS` and `GIT_EXEC_PATH` are never passed.
//! - It ends after [`FETCH_TIMEOUT`], and the error names the URL and the time.
//!
//! The fetch also takes the remote's `HEAD` into `refs/collide/head`: a bare mirror's own `HEAD` is not the remote's,
//! and a locator with no ref means the remote's.

use crate::bounded_run::{run_bounded, tail};
use crate::hosts::{HostsError, KnownHosts};
use collide_core::lock::{UrlTarget, url_target};
use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

/// How long one fetch may run.
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(120);

/// The ref the remote's `HEAD` is fetched into.
pub const HEAD_REF: &str = "refs/collide/head";

const REFSPECS: [&str; 3] = [
    "+refs/heads/*:refs/heads/*",
    "+refs/tags/*:refs/tags/*",
    "+HEAD:refs/collide/head",
];

/// FNV-1a, 32 bits, which has no seed: the same URL gives the same name in every build and on every machine. A view's
/// file name made from a name that leaves nothing uses it too (`crate::views::view_file_stem`).
pub(crate) fn fnv1a(text: &str) -> u32 {
    text.bytes().fold(0x811c_9dc5, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    })
}

/// The mirror's folder name for `url`: the URL made safe for a path (letters, digits, `.` and `-` kept, the rest `_`,
/// at most 100 of them), a `-`, and eight hex digits of a hash of the URL as written. Two URLs that make the same safe
/// text still differ by the hash.
pub fn mirror_name(url: &str) -> String {
    let safe: String = url
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' {
                c
            } else {
                '_'
            }
        })
        .take(100)
        .collect();
    let safe = match safe.strip_prefix('.') {
        Some(rest) => format!("_{rest}"),
        None => safe,
    };
    format!("{safe}-{:08x}", fnv1a(url))
}

/// `<cache>/git/<name>`: the mirror of `url`.
pub fn mirror_path(cache: &Path, url: &str) -> PathBuf {
    cache.join("git").join(mirror_name(url))
}

/// The only parts of the environment a fetch passes on.
#[derive(Debug, Clone)]
pub struct FetchEnv {
    /// Where `git` is found, and what git itself finds `ssh` on.
    pub path: OsString,
    /// Where git and ssh find their config, keys and known hosts.
    pub home: PathBuf,
    pub ssh_auth_sock: Option<OsString>,
    pub xdg_config_home: Option<OsString>,
}

impl FetchEnv {
    /// The four values from this process: for the daemon. A test builds its own.
    pub fn from_process() -> FetchEnv {
        FetchEnv {
            path: std::env::var_os("PATH").unwrap_or_else(|| "/usr/bin:/bin".into()),
            home: std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| "/".into()),
            ssh_auth_sock: std::env::var_os("SSH_AUTH_SOCK"),
            xdg_config_home: std::env::var_os("XDG_CONFIG_HOME"),
        }
    }
}

/// Why a fetch did not happen or did not finish.
#[derive(Debug)]
pub enum FetchError {
    /// The URL's host has not been fetched from before: nothing ran.
    NeedsApproval {
        host: String,
    },
    BadUrl {
        url: String,
        why: String,
    },
    Timeout {
        url: String,
        after: Duration,
    },
    /// Git ran and ended with a failure; `detail` is the end of what it wrote.
    Failed {
        url: String,
        code: Option<i32>,
        detail: String,
    },
    /// Git could not be run, or the mirror's folder could not be made.
    Io {
        what: String,
    },
    Hosts(HostsError),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FetchError::NeedsApproval { host } => {
                write!(
                    f,
                    "the first fetch from {host} needs a yes, and has not been given one"
                )
            }
            FetchError::BadUrl { url, why } => {
                write!(f, "the URL `{url}` cannot be fetched: {why}")
            }
            FetchError::Timeout { url, after } => write!(
                f,
                "fetching {url} did not end within {} s, and was stopped",
                after.as_secs_f32()
            ),
            FetchError::Failed { url, code, detail } => {
                let code = code.map_or("a signal".to_string(), |code| format!("exit {code}"));
                write!(f, "fetching {url} failed ({code}): {detail}")
            }
            FetchError::Io { what } => write!(f, "{what}"),
            FetchError::Hosts(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for FetchError {}

/// Fetches into mirrors under the rules of the module's docs.
#[derive(Debug, Clone)]
pub struct GitFetch {
    env: FetchEnv,
    timeout: Duration,
    system_config: bool,
}

impl GitFetch {
    pub fn new(env: FetchEnv) -> GitFetch {
        GitFetch {
            env,
            timeout: FETCH_TIMEOUT,
            system_config: true,
        }
    }

    pub fn with_timeout(mut self, timeout: Duration) -> GitFetch {
        self.timeout = timeout;
        self
    }

    /// Tells git to read no system-wide config: the seam a test uses so the machine's own config (a credential helper)
    /// is not part of it.
    pub fn without_system_config(mut self) -> GitFetch {
        self.system_config = false;
        self
    }

    /// Fetches every branch and tag, and the remote's `HEAD`, of `url` into the bare repository `mirror`, making it
    /// when it is not there.
    pub fn fetch(&self, mirror: &Path, url: &str, hosts: &KnownHosts) -> Result<(), FetchError> {
        let bad = |why: &str| FetchError::BadUrl {
            url: url.to_string(),
            why: why.to_string(),
        };
        if url.is_empty() {
            return Err(bad("it is empty"));
        }
        if url.starts_with('-') {
            return Err(bad("it starts with `-`, which git would read as a flag"));
        }
        if url.contains(['\0', '\n']) {
            return Err(bad("it has a NUL or a newline"));
        }
        let url = match url_target(url) {
            UrlTarget::Host(host) => {
                if !hosts.is_known(&host).map_err(FetchError::Hosts)? {
                    return Err(FetchError::NeedsApproval { host });
                }
                url.to_string()
            }
            UrlTarget::Path(path) if path.starts_with("~/") => {
                self.env.home.join(&path[2..]).display().to_string()
            }
            UrlTarget::Path(_) | UrlTarget::Helper => url.to_string(),
            UrlTarget::Unknown => {
                return Err(bad(
                    "it is not a URL git can fetch from; write `https://...`, `ssh://...`, `git@host:path`, an \
                     absolute path or `file://...`",
                ));
            }
        };
        self.ensure_mirror(mirror)?;
        let mut command = self.command(Some(mirror));
        command
            .args(["-c", "gc.auto=0", "-c", "maintenance.auto=false", "fetch"])
            .args([
                "--no-tags",
                "--force",
                "--quiet",
                "--no-write-fetch-head",
                "--no-recurse-submodules",
                "--",
            ])
            .arg(&url)
            .args(REFSPECS)
            .current_dir(mirror);
        self.run(command, &url)
    }

    /// A `git` with the fetch's environment and nothing else. `mirror` is the repository it works in.
    fn command(&self, mirror: Option<&Path>) -> Command {
        let mut command = Command::new(self.git());
        command
            .env_clear()
            .env("PATH", &self.env.path)
            .env("HOME", &self.env.home)
            .env("GIT_TERMINAL_PROMPT", "0")
            .env("GIT_ALLOW_PROTOCOL", "https:ssh:file")
            .env("LC_ALL", "C");
        if let Some(sock) = &self.env.ssh_auth_sock {
            command.env("SSH_AUTH_SOCK", sock);
        }
        if let Some(xdg) = &self.env.xdg_config_home {
            command.env("XDG_CONFIG_HOME", xdg);
        }
        if !self.system_config {
            command.env("GIT_CONFIG_NOSYSTEM", "1");
        }
        if let Some(mirror) = mirror {
            command.env("GIT_DIR", mirror);
        }
        command
    }

    /// `git`, found on the fetch's `PATH` and not the process's.
    fn git(&self) -> PathBuf {
        std::env::split_paths(&self.env.path)
            .map(|dir| dir.join("git"))
            .find(|candidate| candidate.is_file())
            .unwrap_or_else(|| PathBuf::from("git-not-found-on-the-fetch-path"))
    }

    fn ensure_mirror(&self, mirror: &Path) -> Result<(), FetchError> {
        if mirror.exists() {
            return Ok(());
        }
        let io = |what: String| FetchError::Io { what };
        if let Some(parent) = mirror.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                io(format!(
                    "the mirror's folder {} cannot be made: {e}",
                    parent.display()
                ))
            })?;
        }
        let mut command = self.command(None);
        command
            .args(["init", "--bare", "--quiet", "--"])
            .arg(mirror)
            .current_dir(mirror.parent().unwrap_or(Path::new("/")));
        match run_bounded(command, None, self.timeout) {
            Ok(done) if done.code == Some(0) => Ok(()),
            Ok(done) => Err(io(format!(
                "the mirror {} cannot be made: {}",
                mirror.display(),
                tail(&done.stderr)
            ))),
            Err(error) => Err(io(format!(
                "git cannot be run to make {}: {error}",
                mirror.display()
            ))),
        }
    }

    fn run(&self, command: Command, url: &str) -> Result<(), FetchError> {
        match run_bounded(command, None, self.timeout) {
            Ok(done) if done.code == Some(0) => Ok(()),
            Ok(done) => Err(FetchError::Failed {
                url: url.to_string(),
                code: done.code,
                detail: tail(&done.stderr),
            }),
            // `run_bounded` says a timeout in words and a spawn failure in the system's.
            Err(error) if error.starts_with("it did not end within") => Err(FetchError::Timeout {
                url: url.to_string(),
                after: self.timeout,
            }),
            Err(error) => Err(FetchError::Io {
                what: format!("git cannot be run to fetch {url}: {error}"),
            }),
        }
    }
}
