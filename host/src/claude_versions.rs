//! What the daemon knows of the `claude` binary's version and of the newest published one (ts-0068-wx; 0.3, A tested
//! version range and Two version chips).
//!
//! - The version in use is read from the binary a phase would start, cached by [`BinaryKey`], at daemon start and before
//!   each phase ([`ClaudeVersions::gate`]).
//! - A version outside [`TESTED`] gets a one-turn smoke check before its phase. A pass is kept for that binary and marks
//!   the chip; a fail refuses the phase ([`Gate::Refused`]) and is tried again at the next phase, because a failed check
//!   may be an API outage and not the version.
//! - The published versions are the `latest` and `stable` dist-tags of the registry, fetched at start and then once a day by
//!   a thread of its own. A failed fetch keeps the last values and their time. `claude.check_published: false` in
//!   `~/.config/collide/config.yaml` stops the fetch and hides the chip; the file is read at each refresh. With no source
//!   the daemon never fetches: only the daemon's own binary names the real registry ([`registry_source`]).
//! - [`ClaudeVersions::status`] is what `daemon.agent_versions` answers.

use crate::adapters::claude::version::{
    BinaryKey, ClaudeVersion, PUBLISHED_URL, PublishedTags, SmokeOutcome, TESTED, VersionCache,
    VersionError, parse_dist_tags, smoke_check,
};
use crate::http_get::{CurlGet, HttpGet};
use collide_core::owner::{
    AgentVersions, DaemonAgentVersionsResult, PublishedVersions, SmokeCheckResult, TestedVersions,
};
use collide_core::yaml::load_yaml_file;
use serde::Deserialize;
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

/// How long a fetch of the published versions may take (0.3, Two version chips).
pub const FETCH_TIMEOUT: Duration = Duration::from_secs(5);
/// How often the published versions are fetched again.
pub const REFRESH_EVERY: Duration = Duration::from_secs(24 * 60 * 60);
/// The agent the entry is for.
pub const AGENT: &str = "claude";
/// What the published chip says when the daemon was given no source of published versions (a test's or a developer's,
/// under `COLLIDE_DAEMON_ROOT`, which reaches no network): it is not that a look-up failed or has not ended.
pub const NO_SOURCE: &str = "this daemon does not look it up";

/// Where the newest published versions come from.
pub trait PublishedSource: Send + Sync {
    fn fetch(&self) -> Result<PublishedTags, String>;
}

/// The registry's dist-tags at `url`, got through `get`.
pub struct RegistrySource {
    get: Box<dyn HttpGet>,
    url: String,
}

impl RegistrySource {
    pub fn new(get: Box<dyn HttpGet>, url: impl Into<String>) -> RegistrySource {
        RegistrySource {
            get,
            url: url.into(),
        }
    }
}

impl PublishedSource for RegistrySource {
    fn fetch(&self) -> Result<PublishedTags, String> {
        let body = self.get.get(&self.url, FETCH_TIMEOUT)?;
        parse_dist_tags(&body)
    }
}

/// The real registry, through `/usr/bin/curl` and the proxy `captured_env` and the system name.
pub fn registry_source(captured_env: BTreeMap<String, String>) -> Arc<dyn PublishedSource> {
    Arc::new(RegistrySource::new(
        Box::new(CurlGet::system(captured_env)),
        PUBLISHED_URL,
    ))
}

/// Where the daemon gets the published versions from. The default is `Off`: only the daemon's own binary names the real
/// registry (`Registry`), so a test daemon never reaches a network.
#[derive(Clone, Default)]
pub enum PublishedChoice {
    #[default]
    Off,
    /// The npm registry, through curl and the proxy the captured environment and the system name.
    Registry,
    /// A source the caller made (a test's, on loopback).
    Source(Arc<dyn PublishedSource>),
}

