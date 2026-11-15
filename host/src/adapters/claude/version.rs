//! The tested version range, the smoke check and the settings canary (0.3, A tested version range; Settings that fail
//! validation are ignored silently under `-p`). The daemon reads `claude --version` at start and before each phase,
//! cached by the binary's path and modification time ([`BinaryKey`]). Outside [`TESTED`], it runs a one-turn smoke
//! check and refuses the phase, with the version and the range, when it fails. A pass raises no Queue item: it marks
//! the in-use version chip (0.3, Two version chips). The version goes in the fingerprint (0.13).
//!
//! Settings that fail validation are ignored silently under `-p`, so the snapshot's settings carry a no-op hook, and a
//! phase whose first turn shows no `hook_response` for it fails with [`CANARY_FAILURE`] ([`super::session`]).
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `claude: version check` | [`read_version`], [`parse_version`] | runs `claude --version` and reads its first line |
//! | `claude: smoke check` | [`smoke_check`] | a one-turn prompt that must come back as a `result` of the pinned stream shape |
//! | `claude: settings canary` | [`canary_started`] | whether a stream line is the canary hook's `hook_response` (`hook_started` carries no command) |

use super::launch::FIXED_ARGV;
use super::session::ClaudeSession;
use crate::bounded_run::{run_bounded, tail};
use collide_core::adapter::{AgentSession, StampedEvent};
use collide_core::agent::{AgentEvent, SessionStart, TurnEndReason, UsageScope};
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, SystemTime};

/// A `claude` release, as `claude --version` prints it first: `2.1.325`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClaudeVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl ClaudeVersion {
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for ClaudeVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// The range of releases the adapter's contract test passed on, both ends included.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TestedRange {
    pub from: ClaudeVersion,
    pub through: ClaudeVersion,
}

impl TestedRange {
    pub fn contains(&self, version: ClaudeVersion) -> bool {
        self.from <= version && version <= self.through
    }
}

impl fmt::Display for TestedRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} to {}", self.from, self.through)
    }
}

/// The tested range: it starts at 2.1.323 (S1) and runs through 2.1.325 (S9's addendum). The plan names no upper end;
/// Stage 3b's contract test moves it.
pub const TESTED: TestedRange = TestedRange {
    from: ClaudeVersion::new(2, 1, 323),
    through: ClaudeVersion::new(2, 1, 325),
};

/// What a cached version is keyed by: the binary's path and its modification time, so an upgrade in place is seen.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct BinaryKey {
    pub path: PathBuf,
    pub modified: SystemTime,
}

/// Why a version could not be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VersionError {
    /// The binary could not be run. The message says which and why.
    NotRun { message: String },
    /// The first line of its output holds no `major.minor.patch`.
    Unreadable { output: String },
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VersionError::NotRun { message } => {
                write!(f, "could not run `claude --version`: {message}")
            }
            VersionError::Unreadable { output } => {
                write!(f, "`claude --version` printed no version: {output:?}")
            }
        }
    }
}

impl std::error::Error for VersionError {}

/// Reads a version from the text of `claude --version`: the first token of its first line that is `major.minor.patch`
/// (`2.1.325 (Claude Code)`). A leading `v` is allowed.
pub fn parse_version(output: &str) -> Result<ClaudeVersion, VersionError> {
    let first = output.lines().next().unwrap_or_default();
    first
        .split_whitespace()
        .find_map(version_of_token)
        .ok_or_else(|| VersionError::Unreadable {
            output: output.trim().chars().take(200).collect(),
        })
}

/// `2.1.325` or `v2.1.325`; any other shape (`2.1`, `2.1.3-beta`, `2.1.325.1`) is not a version of this range.
fn version_of_token(token: &str) -> Option<ClaudeVersion> {
    let mut parts = token.strip_prefix('v').unwrap_or(token).split('.');
    let mut number = || parts.next()?.parse::<u32>().ok();
    let version = ClaudeVersion::new(number()?, number()?, number()?);
    parts.next().is_none().then_some(version)
}

/// The longest `claude --version` may take. It starts a runtime and prints one line.
pub const VERSION_TIMEOUT: Duration = Duration::from_secs(15);

