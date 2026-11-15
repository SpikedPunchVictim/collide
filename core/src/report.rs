//! The report line (0.5, Reporting): one JSON object per line, appended to `$COLLIDE_REPORT` by anything in a
//! hook's process tree. This module is the pure half of the tailer: it reads one line at a time, applies the
//! caps and counts what it turns away. It reads no clock and logs nothing. The caller passes the time as an
//! uptime reading, and reads the counts and the `truncated` flag back.
//!
//! Not here, because each needs more than one line or the process that wrote it: the span-id and nesting rules,
//! `unclosed` spans, the `inherited` mark, and the rule that `usage` and `limited` count only from a `script`
//! phase with `agentWork: true`. A later item that builds the tailer applies them to the lines this returns.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::time::Duration;

/// The `v` of every line this reads.
pub const REPORT_VERSION: u32 = 1;

/// The longest line, not counting its newline.
pub const LINE_MAX_BYTES: usize = 4 * 1024;

/// Lines taken per second, sustained.
pub const RATE_PER_SECOND: u64 = 100;

/// Lines taken at once, from a full bucket.
pub const RATE_BURST: u64 = 1_000;

/// The most bytes of a file, newlines included.
pub const FILE_MAX_BYTES: u64 = 16 * 1024 * 1024;

/// What the span is marked with once a line has been dropped.
pub const REPORT_TRUNCATED: &str = "report truncated";

/// One report line. Every line carries `v` and its timestamp `t`, because a time stamped at reading would be wrong
/// by the tailing lag. Each variant repeats `v` and `t`: serde cannot flatten them out of a tagged enum that denies
/// unknown keys. A key a reporter does not know is left out, and shows as "not reported" (0.13).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all_fields = "camelCase", deny_unknown_fields)]
pub enum ReportLine {
    #[serde(rename = "span.start")]
    SpanStart {
        v: u32,
        t: String,
        id: String,
        name: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        parent: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attrs: Option<BTreeMap<String, Value>>,
    },
    #[serde(rename = "span.end")]
    SpanEnd {
        v: u32,
        t: String,
        id: String,
        status: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attrs: Option<BTreeMap<String, Value>>,
    },
    #[serde(rename = "metric")]
    Metric {
        v: u32,
        t: String,
        name: String,
        value: f64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        unit: Option<String>,
    },
    #[serde(rename = "progress")]
    Progress {
        v: u32,
        t: String,
        done: u64,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        total: Option<u64>,
    },
    /// Keeps `cacheRead` and `cacheWrite` apart, and may split `cacheWrite` into `cacheWrite1h` and `cacheWrite5m`.
    /// A merged `cached` count is rejected: a write and a read are priced far apart.
    #[serde(rename = "usage")]
    Usage {
        v: u32,
        t: String,
        model: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tokens_in: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        tokens_out: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_read: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_write: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_write_1h: Option<u64>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        cache_write_5m: Option<u64>,
    },
    /// `resetsAt` is the reset time in epoch seconds.
    #[serde(rename = "limited")]
    Limited { v: u32, t: String, resets_at: u64 },
}

/// Why a line was turned away, though it fit the caps. Each is counted, never hidden.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// Not JSON, not an object, an unknown kind or key, or a value of the wrong type.
    Malformed,
    /// No `t`, or a null one.
    NoTimestamp,
    /// A `usage` line with a merged `cached` count.
    MergedCache,
    /// A `v` other than [`REPORT_VERSION`].
    UnsupportedVersion,
}

/// Reads one line, without its newline. A line with no `t` is [`Rejection::NoTimestamp`] whatever else is wrong
/// with it, so that it is counted as the missing time.
pub fn parse_line(line: &[u8]) -> Result<ReportLine, Rejection> {
    let value: Value = serde_json::from_slice(line).map_err(|_| Rejection::Malformed)?;
    let Some(object) = value.as_object() else {
        return Err(Rejection::Malformed);
    };
    if matches!(object.get("t"), None | Some(Value::Null)) {
        return Err(Rejection::NoTimestamp);
    }
    if let Some(v) = object.get("v").and_then(Value::as_u64)
        && v != u64::from(REPORT_VERSION)
    {
        return Err(Rejection::UnsupportedVersion);
    }
    if object.get("kind") == Some(&Value::from("usage")) && object.contains_key("cached") {
        return Err(Rejection::MergedCache);
    }
    serde_json::from_value(value).map_err(|_| Rejection::Malformed)
}

