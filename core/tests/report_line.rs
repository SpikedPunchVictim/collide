//! Tests for the report line (ts-0027): the six line kinds, the lines that are rejected and counted, and the caps
//! (4 KB a line, 100 lines a second with a burst of 1,000, 16 MiB a file). Time is passed in, so a fake clock
//! stands in for the uptime clock.

use collide_core::report::{
    Dropped, FILE_MAX_BYTES, Intake, LINE_MAX_BYTES, RATE_BURST, RATE_PER_SECOND, REPORT_TRUNCATED,
    Rejection, ReportCounts, ReportIntake, ReportLine, TokenBucket, parse_line,
};
use collide_core::schema;
use serde_json::{Value, json};
use std::time::Duration;

fn parse(doc: Value) -> Result<ReportLine, Rejection> {
    parse_line(doc.to_string().as_bytes())
}

/// Parses `doc` as a report line and writes it back: the result must be the same JSON.
fn round_trip(doc: Value) -> ReportLine {
    let line = parse(doc.clone()).unwrap_or_else(|error| panic!("{error:?} in {doc}"));
    assert_eq!(serde_json::to_value(&line).unwrap(), doc);
    line
}

const T: &str = "2026-11-13T09:25:03.004Z";

// The six kinds, as in 0.5's example.

#[test]
fn a_span_start_round_trips() {
    let line = round_trip(
        json!({ "v": 1, "t": T, "kind": "span.start", "id": "4121-1", "name": "map build" }),
    );
    assert!(matches!(line, ReportLine::SpanStart { .. }));
}

#[test]
fn a_span_start_may_name_a_parent() {
    round_trip(
        json!({ "v": 1, "t": T, "kind": "span.start", "id": "4121-2", "name": "n", "parent": "4121-1" }),
    );
}

#[test]
fn a_span_end_round_trips_with_attrs() {
    let line = round_trip(json!({
        "v": 1, "t": T, "kind": "span.end", "id": "4121-1", "status": "ok", "attrs": { "cache": "miss" }
    }));
    assert!(matches!(line, ReportLine::SpanEnd { .. }));
}

#[test]
fn a_metric_round_trips() {
    let line = round_trip(json!({
        "v": 1, "t": T, "kind": "metric", "name": "tests.passed", "value": 412.0, "unit": "count"
    }));
    assert!(matches!(line, ReportLine::Metric { .. }));
}

#[test]
fn a_progress_line_round_trips() {
    let line = round_trip(json!({ "v": 1, "t": T, "kind": "progress", "done": 3, "total": 10 }));
    assert!(matches!(line, ReportLine::Progress { .. }));
}

#[test]
fn a_usage_line_keeps_cache_read_and_write_apart() {
    let line = round_trip(json!({
        "v": 1, "t": T, "kind": "usage", "model": "sonnet",
        "tokensIn": 1200, "tokensOut": 800, "cacheRead": 50000, "cacheWrite": 3000
    }));
    assert!(matches!(line, ReportLine::Usage { .. }));
}

#[test]
fn a_usage_line_may_split_the_cache_write_by_lifetime() {
    round_trip(json!({
        "v": 1, "t": T, "kind": "usage", "model": "sonnet", "cacheWrite1h": 2000, "cacheWrite5m": 1000
    }));
}

#[test]
fn a_usage_line_leaves_out_what_the_reporter_does_not_know() {
    let line = round_trip(json!({ "v": 1, "t": T, "kind": "usage", "model": "sonnet" }));
    let ReportLine::Usage {
        tokens_in,
        cache_read,
        ..
    } = line
    else {
        panic!("not a usage line");
    };
    assert_eq!((tokens_in, cache_read), (None, None));
}

#[test]
fn a_limited_line_round_trips() {
    let line = round_trip(json!({ "v": 1, "t": T, "kind": "limited", "resetsAt": 1763049600 }));
    assert!(matches!(line, ReportLine::Limited { .. }));
}

// Rejected lines.