/// Runs `binary --version` with exactly `env` and parses it. The caller caches the answer by [`BinaryKey`]
/// ([`VersionCache`]).
pub fn read_version(
    binary: &Path,
    env: &BTreeMap<String, String>,
) -> Result<ClaudeVersion, VersionError> {
    read_version_within(binary, env, VERSION_TIMEOUT)
}

/// [`read_version`] with its own limit, which a test shortens.
pub fn read_version_within(
    binary: &Path,
    env: &BTreeMap<String, String>,
    timeout: Duration,
) -> Result<ClaudeVersion, VersionError> {
    let mut command = Command::new(binary);
    command.arg("--version").env_clear().envs(env);
    let run = run_bounded(command, None, timeout).map_err(|error| VersionError::NotRun {
        message: format!("{}: {error}", binary.display()),
    })?;
    if run.code != Some(0) {
        return Err(VersionError::NotRun {
            message: format!(
                "{} exited with {}: {}",
                binary.display(),
                run.code
                    .map_or("a signal".to_string(), |code| format!("code {code}")),
                tail(&run.stderr)
            ),
        });
    }
    parse_version(&String::from_utf8_lossy(&run.stdout))
}

/// The key of a binary now: its path and its modification time.
pub fn binary_key(binary: &Path) -> std::io::Result<BinaryKey> {
    Ok(BinaryKey {
        path: binary.to_path_buf(),
        modified: std::fs::metadata(binary)?.modified()?,
    })
}

/// The versions read so far, by [`BinaryKey`]: `claude --version` runs once for a binary and again when its
/// modification time changes (an upgrade in place). A failed read is not kept, so the next call tries again.
#[derive(Debug, Default)]
pub struct VersionCache {
    read: Mutex<HashMap<BinaryKey, ClaudeVersion>>,
}

impl VersionCache {
    pub fn new() -> VersionCache {
        VersionCache::default()
    }

    /// The version of `binary` and the key it was cached under.
    pub fn version(
        &self,
        binary: &Path,
        env: &BTreeMap<String, String>,
    ) -> Result<(ClaudeVersion, BinaryKey), VersionError> {
        let key = binary_key(binary).map_err(|error| VersionError::NotRun {
            message: format!("{}: {error}", binary.display()),
        })?;
        if let Some(version) = self.lock().get(&key) {
            return Ok((*version, key));
        }
        let version = read_version(binary, env)?;
        let mut read = self.lock();
        // An older reading of the same path is of a binary that is gone.
        read.retain(|old, _| old.path != key.path);
        read.insert(key.clone(), version);
        Ok((version, key))
    }

    fn lock(&self) -> MutexGuard<'_, HashMap<BinaryKey, ClaudeVersion>> {
        self.read.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// What the one-turn smoke check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmokeOutcome {
    /// The prompt came back as a `result` with the pinned stream shape: the in-use version chip is marked.
    Passed,
    /// It did not. `reason` is what differed: the daemon refuses the phase, naming the version and [`TESTED`].
    Failed { reason: String },
}

/// The words the smoke check adds to a phase's fixed line ([`FIXED_ARGV`], so a flag a release dropped is found here and
/// not in the owner's first phase): the cheapest model, a small budget, no user settings, and no session left on disk.
const SMOKE_EXTRA_ARGV: [&str; 7] = [
    "--model",
    "haiku",
    "--max-budget-usd",
    "0.05",
    "--setting-sources",
    "",
    "--no-session-persistence",
];

/// The turn of the smoke check: it asks for no tool, so nothing in the directory is read or changed.
const SMOKE_PROMPT: &str = "Reply with the single word ok. Do not use any tool.";

/// The longest a smoke check may take. One turn of the cheapest model takes seconds, and the phase that waits on it is inside a `run.start_file` call.
pub const SMOKE_TIMEOUT: Duration = Duration::from_secs(45);

