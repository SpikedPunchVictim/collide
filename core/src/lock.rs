//! The pack lock files and the user's pack list (0.7, Pack sources; Repo packs and user packs).
//!
//! - [`PackLock`] is `.collide/packs.lock` and, the same shape, `~/.config/collide/packs.lock`. It pins each
//!   `git:` pack by its source, commit and tree id.
//! - [`UserPacks`] is `~/.config/collide/packs.yaml`: the packs a user added with `collide pack add`.
//!
//! **A source is text with a scheme: `<scheme>:<rest>`.** `repo:`, `git:` and `dev:` are the three of 0.7.
//! Because the schema says only "a lowercase scheme, a colon, something", a package manager's source (`npm:...`,
//! deferred in 0.7: "The lock's `source` field leaves room for them") needs no schema change: a new scheme parses
//! as [`PackSource::Other`], and the code that fetches decides whether it supports it. A lock entry keeps its
//! commit and tree id for every source, because "every source resolves to a git tree id" (0.7).
//!
//! The checks are pure: they read the parsed files, not the disk or the network. Fetching, and comparing a pin with
//! the tree a fetch returns, is Stage 4a's.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A pack source, split at its scheme. The text after the scheme is not parsed further here (the `git:` form,
/// `<url>//<path>@<ref>`, is Stage 4a's).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PackSource {
    /// `repo:<path>`: in the line's snapshot. Never locked.
    Repo { path: String },
    /// `git:<locator>`: any git URL, with an optional path and ref.
    Git { locator: String },
    /// `dev:<path>`: a folder being developed. Per user, never locked.
    Dev { path: String },
    /// A scheme this version does not know, such as a package manager's.
    Other { scheme: String, rest: String },
}

impl PackSource {
    pub fn scheme(&self) -> &str {
        match self {
            PackSource::Repo { .. } => "repo",
            PackSource::Git { .. } => "git",
            PackSource::Dev { .. } => "dev",
            PackSource::Other { scheme, .. } => scheme,
        }
    }

    /// Splits `text` at its scheme. A `git:` locator that starts with `-` is refused, because git would read it as
    /// a flag (0.7, Fetch rules).
    pub fn parse(text: &str) -> Result<PackSource, SourceError> {
        let Some((scheme, rest)) = text.split_once(':') else {
            return Err(SourceError::NoScheme {
                text: text.to_string(),
            });
        };
        let mut chars = scheme.chars();
        let scheme_ok = chars.next().is_some_and(|c| c.is_ascii_lowercase())
            && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || "+.-".contains(c));
        if !scheme_ok {
            return Err(SourceError::NoScheme {
                text: text.to_string(),
            });
        }
        if rest.is_empty() {
            return Err(SourceError::EmptyAfterScheme {
                scheme: scheme.to_string(),
            });
        }
        let rest = rest.to_string();
        Ok(match scheme {
            "repo" => PackSource::Repo { path: rest },
            "git" if rest.starts_with('-') => {
                return Err(SourceError::FlagLikeLocator {
                    text: text.to_string(),
                });
            }
            "git" => PackSource::Git { locator: rest },
            "dev" => PackSource::Dev { path: rest },
            other => PackSource::Other {
                scheme: other.to_string(),
                rest,
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceError {
    /// No lowercase scheme before a colon.
    NoScheme {
        text: String,
    },
    EmptyAfterScheme {
        scheme: String,
    },
    /// A `git:` source whose URL starts with `-`.
    FlagLikeLocator {
        text: String,
    },
}

impl fmt::Display for SourceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SourceError::NoScheme { text } => write!(
                f,
                "source `{text}` has no scheme; write `git:`, `dev:` or `repo:` before it"
            ),
            SourceError::EmptyAfterScheme { scheme } => {
                write!(f, "source `{scheme}:` names nothing after the scheme")
            }
            SourceError::FlagLikeLocator { text } => write!(
                f,
                "source `{text}` starts with `-` after its scheme, which git would read as a flag"
            ),
        }
    }
}

impl std::error::Error for SourceError {}

/// One pinned pack. `source` is the same text `board.yaml` (or `packs.yaml`) lists the pack by.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LockEntry {
    /// `<scheme>:<rest>`, as above. Today a `git:` source.
    #[schemars(regex(pattern = "^[a-z][a-z0-9+.-]*:.+$"))]
    pub source: String,
    /// The commit the source resolved to: 40 hex characters (SHA-1) or 64 (SHA-256).
    pub commit: String,
    /// The git tree id at the pack's path in that commit, as git computes it.
    pub tree: String,
}

/// `.collide/packs.lock`, and the user's `packs.lock`: every pinned pack, one entry per source.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackLock {
    #[serde(default)]
    pub packs: Vec<LockEntry>,
}

/// One pack the user added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UserPackEntry {
    /// `<scheme>:<rest>`: a `git:` or `dev:` source.
    #[schemars(regex(pattern = "^[a-z][a-z0-9+.-]*:.+$"))]
    pub source: String,
}