#[test]
fn a_line_with_no_t_is_rejected_for_every_kind() {
    let lines = [
        json!({ "v": 1, "kind": "span.start", "id": "1-1", "name": "n" }),
        json!({ "v": 1, "kind": "span.end", "id": "1-1", "status": "ok" }),
        json!({ "v": 1, "kind": "metric", "name": "m", "value": 1.0 }),
        json!({ "v": 1, "kind": "progress", "done": 1 }),
        json!({ "v": 1, "kind": "usage", "model": "m" }),
        json!({ "v": 1, "kind": "limited", "resetsAt": 1 }),
    ];
    for doc in lines {
        assert_eq!(parse(doc.clone()), Err(Rejection::NoTimestamp), "{doc}");
    }
}

#[test]
fn a_line_with_a_null_t_is_rejected() {
    let doc = json!({ "v": 1, "t": null, "kind": "progress", "done": 1 });
    assert_eq!(parse(doc), Err(Rejection::NoTimestamp));
}

#[test]
fn a_usage_line_with_a_merged_cached_count_is_rejected() {
    let doc = json!({ "v": 1, "t": T, "kind": "usage", "model": "sonnet", "cached": 53000 });
    assert_eq!(parse(doc), Err(Rejection::MergedCache));
}

#[test]
fn a_merged_cached_count_is_rejected_beside_the_split_ones_too() {
    let doc = json!({ "v": 1, "t": T, "kind": "usage", "model": "m", "cacheRead": 1, "cached": 2 });
    assert_eq!(parse(doc), Err(Rejection::MergedCache));
}

#[test]
fn a_cached_key_on_another_kind_is_malformed_not_a_merged_count() {
    let doc = json!({ "v": 1, "t": T, "kind": "progress", "done": 1, "cached": 2 });
    assert_eq!(parse(doc), Err(Rejection::Malformed));
}

#[test]
fn a_line_that_is_not_a_report_line_is_malformed() {
    let bad: [&[u8]; 9] = [
        b"",
        b"not json",
        b"[]",
        b"42",
        &[0xff, 0xfe],
        br#"{"v":1,"t":"x","kind":"frobnicate"}"#,
        br#"{"v":1,"t":"x","kind":"progress"}"#,
        br#"{"v":1,"t":"x","kind":"progress","done":-1}"#,
        br#"{"v":1,"t":"x","kind":"progress","done":1,"extra":true}"#,
    ];
    for bytes in bad {
        assert_eq!(parse_line(bytes), Err(Rejection::Malformed), "{bytes:?}");
    }
}

#[test]
fn a_line_of_another_version_is_rejected() {
    let doc = json!({ "v": 2, "t": T, "kind": "progress", "done": 1 });
    assert_eq!(parse(doc), Err(Rejection::UnsupportedVersion));
    assert_eq!(
        parse(json!({ "t": T, "kind": "progress", "done": 1 })),
        Err(Rejection::Malformed)
    );
}

// The token bucket, on a fake clock.

fn ms(n: u64) -> Duration {
    Duration::from_millis(n)
}

fn bucket() -> TokenBucket {
    TokenBucket::new(RATE_PER_SECOND, RATE_BURST, Duration::ZERO)
}

#[test]
fn the_caps_are_the_plans() {
    assert_eq!(LINE_MAX_BYTES, 4 * 1024);
    assert_eq!(RATE_PER_SECOND, 100);
    assert_eq!(RATE_BURST, 1_000);
    assert_eq!(FILE_MAX_BYTES, 16 * 1024 * 1024);
    assert_eq!(REPORT_TRUNCATED, "report truncated");
}

#[test]
fn a_burst_of_1000_lines_passes_and_the_1001st_does_not() {
    let mut bucket = bucket();
    for i in 0..1_000 {
        assert!(bucket.take(Duration::ZERO), "line {i}");
    }
    assert!(!bucket.take(Duration::ZERO));
}

#[test]
fn the_bucket_refills_at_100_lines_a_second() {
    let mut bucket = bucket();
    for _ in 0..1_000 {
        bucket.take(Duration::ZERO);
    }
    // One token every 10 ms: a line at each step passes, and a second line at the same instant does not.
    for step in 1..=100 {
        assert!(bucket.take(ms(step * 10)), "step {step}");
        assert!(!bucket.take(ms(step * 10)), "step {step} twice");
    }
}

