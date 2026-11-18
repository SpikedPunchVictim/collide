//! A daemon over a fixture checkout, for the tests of `config.get` and `collide config get` (ts-0098-nx). The daemon
//! runs in the test's own process under the temp root, which is also the home, so `~/.config/collide` is a folder of
//! the test's. The loader's context is built over fixtures: a cache and an SDK stand-in in the root, and the source
//! packs as the built-in bundle, so no test depends on a staged bundle.

use super::git_repo::GitRepo;
use super::{repo_root, test_process_tree};
use collide_core::owner::{BoardOpenParams, BoardOpenResult};
use collide_host::base_env::CaptureOptions;
use collide_host::board_load::LoadContext;
use collide_host::builtin_packs::{BuiltinPacks, Environment};
use collide_host::config_get::LoadContextFactory;
use collide_host::daemon::{Daemon, DaemonOptions, DaemonPaths};
use collide_host::git_fetch::{FetchEnv, GitFetch};
use collide_host::hosts::KnownHosts;
use collide_host::owner_client::OwnerClient;
use collide_host::settings_files::user_config_dir;
use std::collections::BTreeMap;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// A log writer that keeps what it is given, so a test can read what the daemon wrote.
#[derive(Clone, Default)]
pub struct CapturedLog(Arc<Mutex<Vec<u8>>>);

impl CapturedLog {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap()).into_owned()
    }
}