/// One token is this many units, so that refilling is whole-number arithmetic and no rounding can move a boundary.
const UNITS_PER_TOKEN: u128 = 1_000_000_000;

/// A token bucket that is told the time. `now` is an uptime reading: only differences between readings matter.
#[derive(Debug, Clone)]
pub struct TokenBucket {
    per_second: u128,
    capacity: u128,
    units: u128,
    last: Duration,
}

impl TokenBucket {
    /// A full bucket of `burst` tokens that refills `per_second` tokens a second, starting at `now`.
    pub fn new(per_second: u64, burst: u64, now: Duration) -> Self {
        let capacity = u128::from(burst) * UNITS_PER_TOKEN;
        TokenBucket {
            per_second: u128::from(per_second),
            capacity,
            units: capacity,
            last: now,
        }
    }

    /// Takes one token at `now`, or says there is none. A `now` before the last reading gives no tokens and does
    /// not move the last reading.
    pub fn take(&mut self, now: Duration) -> bool {
        if now > self.last {
            // A token a second is one unit a nanosecond, so the units earned are nanoseconds times the rate.
            let earned = (now - self.last).as_nanos() * self.per_second;
            self.units = (self.units + earned).min(self.capacity);
            self.last = now;
        }
        if self.units >= UNITS_PER_TOKEN {
            self.units -= UNITS_PER_TOKEN;
            true
        } else {
            false
        }
    }
}

/// Why a line was dropped unread. Dropped lines mark the report truncated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dropped {
    /// Over [`LINE_MAX_BYTES`].
    TooLong,
    /// The bucket was empty.
    RateLimited,
    /// The file is past [`FILE_MAX_BYTES`]. Every later line is dropped too.
    FileFull,
}

/// What became of one line.
#[derive(Debug, Clone, PartialEq)]
pub enum Intake {
    Record(ReportLine),
    Rejected(Rejection),
    Dropped(Dropped),
}

/// How many lines went each way.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReportCounts {
    pub accepted: u64,
    pub malformed: u64,
    pub no_timestamp: u64,
    pub merged_cache: u64,
    pub unsupported_version: u64,
    pub too_long: u64,
    pub rate_limited: u64,
    pub file_full: u64,
}

/// The lines of one report file, in order. Every line is checked against the caps in this order: the file's size
/// (its bytes count whether the line is kept or not), then the rate (every line costs a token, so a flood of junk
/// is limited too), then the line's length, and only then is it parsed.
#[derive(Debug, Clone)]
pub struct ReportIntake {
    bucket: TokenBucket,
    file_bytes: u64,
    counts: ReportCounts,
    truncated: bool,
}

impl ReportIntake {
    /// The start of a file at `now`.
    pub fn new(now: Duration) -> Self {
        ReportIntake {
            bucket: TokenBucket::new(RATE_PER_SECOND, RATE_BURST, now),
            file_bytes: 0,
            counts: ReportCounts::default(),
            truncated: false,
        }
    }

    /// Takes the next complete line, without its newline, read at `now`.
    pub fn accept(&mut self, now: Duration, line: &[u8]) -> Intake {
        self.file_bytes += line.len() as u64 + 1;
        if self.file_bytes > FILE_MAX_BYTES {
            return self.drop_line(Dropped::FileFull);
        }
        if !self.bucket.take(now) {
            return self.drop_line(Dropped::RateLimited);
        }
        if line.len() > LINE_MAX_BYTES {
            return self.drop_line(Dropped::TooLong);
        }
        match parse_line(line) {
            Ok(record) => {
                self.counts.accepted += 1;
                Intake::Record(record)
            }
            Err(rejection) => {
                match rejection {
                    Rejection::Malformed => self.counts.malformed += 1,
                    Rejection::NoTimestamp => self.counts.no_timestamp += 1,
                    Rejection::MergedCache => self.counts.merged_cache += 1,
                    Rejection::UnsupportedVersion => self.counts.unsupported_version += 1,
                }
                Intake::Rejected(rejection)
            }
        }
    }

    fn drop_line(&mut self, reason: Dropped) -> Intake {
        match reason {
            Dropped::TooLong => self.counts.too_long += 1,
            Dropped::RateLimited => self.counts.rate_limited += 1,
            Dropped::FileFull => self.counts.file_full += 1,
        }
        self.truncated = true;
        Intake::Dropped(reason)
    }

    pub fn counts(&self) -> &ReportCounts {
        &self.counts
    }

    /// Whether any line was dropped: the span is then marked [`REPORT_TRUNCATED`].
    pub fn truncated(&self) -> bool {
        self.truncated
    }
}
