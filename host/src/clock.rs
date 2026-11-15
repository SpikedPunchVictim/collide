//! The three clocks (0.2, Clocks; R5). macOS has three, and a wrong choice breaks a timeout or a countdown the first
//! time a laptop sleeps, so a timer names the clock it runs on ([`Timer::start`] takes a [`Clock`], with no default)
//! and a reading carries the clock it came from ([`Reading`]). Collide never uses one where another is meant:
//!
//! | clock | stops in sleep | use |
//! |---|---|---|
//! | uptime (`mach_absolute_time`, `CLOCK_UPTIME_RAW`) | yes | work timeouts: a phase, an ask, a `wait` timeout (0.7) |
//! | continuous (`CLOCK_MONOTONIC_RAW`) | no | the cache countdown (0.3, 0.13) and spans shown as elapsed |
//! | wall (`SystemTime`) | no, and can jump | the usage-limit reset (0.3), event timestamps, the journal's `wall_ns`, display |
//!
//! A test reads this table from the source of this file and checks it against [`Clock`], so a clock added to the enum
//! without a row, or the reverse, fails.
//!
//! ## Core reads no clock
//!
//! `core` takes time as values: its pending-poll rules take [`Uptime`], a whole-second length (`WaitTiming`,
//! `NextPoll`). That alias stays where it is, and this module is the host's side of it: a [`Reading`] of the uptime
//! clock converts to it with [`Reading::as_core_uptime`], and a reading of another clock does not. Reading a clock is
//! the host's job ([`Clocks`]), and a test drives the rules with [`Clocks`] fakes.
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `clock: system readings` | [`SystemClocks`] | the real readings, through `clock_gettime` and `mach_absolute_time` |
//! | `clock: timer` | [`Timer`] | a start, a length and the clock they are on; when it ends and what is left |
//! | `clock: sleep gap` | [`sleep_gap`] | how long the machine slept between two moments |

use collide_core::run_rules::Uptime;
use std::fmt;
use std::time::Duration;

/// A clock of the table above. The doc comment of each variant is the use that clock is for, and the table is the
/// same list in one place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Clock {
    /// Time the machine was awake, which stops while it sleeps. **Work timeouts**: a phase's, an ask's and a `wait`
    /// phase's `timeout:`. A phase that slept for an hour has not worked for an hour.
    Uptime,
    /// Time since boot that keeps counting through sleep, and never jumps. **The cache countdown** (a cache that
    /// expires in five minutes does not wait for the laptop to wake) **and spans shown as elapsed**.
    Continuous,
    /// The calendar clock, which keeps counting through sleep and can be set by the user or the network.
    /// **Wall display and limit reset**: the usage-limit reset time of day, event timestamps, the journal's
    /// `wall_ns`, and anything shown to a person as a time.
    Wall,
}

impl Clock {
    /// Every clock, in the order of the table.
    pub const ALL: [Clock; 3] = [Clock::Uptime, Clock::Continuous, Clock::Wall];

    /// The name the table gives the clock, which is also what an error says.
    pub fn name(self) -> &'static str {
        match self {
            Clock::Uptime => "uptime",
            Clock::Continuous => "continuous",
            Clock::Wall => "wall",
        }
    }

    /// The table's second column: whether the clock stops while the machine sleeps.
    pub fn stops_in_sleep(self) -> bool {
        matches!(self, Clock::Uptime)
    }
}

impl fmt::Display for Clock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One reading of one clock, in nanoseconds. For [`Clock::Uptime`] and [`Clock::Continuous`] the origin is the
/// clock's own and no two clocks share one, so only readings of the same clock compare. For [`Clock::Wall`] the
/// origin is the Unix epoch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reading {
    pub clock: Clock,
    pub nanos: u64,
}

impl Reading {
    /// The reading as `core`'s pending-poll rules take it ([`Uptime`], whole seconds, rounded down), or `None` for a
    /// reading of another clock: a work timeout is never measured on the wrong clock.
    pub fn as_core_uptime(&self) -> Option<Uptime> {
        (self.clock == Clock::Uptime).then(|| Uptime::from_secs(self.nanos / 1_000_000_000))
    }
}

/// A reading of one clock was used with another.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockMismatch {
    pub expected: Clock,
    pub got: Clock,
}

impl fmt::Display for ClockMismatch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "a {} clock reading was given where the {} clock is meant",
            self.got, self.expected
        )
    }
}

impl std::error::Error for ClockMismatch {}

/// Where readings come from. The daemon and the shim take it as a value (0.15, Dependency injection), so a test
/// injects a gap of sleep with a fake and never waits.
pub trait Clocks {
    fn read(&self, clock: Clock) -> Reading;
}

/// The real clocks.
pub struct SystemClocks;

impl Clocks for SystemClocks {
    fn read(&self, clock: Clock) -> Reading {
        let _ = clock;
        todo!("clock: system readings")
    }
}

/// A span of time on one clock, started at a reading. A work timeout is a `Timer` on [`Clock::Uptime`], the cache
/// countdown one on [`Clock::Continuous`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Timer {
    clock: Clock,
    started: Reading,
    length: Duration,
}

impl Timer {
    /// Starts a timer of `length` on `clock` at `now`. The clock is named by the caller, and `now` must be a reading
    /// of it ([`ClockMismatch`] otherwise).
    pub fn start(clock: Clock, length: Duration, now: Reading) -> Result<Timer, ClockMismatch> {
        let _ = (clock, length, now);
        todo!("clock: timer")
    }

    /// The clock the timer runs on.
    pub fn clock(&self) -> Clock {
        self.clock
    }

