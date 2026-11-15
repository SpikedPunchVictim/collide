//! A length of time as a pack writes it: a whole number and a unit, `30s`, `120m`, `2h`, `1d` (0.7: `interval:`,
//! `timeout:`). Seconds are the smallest unit. A bare number is refused, so that `timeout: 120` never has to
//! guess between seconds and minutes.

use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::de::{self, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::borrow::Cow;
use std::fmt;

/// A whole number of seconds, written in the largest of `d`, `h`, `m` and `s` that divides it exactly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Duration {
    secs: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DurationError {
    Empty,
    NoNumber { text: String },
    NoUnit { text: String },
    UnknownUnit { text: String, unit: String },
    TooLarge { text: String },
}

impl fmt::Display for DurationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DurationError::Empty => write!(f, "a duration is empty; write it as `30s` or `5m`"),
            DurationError::NoNumber { text } => {
                write!(f, "duration `{text}` does not start with a whole number")
            }
            DurationError::NoUnit { text } => {
                write!(
                    f,
                    "duration `{text}` has no unit; write `{text}s`, `{text}m` or `{text}h`"
                )
            }
            DurationError::UnknownUnit { text, unit } => {
                write!(
                    f,
                    "duration `{text}` has the unit `{unit}`; the units are s, m, h and d"
                )
            }
            DurationError::TooLarge { text } => write!(f, "duration `{text}` is too large"),
        }
    }
}

impl std::error::Error for DurationError {}

impl Duration {
    pub const fn from_secs(secs: u64) -> Duration {
        Duration { secs }
    }

    pub const fn as_secs(self) -> u64 {
        self.secs
    }

    /// Reads `30s`, `120m`, `2h` or `1d`.
    pub fn parse(text: &str) -> Result<Duration, DurationError> {
        if text.is_empty() {
            return Err(DurationError::Empty);
        }
        let digits = text.bytes().take_while(u8::is_ascii_digit).count();
        if digits == 0 {
            return Err(DurationError::NoNumber {
                text: text.to_string(),
            });
        }
        let (number, unit) = text.split_at(digits);
        let per_unit: u64 = match unit {
            "" => {
                return Err(DurationError::NoUnit {
                    text: text.to_string(),
                });
            }
            "s" => 1,
            "m" => 60,
            "h" => 3600,
            "d" => 86_400,
            _ => {
                return Err(DurationError::UnknownUnit {
                    text: text.to_string(),
                    unit: unit.to_string(),
                });
            }
        };
        let secs = number
            .parse::<u64>()
            .ok()
            .and_then(|number| number.checked_mul(per_unit))
            .ok_or_else(|| DurationError::TooLarge {
                text: text.to_string(),
            })?;
        Ok(Duration { secs })
    }
}

impl fmt::Display for Duration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let secs = self.secs;
        for (unit, per_unit) in [("d", 86_400), ("h", 3600), ("m", 60)] {
            if secs != 0 && secs.is_multiple_of(per_unit) {
                return write!(f, "{}{unit}", secs / per_unit);
            }
        }
        write!(f, "{secs}s")
    }
}

impl Serialize for Duration {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

struct DurationVisitor;

impl Visitor<'_> for DurationVisitor {
    type Value = Duration;

    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a duration such as `30s` or `120m`")
    }

    fn visit_str<E: de::Error>(self, text: &str) -> Result<Duration, E> {
        Duration::parse(text).map_err(E::custom)
    }
}

impl<'de> Deserialize<'de> for Duration {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        deserializer.deserialize_str(DurationVisitor)
    }
}

impl JsonSchema for Duration {
    fn schema_name() -> Cow<'static, str> {
        Cow::Borrowed("Duration")
    }

    fn json_schema(_generator: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "pattern": "^[0-9]+(s|m|h|d)$",
            "description": "A whole number and a unit: 30s, 120m, 2h, 1d."
        })
    }
}
