//! The tested version range, the smoke check and the settings canary (0.3, A tested version range; Settings that fail
//! validation are ignored silently under `-p`). The daemon reads `claude --version` at start and before each phase,
//! cached by the binary's path and modification time ([`BinaryKey`]). Outside [`TESTED`], it runs a one-turn smoke
//! check, then warns in the Queue when the check passes and refuses the phase, with the version and the range, when it
//! fails. The version goes in the fingerprint (0.13).
//!
//! Settings that fail validation are ignored silently under `-p`, so the snapshot's settings carry a no-op hook, and a
//! phase whose first turn shows no `hook_started` event for it fails with [`CANARY_FAILURE`].
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `claude: version check` | [`read_version`], [`parse_version`] | runs `claude --version` and reads its first line |
//! | `claude: smoke check` | [`smoke_check`] | a one-turn prompt that must come back as a `result` of the pinned stream shape |
//! | `claude: settings canary` | [`canary_started`] | whether a stream line is the canary hook's `hook_started` |

use std::fmt;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

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

/// Reads a version from the text of `claude --version`.
pub fn parse_version(output: &str) -> Result<ClaudeVersion, VersionError> {
    let _ = output;
    todo!("claude: version check")
}

/// Runs `binary --version` and parses it. The caller caches the answer by [`BinaryKey`].
pub fn read_version(binary: &Path) -> Result<ClaudeVersion, VersionError> {
    let _ = binary;
    todo!("claude: version check")
}

/// What the one-turn smoke check found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SmokeOutcome {
    /// The prompt came back as a `result` with the pinned stream shape: the daemon warns in the Queue.
    Passed,
    /// It did not. `reason` is what differed: the daemon refuses the phase, naming the version and [`TESTED`].
    Failed { reason: String },
}

/// Runs one turn of a harmless prompt on `binary` in `cwd` and checks the stream's shape against the pinned one.
pub fn smoke_check(binary: &Path, cwd: &Path) -> SmokeOutcome {
    let _ = (binary, cwd);
    todo!("claude: smoke check")
}

/// What the Queue says when the canary never appeared (0.3).
pub const CANARY_FAILURE: &str = "Claude ignored Collide's settings";

/// The text the canary hook's command carries, so its `hook_started` event is told from the user's own hooks. The
/// plan fixes the hook (a no-op) and not its marker; this is the adapter's choice.
pub const CANARY_MARKER: &str = "collide-settings-canary";

/// Whether one line of the child's stdout is the `hook_started` event of the canary hook. A phase that has read its
/// whole first turn without one fails with [`CANARY_FAILURE`].
pub fn canary_started(line: &[u8]) -> bool {
    let _ = line;
    todo!("claude: settings canary")
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
    #[should_panic(expected = "claude: version check")]
    fn parsing_a_version_is_a_stub_tagged_for_stage_3b() {
        let _ = parse_version("2.1.325 (Claude Code)");
    }

    #[test]
    #[should_panic(expected = "claude: smoke check")]
    fn the_smoke_check_is_a_stub_tagged_for_stage_3b() {
        let _ = smoke_check(Path::new("/x/claude"), Path::new("/x"));
    }

    #[test]
    #[should_panic(expected = "claude: settings canary")]
    fn the_canary_check_is_a_stub_tagged_for_stage_3b() {
        let _ = canary_started(b"{}");
    }
}