#[test]
fn half_a_token_is_not_a_line() {
    let mut bucket = bucket();
    for _ in 0..1_000 {
        bucket.take(Duration::ZERO);
    }
    assert!(!bucket.take(ms(5)));
    // The half token stays: another 5 ms makes a whole one.
    assert!(bucket.take(ms(10)));
}

#[test]
fn a_second_of_idle_time_buys_exactly_100_lines() {
    let mut bucket = bucket();
    for _ in 0..1_000 {
        bucket.take(Duration::ZERO);
    }
    let taken = (0..200).filter(|_| bucket.take(ms(1_000))).count();
    assert_eq!(taken, 100);
}

#[test]
fn the_bucket_never_holds_more_than_the_burst() {
    let mut bucket = bucket();
    for _ in 0..1_000 {
        bucket.take(Duration::ZERO);
    }
    let an_hour = Duration::from_secs(3_600);
    let taken = (0..2_000).filter(|_| bucket.take(an_hour)).count();
    assert_eq!(taken, 1_000);
}

#[test]
fn a_clock_that_goes_back_gives_no_tokens() {
    let mut bucket = TokenBucket::new(RATE_PER_SECOND, RATE_BURST, Duration::from_secs(100));
    for _ in 0..1_000 {
        bucket.take(Duration::from_secs(100));
    }
    assert!(!bucket.take(Duration::from_secs(50)));
    // The earlier reading does not count as a new start either.
    assert!(!bucket.take(Duration::from_secs(100)));
}

// The intake: caps and counts.

/// A valid metric line of exactly `len` bytes (no newline).
fn line_of(len: usize) -> Vec<u8> {
    let base = json!({ "v": 1, "t": T, "kind": "metric", "name": "", "value": 1.0 }).to_string();
    let mut name = String::new();
    name.push_str(&"n".repeat(len - base.len()));
    let line = json!({ "v": 1, "t": T, "kind": "metric", "name": name, "value": 1.0 }).to_string();
    assert_eq!(line.len(), len);
    line.into_bytes()
}

fn counts(f: impl FnOnce(&mut ReportCounts)) -> ReportCounts {
    let mut counts = ReportCounts::default();
    f(&mut counts);
    counts
}

#[test]
fn a_line_of_exactly_4_kb_is_kept() {
    let mut intake = ReportIntake::new(Duration::ZERO);
    let outcome = intake.accept(Duration::ZERO, &line_of(LINE_MAX_BYTES));
    assert!(matches!(outcome, Intake::Record(_)), "{outcome:?}");
    assert!(!intake.truncated());
}

#[test]
fn a_line_over_4_kb_is_dropped_and_the_report_is_truncated() {
    let mut intake = ReportIntake::new(Duration::ZERO);
    let outcome = intake.accept(Duration::ZERO, &line_of(LINE_MAX_BYTES + 1));
    assert_eq!(outcome, Intake::Dropped(Dropped::TooLong));
    assert!(intake.truncated());
    assert_eq!(*intake.counts(), counts(|c| c.too_long = 1));
}

#[test]
fn an_oversized_line_is_not_read_as_malformed() {
    let mut intake = ReportIntake::new(Duration::ZERO);
    intake.accept(Duration::ZERO, &vec![b'x'; LINE_MAX_BYTES + 1]);
    assert_eq!(intake.counts().malformed, 0);
    assert_eq!(intake.counts().too_long, 1);
}

#[test]
fn the_intake_drops_the_1001st_line_of_a_burst() {
    let mut intake = ReportIntake::new(Duration::ZERO);
    let line = line_of(100);
    for i in 0..1_000 {
        let outcome = intake.accept(Duration::ZERO, &line);
        assert!(matches!(outcome, Intake::Record(_)), "line {i}");
    }
    assert!(!intake.truncated());
    assert_eq!(
        intake.accept(Duration::ZERO, &line),
        Intake::Dropped(Dropped::RateLimited)
    );
    assert!(intake.truncated());
    assert_eq!(
        *intake.counts(),
        counts(|c| {
            c.accepted = 1_000;
            c.rate_limited = 1;
        })
    );
}