/// Runs one turn of a harmless prompt on `binary` in `cwd`, with exactly `env`, and checks the stream's shape against
/// the pinned one: the session reads the lines as it reads a phase's, and the turn must end with a report, a usage and
/// `end_turn`, and the child with code 0. Anything else is a failure with what differed.
pub fn smoke_check(binary: &Path, cwd: &Path, env: &BTreeMap<String, String>) -> SmokeOutcome {
    smoke_check_within(binary, cwd, env, SMOKE_TIMEOUT)
}

/// [`smoke_check`] with its own limit, which a test shortens.
pub fn smoke_check_within(
    binary: &Path,
    cwd: &Path,
    env: &BTreeMap<String, String>,
    timeout: Duration,
) -> SmokeOutcome {
    let mut session = ClaudeSession::new(SessionStart::AgentAssigned {});
    let turn = session.send_turn(SMOKE_PROMPT);
    let mut command = Command::new(binary);
    command
        .args(FIXED_ARGV)
        .args(SMOKE_EXTRA_ARGV)
        .current_dir(cwd)
        .env_clear()
        .envs(env);
    let input: Vec<u8> = turn.writes.concat();
    let run = match run_bounded(command, Some(&input), timeout) {
        Ok(run) => run,
        Err(error) => {
            return SmokeOutcome::Failed {
                reason: format!("it did not run: {error}"),
            };
        }
    };
    let mut events = Vec::new();
    for line in run.stdout.split(|byte| *byte == b'\n') {
        if !line.is_empty() {
            events.extend(session.feed(line, 0).events);
        }
    }
    match shape_problem(&events, run.code) {
        None => SmokeOutcome::Passed,
        Some(reason) => SmokeOutcome::Failed {
            reason: format!("{reason}{}", stderr_note(&run.stderr)),
        },
    }
}

fn stderr_note(stderr: &[u8]) -> String {
    let text = tail(stderr);
    if text.is_empty() {
        String::new()
    } else {
        format!(" (stderr: {text})")
    }
}

/// What differs from the pinned shape, or `None` when the stream is what a phase expects of one turn.
fn shape_problem(events: &[StampedEvent], code: Option<i32>) -> Option<String> {
    let ended = events.iter().find_map(|stamped| match &stamped.event {
        AgentEvent::TurnEnd { reason, empty, .. } => Some((reason, *empty)),
        _ => None,
    });
    let Some((reason, empty)) = ended else {
        return Some("the stream held no `result` line".to_string());
    };
    if !matches!(reason, TurnEndReason::EndTurn {}) {
        return Some(format!("the turn ended with {reason:?}, not `end_turn`"));
    }
    if empty {
        return Some("the turn gave no output and no usage".to_string());
    }
    if !events
        .iter()
        .any(|stamped| matches!(stamped.event, AgentEvent::FinalReport { .. }))
    {
        return Some("the `result` line held no text".to_string());
    }
    let turn_usage = events.iter().find_map(|stamped| match &stamped.event {
        AgentEvent::Usage { usage } if usage.scope == UsageScope::Turn => Some(usage),
        _ => None,
    });
    if turn_usage.is_none_or(|usage| usage.output.is_none()) {
        return Some("the `result` line gave no output token count".to_string());
    }
    if code != Some(0) {
        return Some(format!(
            "the process ended with {}",
            code.map_or("a signal".to_string(), |code| format!("code {code}"))
        ));
    }
    None
}

/// What the Queue says when the canary never appeared (0.3).
pub const CANARY_FAILURE: &str = "Claude ignored Collide's settings";

/// The text the canary hook writes, so its own `hook_response` is told from the user's hooks. The plan fixes the hook
/// (a no-op) and not its marker; this is the adapter's choice. `hook_started` carries no command (measured, S7's
/// recorded streams: `hook_name` is `SessionStart:startup` and `hook_event` `SessionStart`, so a user's own
/// `SessionStart` hook starts the same way), and a hook's `hook_response` carries its `stdout`, `stderr` and `output`.
/// So the hook writes its marker to stderr, which is not added to the model's context, and the canary is the
/// `hook_response` that holds it.
pub const CANARY_MARKER: &str = "collide-settings-canary";