/// `~/.config/collide/packs.yaml`: the user's packs, in the order they load after the board's.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UserPacks {
    #[serde(default)]
    pub packs: Vec<UserPackEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockError {
    BadSource {
        index: usize,
        error: SourceError,
    },
    /// A `repo:` or `dev:` source in a lock: neither is pinned by an entry (0.7, Pack sources).
    NotPinned {
        index: usize,
        scheme: String,
    },
    /// A commit or tree id that is not 40 or 64 lowercase hex characters. `field` is `commit` or `tree`.
    BadObjectId {
        index: usize,
        field: &'static str,
        value: String,
    },
    /// The same source in two entries. `first` and `second` are their places in `packs:`.
    DuplicateSource {
        source: String,
        first: usize,
        second: usize,
    },
}

impl fmt::Display for LockError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LockError::BadSource { index, error } => write!(f, "entry {index}: {error}"),
            LockError::NotPinned { index, scheme } => write!(
                f,
                "entry {index}: a `{scheme}:` pack has no lock entry; it is not pinned by one"
            ),
            LockError::BadObjectId {
                index,
                field,
                value,
            } => write!(
                f,
                "entry {index}: `{field}: {value}` is not 40 or 64 lowercase hex characters"
            ),
            LockError::DuplicateSource {
                source,
                first,
                second,
            } => write!(f, "entries {first} and {second} both pin `{source}`"),
        }
    }
}

impl std::error::Error for LockError {}

fn is_object_id(text: &str) -> bool {
    matches!(text.len(), 40 | 64)
        && text
            .chars()
            .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
}

/// Checks a lock: every source parses and is one a lock holds, every id is a hex object id, and no source is
/// pinned twice. Returns every refusal, in the order of the entries. An unknown scheme passes (a package
/// manager's source, later).
pub fn check_lock(lock: &PackLock) -> Result<(), Vec<LockError>> {
    let mut errors = Vec::new();
    let mut seen: Vec<(&str, usize)> = Vec::new();
    for (index, entry) in lock.packs.iter().enumerate() {
        match PackSource::parse(&entry.source) {
            Err(error) => errors.push(LockError::BadSource { index, error }),
            Ok(PackSource::Repo { .. } | PackSource::Dev { .. }) => {
                let scheme = entry.source.split(':').next().unwrap_or_default();
                errors.push(LockError::NotPinned {
                    index,
                    scheme: scheme.to_string(),
                });
            }
            Ok(_) => {}
        }
        for (field, value) in [("commit", &entry.commit), ("tree", &entry.tree)] {
            if !is_object_id(value) {
                errors.push(LockError::BadObjectId {
                    index,
                    field,
                    value: value.clone(),
                });
            }
        }
        match seen.iter().find(|(source, _)| *source == entry.source) {
            Some((_, first)) => errors.push(LockError::DuplicateSource {
                source: entry.source.clone(),
                first: *first,
                second: index,
            }),
            None => seen.push((&entry.source, index)),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserPacksError {
    BadSource {
        index: usize,
        error: SourceError,
    },
    /// A `repo:` source: a user's pack is never in a repo's snapshot (0.7, Repo packs and user packs).
    RepoSource {
        index: usize,
    },
    DuplicateSource {
        source: String,
        first: usize,
        second: usize,
    },
}

impl fmt::Display for UserPacksError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            UserPacksError::BadSource { index, error } => write!(f, "entry {index}: {error}"),
            UserPacksError::RepoSource { index } => write!(
                f,
                "entry {index}: a user pack comes from `git:` or `dev:`, not `repo:`"
            ),
            UserPacksError::DuplicateSource {
                source,
                first,
                second,
            } => write!(f, "entries {first} and {second} both list `{source}`"),
        }
    }
}

impl std::error::Error for UserPacksError {}

/// Checks the user's pack list: every source parses, none is `repo:`, and none is listed twice. A `dev:` source
/// is allowed here (and not in a lock), and so is an unknown scheme.
pub fn check_user_packs(user_packs: &UserPacks) -> Result<(), Vec<UserPacksError>> {
    let mut errors = Vec::new();
    let mut seen: Vec<(&str, usize)> = Vec::new();
    for (index, entry) in user_packs.packs.iter().enumerate() {
        match PackSource::parse(&entry.source) {
            Err(error) => errors.push(UserPacksError::BadSource { index, error }),
            Ok(PackSource::Repo { .. }) => errors.push(UserPacksError::RepoSource { index }),
            Ok(_) => {}
        }
        match seen.iter().find(|(source, _)| *source == entry.source) {
            Some((_, first)) => errors.push(UserPacksError::DuplicateSource {
                source: entry.source.clone(),
                first: *first,
                second: index,
            }),
            None => seen.push((&entry.source, index)),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}