#[test]
fn the_intake_takes_lines_again_as_the_clock_advances() {
    let mut intake = ReportIntake::new(Duration::ZERO);
    let line = line_of(100);
    for _ in 0..1_001 {
        intake.accept(Duration::ZERO, &line);
    }
    assert!(matches!(intake.accept(ms(10), &line), Intake::Record(_)));
    assert!(matches!(
        intake.accept(ms(10), &line),
        Intake::Dropped(Dropped::RateLimited)
    ));
}

#[test]
fn rejected_lines_are_counted_by_reason_and_are_not_dropped() {
    let mut intake = ReportIntake::new(Duration::ZERO);
    let no_t = json!({ "v": 1, "kind": "progress", "done": 1 }).to_string();
    let merged = json!({ "v": 1, "t": T, "kind": "usage", "model": "m", "cached": 5 }).to_string();
    assert_eq!(
        intake.accept(Duration::ZERO, no_t.as_bytes()),
        Intake::Rejected(Rejection::NoTimestamp)
    );
    assert_eq!(
        intake.accept(Duration::ZERO, merged.as_bytes()),
        Intake::Rejected(Rejection::MergedCache)
    );
    intake.accept(Duration::ZERO, b"junk");
    assert_eq!(
        *intake.counts(),
        counts(|c| {
            c.no_timestamp = 1;
            c.merged_cache = 1;
            c.malformed = 1;
        })
    );
    assert!(!intake.truncated());
}

#[test]
fn an_accepted_line_comes_back_as_its_record() {
    let mut intake = ReportIntake::new(Duration::ZERO);
    let doc = json!({ "v": 1, "t": T, "kind": "progress", "done": 3, "total": 10 });
    let outcome = intake.accept(Duration::ZERO, doc.to_string().as_bytes());
    assert_eq!(outcome, Intake::Record(parse(doc).unwrap()));
}

/// Feeds 4096 lines of 4095 bytes plus a newline, which is 16 MiB exactly, 10 ms apart so the rate cap is not hit.
fn fill_the_file(intake: &mut ReportIntake) -> Duration {
    let line = line_of(LINE_MAX_BYTES - 1);
    let mut now = Duration::ZERO;
    for i in 0..4_096 {
        let outcome = intake.accept(now, &line);
        assert!(
            matches!(outcome, Intake::Record(_)),
            "line {i}: {outcome:?}"
        );
        now += ms(10);
    }
    now
}

#[test]
fn a_file_of_exactly_16_mib_is_kept_whole() {
    let mut intake = ReportIntake::new(Duration::ZERO);
    fill_the_file(&mut intake);
    assert!(!intake.truncated());
    assert_eq!(intake.counts().accepted, 4_096);
}

#[test]
fn a_line_past_16_mib_is_dropped_and_so_is_every_later_one() {
    let mut intake = ReportIntake::new(Duration::ZERO);
    let now = fill_the_file(&mut intake);
    let short = line_of(100);
    assert_eq!(
        intake.accept(now, &short),
        Intake::Dropped(Dropped::FileFull)
    );
    assert_eq!(
        intake.accept(now + ms(10), &short),
        Intake::Dropped(Dropped::FileFull)
    );
    assert!(intake.truncated());
    assert_eq!(intake.counts().file_full, 2);
    assert_eq!(intake.counts().accepted, 4_096);
}

#[test]
fn the_bytes_of_dropped_and_rejected_lines_count_toward_the_file() {
    let mut intake = ReportIntake::new(Duration::ZERO);
    let junk = vec![b'x'; LINE_MAX_BYTES - 1];
    let mut now = Duration::ZERO;
    for _ in 0..4_096 {
        intake.accept(now, &junk);
        now += ms(10);
    }
    assert_eq!(intake.counts().malformed, 4_096);
    assert_eq!(
        intake.accept(now, &line_of(100)),
        Intake::Dropped(Dropped::FileFull)
    );
}

// Schema.

#[test]
fn the_registry_lists_the_report_line() {
    let files = schema::registry().render();
    for file in ["ReportLine.input.json", "ReportLine.output.json"] {
        assert!(files.contains_key(file), "{file} is not in the registry");
    }
}