impl Write for CapturedLog {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().unwrap().extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct Fixture {
    pub repo: GitRepo,
    pub daemon: Daemon,
}

fn make_sdk(dir: &Path) {
    fs::create_dir_all(dir.join("dist")).unwrap();
    fs::write(
        dir.join("package.json"),
        "{\"name\":\"@collide/pack-sdk\",\"version\":\"0.1.0\",\"type\":\"module\"}\n",
    )
    .unwrap();
    fs::write(
        dir.join("dist/index.js"),
        "export const name = 'stand-in'\n",
    )
    .unwrap();
}

/// A loader over the fixture's cache and SDK stand-in in `root`, with the source packs as the built-in bundle.
pub fn plain_factory(root: &Path) -> LoadContextFactory {
    let (cache, sdk) = (root.join("cache"), root.join("sdk-copy"));
    Arc::new(move |path| {
        Ok(LoadContext::new(
            cache.clone(),
            sdk.clone(),
            Environment {
                path: path.to_os_string(),
            },
            BuiltinPacks::new(repo_root().join("packs")),
        ))
    })
}

/// The daemon's options over the folders of `root`: its fake login shell, its git config of nothing, and the loader
/// over its own cache and SDK stand-in. `configure` is run on them last.
fn options_for(root: &Path, configure: impl FnOnce(&mut DaemonOptions)) -> DaemonOptions {
    let shell = root.join("fake-shell");
    fs::write(&shell, "#!/bin/sh\neval \"$4\"\n").unwrap();
    fs::set_permissions(&shell, std::os::unix::fs::PermissionsExt::from_mode(0o755)).unwrap();
    let mut options = DaemonOptions::new(
        DaemonPaths::under(root),
        shell.to_str().unwrap().to_string(),
        Box::new(std::io::sink()),
    );
    options.process_tree = test_process_tree();
    options.capture = CaptureOptions {
        timeout: Duration::from_secs(10),
        cap_bytes: 1024 * 1024,
        fallback: BTreeMap::from([("PATH".to_string(), "/usr/bin:/bin".to_string())]),
    };
    options.home = Some(root.to_str().unwrap().to_string());
    options.git_env = vec![
        ("HOME".to_string(), root.to_str().unwrap().to_string()),
        ("GIT_CONFIG_GLOBAL".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_SYSTEM".to_string(), "/dev/null".to_string()),
        ("GIT_CONFIG_NOSYSTEM".to_string(), "1".to_string()),
    ];
    options.load_context = Some(plain_factory(root));
    configure(&mut options);
    options
}

impl Fixture {
    /// A checkout on `main` with one commit, and a daemon that has not been told of it yet.
    pub fn new() -> Fixture {
        Fixture::with_options(|_| {})
    }

    /// As [`Fixture::new`], with `configure` run on the daemon's options before it starts: a test gives it a keychain
    /// of its own, or a log it can read.
    pub fn with_options(configure: impl FnOnce(&mut DaemonOptions)) -> Fixture {
        let repo = GitRepo::new();
        repo.write("README.md", b"fixture\n");
        repo.commit_all("start");
        let root = repo.root().path().to_path_buf();
        make_sdk(&root.join("sdk-copy"));
        let daemon = Daemon::start(options_for(&root, configure)).expect("the daemon starts");
        Fixture { repo, daemon }
    }

    /// A daemon over the folders of `repo`, whose last daemon was stopped by the caller: what a restart is.
    pub fn restart_over(repo: GitRepo, configure: impl FnOnce(&mut DaemonOptions)) -> Fixture {
        let root = repo.root().path().to_path_buf();
        let daemon = Daemon::start(options_for(&root, configure)).expect("the daemon starts again");
        Fixture { repo, daemon }
    }

    /// Stops the daemon and starts another over the same folders, with the store it left: what a restart finds. The
    /// checkout, the store and the user's files are kept. `configure` is run on the new daemon's options.
    pub fn restart(self, configure: impl FnOnce(&mut DaemonOptions)) -> Fixture {
        let Fixture { repo, daemon } = self;
        drop(daemon);
        let root = repo.root().path().to_path_buf();
        let daemon = Daemon::start(options_for(&root, configure)).expect("the daemon starts again");
        Fixture { repo, daemon }
    }

    /// As [`Fixture::new`], with a loader that fetches `git:` packs (ts-0100-pu). `git` is found on `path`, with the
    /// temp root as its home and no system config: a test that must not run real git gives a `path` with a fake one.
    pub fn fetching(path: std::ffi::OsString) -> Fixture {
        Fixture::with_options(|options| {
            let root =
                std::path::PathBuf::from(options.home.clone().expect("the fixture sets a home"));
            let factory: LoadContextFactory = Arc::new(move |tools| {
                let fetch = GitFetch::new(FetchEnv {
                    path: path.clone(),
                    home: root.clone(),
                    ssh_auth_sock: None,
                    xdg_config_home: None,
                })
                .without_system_config();
                Ok(LoadContext::new(
                    root.join("cache"),
                    root.join("sdk-copy"),
                    Environment {
                        path: tools.to_os_string(),
                    },
                    BuiltinPacks::new(repo_root().join("packs")),
                )
                .with_fetch(fetch, KnownHosts::for_home(&root)))
            });
            options.load_context = Some(factory);
        })
    }

    pub fn home(&self) -> &Path {
        self.repo.root().path()
    }

    /// Writes a file of the checkout, to be committed with [`Fixture::commit`].
    pub fn write(&self, rel: &str, text: &str) {
        self.repo.write(rel, text.as_bytes());
    }

    pub fn commit(&self) {
        self.repo.commit_all("change");
    }

    /// Writes `~/.config/collide/<rel>`.
    pub fn write_user(&self, rel: &str, text: &str) {
        let path = user_config_dir(self.home()).join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    pub fn client(&self) -> OwnerClient {
        OwnerClient::connect(self.daemon.paths(), "config-test").expect("the client connects")
    }

    /// Opens the checkout as a board and returns its id (`board-N`).
    pub fn board(&self) -> String {
        let result: BoardOpenResult = self
            .client()
            .call(
                "board.open",
                &BoardOpenParams {
                    root: fs::canonicalize(self.repo.work())
                        .unwrap()
                        .to_str()
                        .unwrap()
                        .to_string(),
                    init_git: false,
                },
                Some(Duration::from_secs(10)),
            )
            .expect("board.open answers");
        result.board.expect("the checkout is a board").id
    }

    /// The pack `ferry`, a repo pack with settings of each kind, listed in `board.yaml`.
    pub fn ferry(&self) {
        self.ferry_with(
            "config:\n\
             \x20 lanePool: { type: integer, minimum: 1, default: 2, overridable: true }\n\
             \x20 greeting: { type: string, default: hello }\n\
             \x20 layout: { type: string, repoOnly: true, default: plain }\n\
             \x20 notifyHook: { type: string, secret: true }\n",
        );
    }

    pub fn ferry_with(&self, config: &str) {
        self.write(
            ".collide/board.yaml",
            "packs:\n  - source: repo:.collide/packs/ferry\n",
        );
        self.write(
            ".collide/packs/ferry/pack.yaml",
            &format!("apiVersion: collide/v0\nname: ferry\nversion: 1.0.0\n{config}"),
        );
    }
}