/// Whether one line of the child's stdout is the canary hook's `hook_response`: a `SessionStart` hook that succeeded and
/// wrote [`CANARY_MARKER`]. A phase that has ended its first turn without one fails with [`CANARY_FAILURE`].
pub fn canary_started(line: &[u8]) -> bool {
    let Ok(value) = serde_json::from_slice::<Value>(line) else {
        return false;
    };
    canary_in(&value)
}

/// [`canary_started`] over a line already read as JSON.
pub(super) fn canary_in(value: &Value) -> bool {
    let text = |key: &str| value.get(key).and_then(Value::as_str);
    value.get("type").and_then(Value::as_str) == Some("system")
        && text("subtype") == Some("hook_response")
        && text("hook_event") == Some("SessionStart")
        && text("outcome") == Some("success")
        && ["stderr", "stdout", "output"]
            .into_iter()
            .filter_map(text)
            .any(|written| written.contains(CANARY_MARKER))
}

/// The package whose newest versions the second chip shows (0.3, Two version chips).
pub const PUBLISHED_PACKAGE: &str = "@anthropic-ai/claude-code";

/// The npm registry's dist-tags of [`PUBLISHED_PACKAGE`]: a small object of tag to version, with no user data in the
/// request. The scope's slash is escaped, as npm's own client writes it.
pub const PUBLISHED_URL: &str =
    "https://registry.npmjs.org/-/package/@anthropic-ai%2fclaude-code/dist-tags";

/// The two dist-tags the chip shows: `latest` is the chip, `stable` is in its tooltip.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublishedTags {
    pub latest: ClaudeVersion,
    pub stable: Option<ClaudeVersion>,
}