    /// Whether the timer has ended at `now`.
    pub fn is_over(&self, now: Reading) -> Result<bool, ClockMismatch> {
        let _ = now;
        todo!("clock: timer")
    }

    /// What is left at `now`, zero once it is over.
    pub fn remaining(&self, now: Reading) -> Result<Duration, ClockMismatch> {
        let _ = now;
        todo!("clock: timer")
    }
}

/// The uptime and the continuous clock read together, so that a later pair shows how long the machine slept between
/// two moments.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockPair {
    uptime: Reading,
    continuous: Reading,
}

impl ClockPair {
    pub fn read(clocks: &dyn Clocks) -> ClockPair {
        ClockPair {
            uptime: clocks.read(Clock::Uptime),
            continuous: clocks.read(Clock::Continuous),
        }
    }
}

/// How long the machine slept between `earlier` and `later`: the time the continuous clock counted and the uptime
/// clock did not (0.2, Clocks; 0.13). A phase counts the gap as waiting, not as active time.
pub fn sleep_gap(earlier: &ClockPair, later: &ClockPair) -> Duration {
    let _ = (earlier, later);
    todo!("clock: sleep gap")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The rows of the table in this file's own doc comment: the first word of the clock cell, the `yes` or `no`
    /// that starts the sleep cell, and the use cell.
    fn doc_table_rows() -> Vec<(String, bool, String)> {
        let source = include_str!("clock.rs");
        let rows: Vec<Vec<&str>> = source
            .lines()
            .filter_map(|line| line.strip_prefix("//! |"))
            .map(|row| {
                row.trim_end_matches('|')
                    .split('|')
                    .map(str::trim)
                    .collect()
            })
            .filter(|cells: &Vec<&str>| cells.len() == 3)
            .collect();
        // Past the header and its rule line, up to the header of the tag table further down.
        let start = rows
            .iter()
            .position(|cells| cells[0] == "clock")
            .expect("the table has a header row");
        rows[start + 2..]
            .iter()
            .take_while(|cells| cells[0] != "tag")
            .map(|cells| {
                let name = cells[0].split_whitespace().next().unwrap().to_string();
                let stops = match cells[1].split([' ', ',']).next().unwrap() {
                    "yes" => true,
                    "no" => false,
                    other => {
                        panic!("the sleep cell of {name} starts with `{other}`, not yes or no")
                    }
                };
                (name, stops, cells[2].to_string())
            })
            .collect()
    }

    #[test]
    fn the_doc_table_and_the_enum_agree() {
        let rows = doc_table_rows();
        let names: Vec<&str> = rows.iter().map(|row| row.0.as_str()).collect();
        let enum_names: Vec<&str> = Clock::ALL.iter().map(|clock| clock.name()).collect();
        assert_eq!(names, enum_names);
        for (row, clock) in rows.iter().zip(Clock::ALL) {
            assert_eq!(row.1, clock.stops_in_sleep(), "stops in sleep, {}", row.0);
            assert!(!row.2.is_empty(), "the use of {} is empty", row.0);
        }
    }

    #[test]
    fn the_table_has_exactly_three_clocks() {
        assert_eq!(doc_table_rows().len(), 3);
        assert_eq!(Clock::ALL.len(), 3);
    }

    #[test]
    fn each_clock_variant_has_a_doc_comment_naming_its_use() {
        let source = include_str!("clock.rs");
        let body = &source[source.find("pub enum Clock {").unwrap()..];
        let body = &body[..body.find("\n}\n").unwrap()];
        for (variant, use_) in [
            ("Uptime", "Work timeouts"),
            ("Continuous", "The cache countdown"),
            ("Wall", "Wall display and limit reset"),
        ] {
            let at = body.find(&format!("    {variant},")).unwrap();
            let doc: Vec<&str> = body[..at]
                .lines()
                .rev()
                .take_while(|line| line.trim_start().starts_with("///"))
                .collect();
            assert!(
                doc.join(" ").contains(use_),
                "the doc comment of {variant} does not name its use ({use_})"
            );
        }
    }

    #[test]
    fn an_uptime_reading_converts_to_the_core_alias_in_whole_seconds() {
        let reading = Reading {
            clock: Clock::Uptime,
            nanos: 90_500_000_000,
        };
        assert_eq!(reading.as_core_uptime(), Some(Uptime::from_secs(90)));
    }

    #[test]
    fn a_reading_of_another_clock_has_no_core_uptime() {
        for clock in [Clock::Continuous, Clock::Wall] {
            assert_eq!(Reading { clock, nanos: 1 }.as_core_uptime(), None);
        }
    }

    #[test]
    fn a_clock_pair_reads_both_clocks_it_is_made_of() {
        struct Fixed;
        impl Clocks for Fixed {
            fn read(&self, clock: Clock) -> Reading {
                Reading {
                    clock,
                    nanos: match clock {
                        Clock::Uptime => 1,
                        Clock::Continuous => 2,
                        Clock::Wall => 3,
                    },
                }
            }
        }
        let pair = ClockPair::read(&Fixed);
        assert_eq!((pair.uptime.nanos, pair.continuous.nanos), (1, 2));
    }

    #[test]
    fn a_mismatch_names_both_clocks() {
        let message = ClockMismatch {
            expected: Clock::Uptime,
            got: Clock::Wall,
        }
        .to_string();
        assert!(
            message.contains("wall") && message.contains("uptime"),
            "{message}"
        );
    }

    #[test]
    #[should_panic(expected = "clock: system readings")]
    fn the_system_clocks_are_a_stub_tagged_for_stage_3a() {
        SystemClocks.read(Clock::Wall);
    }
}