impl PublishedChoice {
    /// The source to use once the environment is captured, or `None` for no fetch.
    pub fn source(
        &self,
        captured_env: BTreeMap<String, String>,
    ) -> Option<Arc<dyn PublishedSource>> {
        match self {
            PublishedChoice::Off => None,
            PublishedChoice::Registry => Some(registry_source(captured_env)),
            PublishedChoice::Source(source) => Some(source.clone()),
        }
    }
}

/// What `collide-daemon` uses, from `COLLIDE_DAEMON_ROOT` as it has it: the real registry for a daemon of the user's own, and
/// none for one under an overridden root, which is a test's or a developer's and reaches no network unless the code
/// that starts it gives it a source (the same rule as the launchd job, [`crate::launch::launchctl_from_env`]).
pub fn published_from_env(root: Option<&str>) -> PublishedChoice {
    match root {
        Some(_) => PublishedChoice::Off,
        None => PublishedChoice::Registry,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_daemon_under_an_overridden_root_never_asks_the_registry() {
        assert!(matches!(
            published_from_env(Some("/tmp/x")),
            PublishedChoice::Off
        ));
        assert!(matches!(published_from_env(Some("")), PublishedChoice::Off));
    }

    #[test]
    fn a_daemon_with_no_root_override_asks_the_real_registry() {
        assert!(matches!(
            published_from_env(None),
            PublishedChoice::Registry
        ));
    }

    #[test]
    fn the_default_choice_is_no_fetch() {
        assert!(matches!(PublishedChoice::default(), PublishedChoice::Off));
        assert!(PublishedChoice::Off.source(BTreeMap::new()).is_none());
    }
}

/// What the daemon is given. The published source is given later ([`ClaudeVersions::start_refreshing`]): the real one
/// needs the captured environment for its proxy.
pub struct VersionsOptions {
    pub refresh_every: Duration,
    /// The home whose `.config/collide/config.yaml` may turn the fetch off.
    pub home: Option<String>,
    pub log: Arc<dyn Fn(&str) + Send + Sync>,
}

impl VersionsOptions {
    pub fn new(home: Option<String>, log: Arc<dyn Fn(&str) + Send + Sync>) -> VersionsOptions {
        VersionsOptions {
            refresh_every: REFRESH_EVERY,
            home,
            log,
        }
    }
}

/// What a phase's start is told about the binary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    /// The phase may start: the version is in the range, or its smoke check passed.
    Ready {
        version: ClaudeVersion,
        key: BinaryKey,
    },
    /// The version is outside the range and its smoke check failed. `message` is for the Queue.
    Refused {
        version: ClaudeVersion,
        message: String,
    },
    /// `claude --version` gave no version.
    Unreadable { message: String },
}

#[derive(Default)]
struct State {
    in_use: Option<(ClaudeVersion, BinaryKey)>,
    in_use_error: Option<String>,
    /// The last smoke check, of `version`.
    smoke: Option<(ClaudeVersion, SmokeCheckResult)>,
    /// The binaries whose smoke check passed.
    passed: HashMap<BinaryKey, SmokeCheckResult>,
    published: Published,
    /// Where the published versions come from; `None` until a source is given, and then no fetch is ever made.
    source: Option<Arc<dyn PublishedSource>>,
    stop: bool,
}

#[derive(Default)]
struct Published {
    /// The fetch has been turned off (or never started: with no source there is nothing to show).
    enabled: bool,
    latest: Option<ClaudeVersion>,
    stable: Option<ClaudeVersion>,
    fetched_ms: Option<u64>,
    error: Option<String>,
}

pub struct ClaudeVersions {
    cache: VersionCache,
    state: Mutex<State>,
    wake: Condvar,
    smoke_lock: Mutex<()>,
    options: VersionsOptions,
}

fn locked<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_millis() as u64)
}

#[derive(Deserialize, Default)]
struct UserFile {
    claude: Option<UserClaude>,
}

#[derive(Deserialize, Default)]
struct UserClaude {
    check_published: Option<bool>,
}

