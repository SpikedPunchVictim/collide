//! Version ranges for `requires` (0.7): a pack's `requires.sdk` and a tool's `version`.
//!
//! A range is comparators joined by spaces: `>=0.1 <0.2`. A comparator is `>=`, `>`, `<=`, `<` or `=`, then a version
//! of the form `major.minor[.patch]`. A missing patch is 0. Numbers are compared as numbers, so `1.10` is above `1.9`.
//! The `semver` crate is not used: its comparators are joined by commas and it has `^`, `~` and `*`, which a pack never
//! writes. A form this module does not know is refused with its text, never read as "any version".

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl Version {
    /// `major.minor` or `major.minor[.patch]`, digits only. Nothing else is a version.
    pub fn parse(text: &str) -> Option<Version> {
        let parts: Vec<&str> = text.split('.').collect();
        if !(2..=3).contains(&parts.len()) {
            return None;
        }
        let mut numbers = [0u64; 3];
        for (slot, part) in numbers.iter_mut().zip(&parts) {
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return None;
            }
            *slot = part.parse().ok()?;
        }
        Some(Version {
            major: numbers[0],
            minor: numbers[1],
            patch: numbers[2],
        })
    }

    /// The first `major.minor[.patch]` in `text`: what a tool's `--version` printed. A number is read whole, so
    /// `10.5.3.1` is `10.5.3` and never `0.5.3`.
    pub fn find_in(text: &str) -> Option<Version> {
        let bytes = text.as_bytes();
        let mut at = 0;
        while at < bytes.len() {
            if !bytes[at].is_ascii_digit() {
                at += 1;
                continue;
            }
            let start = at;
            let mut dots = 0;
            while at < bytes.len() && dots <= 2 {
                if bytes[at].is_ascii_digit() {
                    at += 1;
                } else if bytes[at] == b'.'
                    && dots < 2
                    && bytes.get(at + 1).is_some_and(u8::is_ascii_digit)
                {
                    dots += 1;
                    at += 1;
                } else {
                    break;
                }
            }
            if dots >= 1
                && let Some(version) = Version::parse(&text[start..at])
            {
                return Some(version);
            }
            // A bare number (`Tool 3 of 4`), or one too big to hold: skip past it and look on.
            while at < bytes.len() && (bytes[at].is_ascii_digit() || bytes[at] == b'.') {
                at += 1;
            }
        }
        None
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RangeError {
    Empty,
    /// A word of the range that is not a comparator and a version.
    UnknownForm {
        form: String,
    },
}

impl fmt::Display for RangeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RangeError::Empty => write!(f, "the version range is empty"),
            RangeError::UnknownForm { form } => write!(
                f,
                "the range form `{form}` is not known; write `>=`, `>`, `<=`, `<` or `=` and then \
                 major.minor[.patch], joined by spaces, as `>=0.1 <0.2`"
            ),
        }
    }
}

impl std::error::Error for RangeError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Operator {
    AtLeast,
    Above,
    AtMost,
    Below,
    Exactly,
}

/// Longest first, so `>=` is not read as `>` and a version that starts with `=`.
const OPERATORS: [(&str, Operator); 5] = [
    (">=", Operator::AtLeast),
    ("<=", Operator::AtMost),
    (">", Operator::Above),
    ("<", Operator::Below),
    ("=", Operator::Exactly),
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionRange {
    comparators: Vec<(Operator, Version)>,
}

impl VersionRange {
    pub fn parse(text: &str) -> Result<VersionRange, RangeError> {
        let mut comparators = Vec::new();
        for word in text.split_whitespace() {
            let unknown = || RangeError::UnknownForm { form: word.into() };
            let (operator, rest) = OPERATORS
                .iter()
                .find_map(|(sign, operator)| word.strip_prefix(sign).map(|rest| (*operator, rest)))
                .ok_or_else(unknown)?;
            let version = Version::parse(rest).ok_or_else(unknown)?;
            comparators.push((operator, version));
        }
        if comparators.is_empty() {
            return Err(RangeError::Empty);
        }
        Ok(VersionRange { comparators })
    }

    pub fn contains(&self, version: &Version) -> bool {
        self.comparators
            .iter()
            .all(|(operator, bound)| match operator {
                Operator::AtLeast => version >= bound,
                Operator::Above => version > bound,
                Operator::AtMost => version <= bound,
                Operator::Below => version < bound,
                Operator::Exactly => version == bound,
            })
    }
}

/// Whether `version` is in `range`: [`VersionRange::parse`], then [`VersionRange::contains`].
pub fn version_in_range(range: &str, version: &Version) -> Result<bool, RangeError> {
    VersionRange::parse(range).map(|range| range.contains(version))
}