/// Reads the registry's dist-tags body. `latest` must be there and a version; `stable` is kept when it is one. Any
/// other tag is ignored.
pub fn parse_dist_tags(body: &[u8]) -> Result<PublishedTags, String> {
    let tags: Value = serde_json::from_slice(body)
        .map_err(|error| format!("the registry's answer is not JSON: {error}"))?;
    let tag = |name: &str| {
        tags.get(name)
            .and_then(Value::as_str)
            .and_then(version_of_token)
    };
    let latest = tag("latest")
        .ok_or_else(|| "the registry's answer has no `latest` tag that is a version".to_string())?;
    Ok(PublishedTags {
        latest,
        stable: tag("stable"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_range_starts_at_the_s1_release_and_includes_both_ends() {
        assert!(TESTED.contains(ClaudeVersion::new(2, 1, 323)));
        assert!(TESTED.contains(ClaudeVersion::new(2, 1, 324)));
        assert!(TESTED.contains(ClaudeVersion::new(2, 1, 325)));
    }

    #[test]
    fn a_release_before_or_after_the_range_is_outside_it() {
        assert!(!TESTED.contains(ClaudeVersion::new(2, 1, 322)));
        assert!(!TESTED.contains(ClaudeVersion::new(2, 1, 326)));
        assert!(!TESTED.contains(ClaudeVersion::new(2, 2, 0)));
        assert!(!TESTED.contains(ClaudeVersion::new(3, 0, 0)));
    }

    #[test]
    fn a_version_and_the_range_print_as_the_queue_message_names_them() {
        assert_eq!(ClaudeVersion::new(2, 1, 325).to_string(), "2.1.325");
        assert_eq!(TESTED.to_string(), "2.1.323 to 2.1.325");
    }

    #[test]
    fn the_first_line_of_the_output_holds_the_version() {
        assert_eq!(
            parse_version(
                "2.1.325 (Claude Code)
"
            ),
            Ok(ClaudeVersion::new(2, 1, 325))
        );
        assert_eq!(parse_version("2.1.325"), Ok(ClaudeVersion::new(2, 1, 325)));
        assert_eq!(
            parse_version(
                "Claude Code v2.2.0
second line 9.9.9
"
            ),
            Ok(ClaudeVersion::new(2, 2, 0))
        );
    }

    #[test]
    fn output_with_no_three_part_version_on_its_first_line_is_unreadable() {
        for output in [
            "",
            "
2.1.325
",
            "claude",
            "2.1",
            "2.1.x",
            "2.1.325.1",
            "2.1.3-beta",
            "-1.2.3",
        ] {
            assert!(
                matches!(parse_version(output), Err(VersionError::Unreadable { .. })),
                "{output:?}"
            );
        }
    }

    #[test]
    fn a_long_unreadable_output_is_cut_in_the_error() {
        let Err(VersionError::Unreadable { output }) = parse_version(&"x".repeat(5000)) else {
            panic!("not a version")
        };
        assert_eq!(output.len(), 200);
    }

    #[test]
    fn the_dist_tags_give_latest_and_stable_and_ignore_the_rest() {
        let body = br#"{"latest":"2.1.325","stable":"2.1.311","next":"2.2.0-rc.1"}"#;
        assert_eq!(
            parse_dist_tags(body),
            Ok(PublishedTags {
                latest: ClaudeVersion::new(2, 1, 325),
                stable: Some(ClaudeVersion::new(2, 1, 311)),
            })
        );
        let only_latest = parse_dist_tags(br#"{"latest":"2.1.325"}"#).unwrap();
        assert_eq!(only_latest.stable, None);
    }

    #[test]
    fn dist_tags_with_no_usable_latest_are_an_error_that_says_so() {
        for body in [
            &b"not json"[..],
            br#"{"stable":"2.1.311"}"#,
            br#"{"latest":"soon"}"#,
            br#"{"latest":5}"#,
            br#"[]"#,
        ] {
            assert!(parse_dist_tags(body).is_err(), "{body:?}");
        }
    }

    fn hook_response(extra: Value) -> Vec<u8> {
        let mut line = serde_json::json!({
            "type": "system", "subtype": "hook_response", "hook_event": "SessionStart",
            "hook_name": "SessionStart:startup", "outcome": "success", "exit_code": 0,
            "stdout": "", "stderr": "", "output": ""
        });
        for (key, value) in extra.as_object().unwrap() {
            line[key] = value.clone();
        }
        serde_json::to_vec(&line).unwrap()
    }

    #[test]
    fn the_canary_is_the_session_start_response_that_wrote_the_marker() {
        let marker = format!(
            "{CANARY_MARKER}
"
        );
        assert!(canary_started(&hook_response(
            serde_json::json!({ "stderr": marker, "output": marker })
        )));
        assert!(canary_started(&hook_response(
            serde_json::json!({ "output": marker })
        )));
    }

    #[test]
    fn a_users_own_session_start_hook_is_not_the_canary() {
        assert!(!canary_started(&hook_response(
            serde_json::json!({ "stdout": "CHISLE ACTIVE", "output": "CHISLE ACTIVE" })
        )));
    }

    #[test]
    fn the_started_line_carries_no_command_so_it_is_not_the_canary() {
        let started = serde_json::json!({
            "type": "system", "subtype": "hook_started", "hook_event": "SessionStart",
            "hook_name": "SessionStart:startup", "hook_id": "h1"
        });
        assert!(!canary_started(&serde_json::to_vec(&started).unwrap()));
    }

    #[test]
    fn a_marker_from_another_event_or_a_failed_hook_or_a_line_that_is_not_json_is_not_the_canary() {
        let marker = serde_json::json!({ "stderr": CANARY_MARKER });
        let mut other_event = marker.clone();
        other_event["hook_event"] = "PreToolUse".into();
        let mut failed = marker.clone();
        failed["outcome"] = "error".into();
        assert!(!canary_started(&hook_response(other_event)));
        assert!(!canary_started(&hook_response(failed)));
        assert!(!canary_started(b"{"));
        assert!(!canary_started(b""));
        assert!(!canary_started(
            br#"{"type":"assistant","message":{"content":[{"type":"text","text":"collide-settings-canary"}]}}"#
        ));
    }

    #[test]
    fn the_published_address_is_the_registry_and_names_the_package() {
        assert!(PUBLISHED_URL.starts_with("https://registry.npmjs.org/"));
        assert!(PUBLISHED_URL.contains("@anthropic-ai%2fclaude-code"));
        assert!(!PUBLISHED_URL.contains('?'));
    }
}