/// `claude.check_published` in `$home/.config/collide/config.yaml`; true when the file, the key or the home is missing.
/// A file that cannot be read leaves the fetch on, and says so in the log.
fn check_published(home: Option<&str>, log: &dyn Fn(&str)) -> bool {
    let Some(home) = home.filter(|home| !home.is_empty()) else {
        return true;
    };
    let path = PathBuf::from(home).join(".config/collide/config.yaml");
    match load_yaml_file::<UserFile>(&path) {
        Ok(file) => file
            .claude
            .and_then(|claude| claude.check_published)
            .unwrap_or(true),
        Err(collide_core::yaml::YamlError::Read { source, .. })
            if source.kind() == std::io::ErrorKind::NotFound =>
        {
            true
        }
        Err(error) => {
            log(&format!(
                "config.yaml was not read, so claude.check_published stays on: {error}"
            ));
            true
        }
    }
}

impl ClaudeVersions {
    pub fn new(options: VersionsOptions) -> Arc<ClaudeVersions> {
        Arc::new(ClaudeVersions {
            cache: VersionCache::new(),
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
            smoke_lock: Mutex::new(()),
            options,
        })
    }

    /// Reads the version of `binary` (cached), notes it as the one in use, and decides whether a phase may start on it:
    /// a version in [`TESTED`] may; any other gets a smoke check, unless one passed for this binary.
    pub fn gate(&self, binary: &Path, cwd: &Path, env: &BTreeMap<String, String>) -> Gate {
        let (version, key) = match self.cache.version(binary, env) {
            Ok(read) => read,
            Err(error) => return self.unreadable(error),
        };
        {
            let mut state = locked(&self.state);
            state.in_use = Some((version, key.clone()));
            state.in_use_error = None;
        }
        if TESTED.contains(version) {
            return Gate::Ready { version, key };
        }
        // One smoke check at a time: a second phase waits for the first one's answer and then finds it.
        let _one_at_a_time = locked(&self.smoke_lock);
        if locked(&self.state).passed.contains_key(&key) {
            return Gate::Ready { version, key };
        }
        let outcome = smoke_check(binary, cwd, env);
        let mut state = locked(&self.state);
        match outcome {
            SmokeOutcome::Passed => {
                let result = SmokeCheckResult {
                    passed: true,
                    at_ms: now_ms(),
                    reason: None,
                };
                state.smoke = Some((version, result.clone()));
                state.passed.insert(key.clone(), result);
                Gate::Ready { version, key }
            }
            SmokeOutcome::Failed { reason } => {
                state.smoke = Some((
                    version,
                    SmokeCheckResult {
                        passed: false,
                        at_ms: now_ms(),
                        reason: Some(reason.clone()),
                    },
                ));
                Gate::Refused {
                    version,
                    message: format!(
                        "Claude {version} is outside the tested range {TESTED}, and its smoke check failed: {reason}"
                    ),
                }
            }
        }
    }

    /// Reads the version of `binary` for the status and the fingerprint, without a smoke check.
    pub fn read(
        &self,
        binary: &Path,
        env: &BTreeMap<String, String>,
    ) -> Result<ClaudeVersion, VersionError> {
        match self.cache.version(binary, env) {
            Ok((version, key)) => {
                let mut state = locked(&self.state);
                state.in_use = Some((version, key));
                state.in_use_error = None;
                Ok(version)
            }
            Err(error) => {
                self.unreadable(error.clone());
                Err(error)
            }
        }
    }

    /// Notes that there is no binary to read a version from (not on the `PATH`).
    pub fn note_no_binary(&self, message: String) {
        let mut state = locked(&self.state);
        state.in_use = None;
        state.in_use_error = Some(message);
    }

    fn unreadable(&self, error: VersionError) -> Gate {
        let message = error.to_string();
        let mut state = locked(&self.state);
        state.in_use = None;
        state.in_use_error = Some(message.clone());
        Gate::Unreadable { message }
    }

