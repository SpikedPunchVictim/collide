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
//! **A `git:` locator is `<url>//<path>@<ref>`** ([`GitLocator`]): the path and the ref are optional, and without a ref
//! it is `HEAD`. [`PackSource::parse`] refuses a locator that cannot be split or holds something git must never be
//! given, so every file that lists a source is checked by the same rule.
//!
//! The checks are pure: they read the parsed files, not the disk or the network. Fetching, and comparing a pin with
//! the tree a fetch returns, is the host's (`ts-0100-pu`).

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A pack source, split at its scheme. A `git:` locator is checked here and split by [`GitLocator::parse`]; the text
/// after any other scheme is not parsed further.
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
            "git" => {
                if let Err(error) = GitLocator::parse(&rest) {
                    // The source is echoed in the message, so a password in it is masked as the locator's is.
                    let text = match without_password(&rest) {
                        Some(shown) => format!("git:{shown}"),
                        None => text.to_string(),
                    };
                    return Err(SourceError::BadLocator { text, error });
                }
                PackSource::Git { locator: rest }
            }
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
    /// A `git:` source whose locator cannot be used: see [`LocatorError`].
    BadLocator {
        text: String,
        error: LocatorError,
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
            SourceError::BadLocator { text, error } => write!(f, "source `{text}`: {error}"),
        }
    }
}

impl std::error::Error for SourceError {}

/// A `git:` locator, split: `<url>//<path>@<ref>` (0.7, Pack sources). `path` and `ref` are optional.
///
/// - The `//` that follows a scheme (`https://`, `file://`, `ssh://`) is part of the URL. The path begins after the
///   first `//` that comes after it.
/// - An scp-like URL (`git@host:org/repo`) and a path on disk (`/srv/repo.git`, `~/dev/pack`) are URLs too.
/// - With a path, the ref is the text after the last `@` that follows `//`. Without one, it is the text after the last
///   `@` that follows the URL's own host part and has no `/` after it, so the `@` of `git@host` is never a ref. A ref
///   with a `/` in it goes after an empty path: `https://h/o/r//@feature/x`.
/// - Without a ref it is `HEAD`.
///
/// The URL is as written: a `~` is the host's to expand, and nothing here looks at the disk or the network.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitLocator {
    pub url: String,
    /// A folder of the repository, with no `.` or `..` part. Empty for the repository's own root.
    pub path: String,
    pub reference: Option<String>,
}

/// Why a locator was refused. Each variant holds the locator's text, so a message names the form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocatorError {
    /// A NUL or a newline, which a command line and a git protocol line cannot hold.
    Control {
        text: String,
    },
    /// A locator that starts with `-`: git would read it as a flag.
    FlagLike {
        text: String,
    },
    EmptyUrl {
        text: String,
    },
    BadPath {
        text: String,
        reason: &'static str,
    },
    EmptyRef {
        text: String,
    },
    BadRef {
        text: String,
        reason: &'static str,
    },
    /// A URL with a password in its user part. `text` has the password masked: a fetch uses the person's git
    /// credentials, and a locator is committed in the lock and named in messages.
    Password {
        text: String,
    },
}

impl fmt::Display for LocatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LocatorError::Control { text } => {
                write!(f, "locator {text:?} has a NUL or a newline")
            }
            LocatorError::FlagLike { text } => write!(
                f,
                "locator `{text}` starts with `-`, which git would read as a flag"
            ),
            LocatorError::EmptyUrl { text } => write!(
                f,
                "locator `{text}` has an empty URL; the form is `<url>//<path>@<ref>`"
            ),
            LocatorError::BadPath { text, reason } => {
                write!(f, "locator `{text}`: the path {reason}")
            }
            LocatorError::EmptyRef { text } => {
                write!(f, "locator `{text}`: the ref after `@` is empty")
            }
            LocatorError::BadRef { text, reason } => {
                write!(f, "locator `{text}`: the ref {reason}")
            }
            LocatorError::Password { text } => write!(
                f,
                "locator `{text}` has a password in its URL; remove it: a fetch uses your git credentials (a \
                 credential helper or an SSH key)"
            ),
        }
    }
}

impl std::error::Error for LocatorError {}

/// What a URL would contact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UrlTarget {
    /// A host on the network, lower case, with no user or port. A fetch from one that has not been fetched from before
    /// asks first (0.7, Fetch rules).
    Host(String),
    /// A repository on disk: an absolute path, a `~/` path or a `file://` URL. It has no host and never asks. A `~` is
    /// kept as written.
    Path(String),
    /// A remote-helper URL (`ext::...`, `fd::...`). It names no host; git's own protocol allowlist refuses it.
    Helper,
    /// Text that is no URL git would take: a relative path, or a host and path with no scheme.
    Unknown,
}

impl GitLocator {
    /// Splits `text`, the part of a `git:` source after its scheme.
    pub fn parse(text: &str) -> Result<GitLocator, LocatorError> {
        // First, so no other refusal echoes the password.
        if let Some(shown) = without_password(text) {
            return Err(LocatorError::Password { text: shown });
        }
        let own = || text.to_string();
        if text.contains(['\0', '\n']) {
            return Err(LocatorError::Control { text: own() });
        }
        if text.starts_with('-') {
            return Err(LocatorError::FlagLike { text: own() });
        }
        let start = scheme_end(text);
        let (url, path, reference) = match text[start..].find("//") {
            Some(at) => {
                let rest = &text[start + at + 2..];
                match rest.rfind('@') {
                    Some(sign) => (&text[..start + at], &rest[..sign], Some(&rest[sign + 1..])),
                    None => (&text[..start + at], rest, None),
                }
            }
            None => {
                let from = tail_start(text, start);
                match text[from..].rfind('@') {
                    Some(sign) if !text[from + sign..].contains('/') => {
                        (&text[..from + sign], "", Some(&text[from + sign + 1..]))
                    }
                    _ => (text, "", None),
                }
            }
        };
        if url.is_empty() {
            return Err(LocatorError::EmptyUrl { text: own() });
        }
        if let Some(reason) = path_problem(path) {
            return Err(LocatorError::BadPath {
                text: own(),
                reason,
            });
        }
        if let Some(reference) = reference {
            if reference.is_empty() {
                return Err(LocatorError::EmptyRef { text: own() });
            }
            if reference.starts_with('-') {
                return Err(LocatorError::BadRef {
                    text: own(),
                    reason: "starts with `-`, which git would read as a flag",
                });
            }
            if reference
                .chars()
                .any(|c| c.is_control() || c.is_whitespace())
            {
                return Err(LocatorError::BadRef {
                    text: own(),
                    reason: "has a space or a control character",
                });
            }
        }
        Ok(GitLocator {
            url: url.to_string(),
            path: path.to_string(),
            reference: reference.map(str::to_string),
        })
    }

    /// The ref to resolve: the one written, or `HEAD`.
    pub fn reference(&self) -> &str {
        self.reference.as_deref().unwrap_or("HEAD")
    }

    /// What the URL would contact.
    pub fn target(&self) -> UrlTarget {
        url_target(&self.url)
    }
}

/// The index just past `<scheme>://` at the start of `text`, or 0 when there is none.
/// `text` with the password of its URL's user part written as `***`, or `None` when it has none. Only a URL with a
/// scheme has one: an scp-like URL (`git@host:path`) has a user and no password.
fn without_password(text: &str) -> Option<String> {
    let start = scheme_end(text);
    if start == 0 {
        return None;
    }
    let end = text[start..].find('/').map_or(text.len(), |at| start + at);
    let authority = &text[start..end];
    let at = authority.rfind('@')?;
    let colon = authority[..at].find(':')?;
    Some(format!(
        "{}{}:***{}",
        &text[..start],
        &authority[..colon],
        &text[start + at..]
    ))
}

fn scheme_end(text: &str) -> usize {
    let Some(at) = text.find("://") else {
        return 0;
    };
    let mut chars = text[..at].chars();
    let scheme_ok = chars.next().is_some_and(|c| c.is_ascii_alphabetic())
        && chars.all(|c| c.is_ascii_alphanumeric() || "+.-".contains(c));
    if scheme_ok { at + 3 } else { 0 }
}

/// Where a ref can start, in a locator with no path: past a scheme's authority (`user@host`), or past the `:` of an
/// scp-like URL, so the `@` of a user part is never read as a ref.
fn tail_start(text: &str, start: usize) -> usize {
    if start > 0 {
        return text[start..]
            .find('/')
            .map_or(text.len(), |slash| start + slash);
    }
    match (text.find(':'), text.find('/')) {
        (Some(colon), Some(slash)) if colon < slash => colon + 1,
        (Some(colon), None) => colon + 1,
        _ => 0,
    }
}

fn path_problem(path: &str) -> Option<&'static str> {
    if path.is_empty() {
        return None;
    }
    for part in path.split('/') {
        match part {
            "" => return Some("has an empty part (a leading, trailing or doubled `/`)"),
            "." | ".." => return Some("has a `..` or `.` part"),
            _ => {}
        }
    }
    None
}

/// What `url` (a URL as written, not a whole locator) would contact.
pub fn url_target(url: &str) -> UrlTarget {
    if let Some(at) = url.find("::")
        && !url[..at].contains('/')
    {
        return UrlTarget::Helper;
    }
    let start = scheme_end(url);
    if start > 0 {
        let scheme = url[..start - 3].to_ascii_lowercase();
        let rest = &url[start..];
        if scheme == "file" {
            // `file:///path`, or `file://localhost/path`.
            return match rest.find('/') {
                Some(slash) if !rest[slash..].trim_start_matches('/').is_empty() => {
                    UrlTarget::Path(rest[slash..].to_string())
                }
                _ => UrlTarget::Unknown,
            };
        }
        let authority = rest.split('/').next().unwrap_or_default();
        let host = authority.rsplit('@').next().unwrap_or_default();
        return host_of(host).map_or(UrlTarget::Unknown, UrlTarget::Host);
    }
    if url.starts_with('/') || url.starts_with("~/") {
        return UrlTarget::Path(url.to_string());
    }
    match (url.find(':'), url.find('/')) {
        (Some(colon), slash) if slash.is_none_or(|slash| colon < slash) => {
            let host = url[..colon].rsplit('@').next().unwrap_or_default();
            host_of(host).map_or(UrlTarget::Unknown, UrlTarget::Host)
        }
        _ => UrlTarget::Unknown,
    }
}

/// The host in `host[:port]` or `[v6][:port]`, lower case, or `None` when there is none.
fn host_of(text: &str) -> Option<String> {
    let host = match text.strip_prefix('[') {
        Some(inner) => inner.split(']').next().unwrap_or_default(),
        None => match text.rsplit_once(':') {
            Some((host, port)) if !port.is_empty() && port.bytes().all(|b| b.is_ascii_digit()) => {
                host
            }
            _ => text,
        },
    };
    let usable = !host.is_empty() && !host.chars().any(|c| c.is_whitespace() || c.is_control());
    usable.then(|| host.to_ascii_lowercase())
}

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