    /// Gives the source of the published versions. Until the first fetch ends, the chip is "unknown", not hidden: only
    /// the key hides it.
    pub fn use_source(&self, source: Arc<dyn PublishedSource>) {
        let on = check_published(self.options.home.as_deref(), &*self.options.log);
        let mut state = locked(&self.state);
        state.source = Some(source);
        state.published.enabled = on;
    }

    /// Fetches the published versions now, if there is a source and the fetch is on. A failure keeps the last values and
    /// says why.
    pub fn refresh_published(&self) {
        let Some(source) = locked(&self.state).source.clone() else {
            return;
        };
        let on = check_published(self.options.home.as_deref(), &*self.options.log);
        locked(&self.state).published.enabled = on;
        if !on {
            return;
        }
        // The state is not held while the fetch waits on the network.
        let fetched = source.fetch();
        let mut state = locked(&self.state);
        match fetched {
            Ok(tags) => {
                state.published.latest = Some(tags.latest);
                state.published.stable = tags.stable;
                state.published.fetched_ms = Some(now_ms());
                state.published.error = None;
            }
            Err(error) => {
                (self.options.log)(&format!(
                    "the published claude version was not fetched: {error}"
                ));
                state.published.error = Some(error);
            }
        }
    }

    /// Uses `source` and starts the thread that fetches at once and then every `refresh_every`, until
    /// [`ClaudeVersions::stop`].
    pub fn start_refreshing(
        self: &Arc<ClaudeVersions>,
        source: Arc<dyn PublishedSource>,
    ) -> JoinHandle<()> {
        self.use_source(source);
        let versions = Arc::clone(self);
        std::thread::spawn(move || {
            loop {
                if locked(&versions.state).stop {
                    return;
                }
                versions.refresh_published();
                let state = locked(&versions.state);
                let (state, _) = versions
                    .wake
                    .wait_timeout_while(state, versions.options.refresh_every, |state| !state.stop)
                    .unwrap_or_else(PoisonError::into_inner);
                if state.stop {
                    return;
                }
            }
        })
    }

    pub fn stop(&self) {
        locked(&self.state).stop = true;
        self.wake.notify_all();
    }

    /// The answer of `daemon.agent_versions`.
    pub fn status(&self) -> DaemonAgentVersionsResult {
        // With no source the chip says so, unless the setting hides it. The setting's file is read with the state not
        // held.
        let no_source = locked(&self.state).source.is_none();
        let not_looked_up =
            no_source && check_published(self.options.home.as_deref(), &*self.options.log);
        let state = locked(&self.state);
        let in_use = state.in_use.as_ref().map(|(version, _)| *version);
        let smoke = state
            .smoke
            .as_ref()
            .filter(|(version, _)| Some(*version) == in_use)
            .map(|(_, result)| result.clone());
        let published = if not_looked_up {
            Some(PublishedVersions {
                latest: None,
                stable: None,
                fetched_ms: None,
                error: Some(NO_SOURCE.to_string()),
            })
        } else {
            (state.source.is_some() && state.published.enabled).then(|| PublishedVersions {
                latest: state.published.latest.map(|version| version.to_string()),
                stable: state.published.stable.map(|version| version.to_string()),
                fetched_ms: state.published.fetched_ms,
                error: state.published.error.clone(),
            })
        };
        DaemonAgentVersionsResult {
            agents: vec![AgentVersions {
                agent: AGENT.to_string(),
                in_use: in_use.map(|version| version.to_string()),
                in_use_error: state.in_use_error.clone(),
                tested: TestedVersions {
                    from: TESTED.from.to_string(),
                    through: TESTED.through.to_string(),
                },
                in_tested_range: in_use.map(|version| TESTED.contains(version)),
                smoke,
                published,
            }],
        }
    }
}
