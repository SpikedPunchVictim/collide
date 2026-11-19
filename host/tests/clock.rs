//! Tests for the clocks (ts-0051; 0.2, Clocks; 0.13). Fake clocks inject a sleep without waiting, and one real test
//! reads the machine's own clocks a few times.

use collide_host::clock::{
    Clock, ClockMismatch, ClockPair, Clocks, Reading, SystemClocks, Timer, civil_date,
    local_utc_offset, sleep_gap,
};
use std::cell::Cell;
use std::thread;
use std::time::Duration;

const SECOND: Duration = Duration::from_secs(1);

/// Three clocks a test moves by hand. `awake` moves all of them, as a machine that stays up does. `sleep` moves the
/// continuous and the wall clock only, as a machine that sleeps does (0.2).
struct FakeClocks {
    uptime: Cell<u64>,
    continuous: Cell<u64>,
    wall: Cell<u64>,
}

impl FakeClocks {
    fn new() -> FakeClocks {
        FakeClocks {
            uptime: Cell::new(1_000_000_000),
            continuous: Cell::new(5_000_000_000),
            wall: Cell::new(1_700_000_000_000_000_000),
        }
    }

    fn awake(&self, span: Duration) {
        self.uptime.set(self.uptime.get() + span.as_nanos() as u64);
        self.sleep(span);
    }

    fn sleep(&self, span: Duration) {
        self.continuous
            .set(self.continuous.get() + span.as_nanos() as u64);
        self.wall.set(self.wall.get() + span.as_nanos() as u64);
    }
}

impl Clocks for FakeClocks {
    fn read(&self, clock: Clock) -> Reading {
        let nanos = match clock {
            Clock::Uptime => self.uptime.get(),
            Clock::Continuous => self.continuous.get(),
            Clock::Wall => self.wall.get(),
        };
        Reading { clock, nanos }
    }
}

// The sleep gap (0.13: sleep is not work).

#[test]
fn a_machine_that_stayed_awake_has_no_sleep_gap() {
    let clocks = FakeClocks::new();
    let earlier = ClockPair::read(&clocks);
    clocks.awake(Duration::from_secs(90));
    assert_eq!(
        sleep_gap(&earlier, &ClockPair::read(&clocks)),
        Duration::ZERO
    );
}

#[test]
fn a_sleep_in_the_middle_is_the_whole_gap() {
    let clocks = FakeClocks::new();
    let earlier = ClockPair::read(&clocks);
    clocks.awake(Duration::from_secs(10));
    clocks.sleep(Duration::from_secs(3600));
    clocks.awake(Duration::from_secs(5));
    assert_eq!(
        sleep_gap(&earlier, &ClockPair::read(&clocks)),
        Duration::from_secs(3600)
    );
}

#[test]
fn two_sleeps_add_up_to_one_gap() {
    let clocks = FakeClocks::new();
    let earlier = ClockPair::read(&clocks);
    clocks.sleep(Duration::from_secs(60));
    clocks.awake(SECOND);
    clocks.sleep(Duration::from_secs(120));
    assert_eq!(
        sleep_gap(&earlier, &ClockPair::read(&clocks)),
        Duration::from_secs(180)
    );
}

#[test]
fn a_gap_is_the_same_wherever_the_clocks_started() {
    let clocks = FakeClocks::new();
    clocks.awake(Duration::from_secs(7));
    clocks.sleep(Duration::from_secs(1000));
    let earlier = ClockPair::read(&clocks);
    clocks.sleep(Duration::from_secs(2));
    assert_eq!(
        sleep_gap(&earlier, &ClockPair::read(&clocks)),
        Duration::from_secs(2)
    );
}

#[test]
fn pairs_given_in_the_wrong_order_have_no_gap_and_do_not_panic() {
    let clocks = FakeClocks::new();
    let earlier = ClockPair::read(&clocks);
    clocks.sleep(Duration::from_secs(60));
    let later = ClockPair::read(&clocks);
    assert_eq!(sleep_gap(&later, &earlier), Duration::ZERO);
}

#[test]
fn a_gap_from_a_pair_whose_uptime_ran_ahead_is_zero_not_negative() {
    // The two clocks are read a moment apart, so awake the uptime delta can pass the continuous one by a few ns.
    let earlier = ClockPair::read(&FakeClocks::new());
    let clocks = FakeClocks::new();
    clocks.uptime.set(clocks.uptime.get() + 500);
    assert_eq!(
        sleep_gap(&earlier, &ClockPair::read(&clocks)),
        Duration::ZERO
    );
}

// The timers.

#[test]
fn a_work_timeout_on_the_uptime_clock_does_not_count_sleep() {
    let clocks = FakeClocks::new();
    let timeout = Timer::start(
        Clock::Uptime,
        Duration::from_secs(60),
        clocks.read(Clock::Uptime),
    )
    .unwrap();
    clocks.awake(Duration::from_secs(10));
    clocks.sleep(Duration::from_secs(3600));
    let now = clocks.read(Clock::Uptime);
    assert_eq!(timeout.is_over(now), Ok(false));
    assert_eq!(timeout.remaining(now), Ok(Duration::from_secs(50)));
}

#[test]
fn a_work_timeout_ends_once_enough_uptime_has_passed() {
    let clocks = FakeClocks::new();
    let timeout = Timer::start(
        Clock::Uptime,
        Duration::from_secs(60),
        clocks.read(Clock::Uptime),
    )
    .unwrap();
    clocks.awake(Duration::from_secs(60));
    let now = clocks.read(Clock::Uptime);
    assert_eq!(timeout.is_over(now), Ok(true));
    assert_eq!(timeout.remaining(now), Ok(Duration::ZERO));
}

#[test]
fn the_cache_countdown_on_the_continuous_clock_counts_sleep() {
    let clocks = FakeClocks::new();
    let countdown = Timer::start(
        Clock::Continuous,
        Duration::from_secs(300),
        clocks.read(Clock::Continuous),
    )
    .unwrap();
    clocks.awake(Duration::from_secs(10));
    clocks.sleep(Duration::from_secs(3600));
    let now = clocks.read(Clock::Continuous);
    assert_eq!(countdown.is_over(now), Ok(true));
    assert_eq!(countdown.remaining(now), Ok(Duration::ZERO));
}

#[test]
fn the_cache_countdown_has_time_left_before_it_ends() {
    let clocks = FakeClocks::new();
    let countdown = Timer::start(
        Clock::Continuous,
        Duration::from_secs(300),
        clocks.read(Clock::Continuous),
    )
    .unwrap();
    clocks.sleep(Duration::from_secs(100));
    let now = clocks.read(Clock::Continuous);
    assert_eq!(countdown.is_over(now), Ok(false));
    assert_eq!(countdown.remaining(now), Ok(Duration::from_secs(200)));
}

#[test]
fn the_usage_limit_reset_on_the_wall_clock_counts_sleep() {
    let clocks = FakeClocks::new();
    let reset = Timer::start(
        Clock::Wall,
        Duration::from_secs(2 * 3600),
        clocks.read(Clock::Wall),
    )
    .unwrap();
    clocks.sleep(Duration::from_secs(3 * 3600));
    assert_eq!(reset.is_over(clocks.read(Clock::Wall)), Ok(true));
}

#[test]
fn a_timer_of_no_length_is_over_at_once() {
    let clocks = FakeClocks::new();
    for clock in Clock::ALL {
        let now = clocks.read(clock);
        let timer = Timer::start(clock, Duration::ZERO, now).unwrap();
        assert_eq!(timer.is_over(now), Ok(true), "{clock}");
    }
}

#[test]
fn a_timer_reads_a_moment_before_its_start_as_all_of_its_length_left() {
    // The wall clock can be set back: that must not make a timer panic or end early.
    let clocks = FakeClocks::new();
    let timer = Timer::start(Clock::Wall, SECOND, clocks.read(Clock::Wall)).unwrap();
    let before = Reading {
        clock: Clock::Wall,
        nanos: clocks.wall.get() - 10 * 1_000_000_000,
    };
    assert_eq!(timer.is_over(before), Ok(false));
    assert_eq!(timer.remaining(before), Ok(SECOND));
}

#[test]
fn a_timer_longer_than_the_clock_can_count_never_ends_and_does_not_overflow() {
    let clocks = FakeClocks::new();
    let timer = Timer::start(Clock::Uptime, Duration::MAX, clocks.read(Clock::Uptime)).unwrap();
    clocks.awake(Duration::from_secs(10));
    assert_eq!(timer.is_over(clocks.read(Clock::Uptime)), Ok(false));
}

#[test]
fn a_timer_says_which_clock_it_runs_on() {
    let clocks = FakeClocks::new();
    for clock in Clock::ALL {
        let timer = Timer::start(clock, SECOND, clocks.read(clock)).unwrap();
        assert_eq!(timer.clock(), clock);
    }
}

// The refusals.

#[test]
fn a_timer_cannot_start_on_one_clock_with_a_reading_of_another() {
    let clocks = FakeClocks::new();
    for clock in Clock::ALL {
        for got in Clock::ALL.into_iter().filter(|got| *got != clock) {
            assert_eq!(
                Timer::start(clock, SECOND, clocks.read(got)),
                Err(ClockMismatch {
                    expected: clock,
                    got
                }),
                "start on {clock} with a {got} reading"
            );
        }
    }
}

#[test]
fn a_started_timer_refuses_a_reading_of_another_clock() {
    let clocks = FakeClocks::new();
    for clock in Clock::ALL {
        let timer = Timer::start(clock, SECOND, clocks.read(clock)).unwrap();
        for got in Clock::ALL.into_iter().filter(|got| *got != clock) {
            let mismatch = ClockMismatch {
                expected: clock,
                got,
            };
            assert_eq!(
                timer.is_over(clocks.read(got)),
                Err(mismatch),
                "{clock}/{got}"
            );
            assert_eq!(
                timer.remaining(clocks.read(got)),
                Err(mismatch),
                "{clock}/{got}"
            );
        }
    }
}

// The real clocks.

#[test]
fn real_readings_carry_the_clock_they_were_asked_for() {
    for clock in Clock::ALL {
        assert_eq!(SystemClocks.read(clock).clock, clock);
    }
}

#[test]
fn real_readings_never_go_backwards_over_repeated_reads() {
    for clock in [Clock::Uptime, Clock::Continuous] {
        let mut last = SystemClocks.read(clock).nanos;
        for _ in 0..1000 {
            let now = SystemClocks.read(clock).nanos;
            assert!(now >= last, "{clock} went from {last} to {now}");
            last = now;
        }
    }
}

#[test]
fn real_uptime_and_continuous_time_advance_together_while_awake() {
    let before = ClockPair::read(&SystemClocks);
    let uptime_before = SystemClocks.read(Clock::Uptime).nanos;
    let continuous_before = SystemClocks.read(Clock::Continuous).nanos;
    thread::sleep(Duration::from_millis(5));
    let uptime_span = SystemClocks.read(Clock::Uptime).nanos - uptime_before;
    let continuous_span = SystemClocks.read(Clock::Continuous).nanos - continuous_before;
    let after = ClockPair::read(&SystemClocks);
    let five_ms = 5_000_000;
    assert!(uptime_span >= five_ms, "uptime moved {uptime_span} ns");
    assert!(
        continuous_span >= five_ms,
        "continuous moved {continuous_span} ns"
    );
    let gap = sleep_gap(&before, &after);
    assert!(
        gap < Duration::from_millis(50),
        "an awake machine reported a sleep gap of {gap:?}"
    );
}

#[test]
fn the_real_wall_clock_is_the_unix_time_in_nanoseconds() {
    // 2025-01-01 in ns since the epoch, and a bound far past any machine now.
    let wall = SystemClocks.read(Clock::Wall).nanos;
    assert!(wall > 1_735_689_600_000_000_000, "{wall}");
    assert!(wall < 4_000_000_000_000_000_000, "{wall}");
}

// The calendar date of a wall reading (0.10, the actor's `at`).

const SECOND_NANOS: u64 = 1_000_000_000;

#[test]
fn the_date_of_the_epoch_in_utc_is_the_first_of_january_1970() {
    assert_eq!(civil_date(0, 0), "1970-01-01");
}

#[test]
fn the_date_of_a_midday_reading_is_that_day() {
    assert_eq!(civil_date(1_781_524_800 * SECOND_NANOS, 0), "2026-06-15");
}

#[test]
fn the_last_second_of_a_leap_day_is_still_that_day() {
    assert_eq!(civil_date(1_709_251_199 * SECOND_NANOS, 0), "2024-02-29");
}

#[test]
fn the_year_2100_has_no_leap_day() {
    assert_eq!(civil_date(4_107_542_399 * SECOND_NANOS, 0), "2100-02-28");
    assert_eq!(civil_date(4_107_542_400 * SECOND_NANOS, 0), "2100-03-01");
}

#[test]
fn a_zone_west_of_utc_dates_the_early_hours_as_the_day_before() {
    let half_past_midnight = 1_794_961_800 * SECOND_NANOS;
    assert_eq!(civil_date(half_past_midnight, 0), "2026-11-18");
    assert_eq!(civil_date(half_past_midnight, -5 * 3600), "2026-11-17");
}

#[test]
fn a_zone_east_of_utc_dates_the_late_hours_as_the_day_after() {
    let late = 1_794_961_800 * SECOND_NANOS - 3600 * SECOND_NANOS;
    assert_eq!(civil_date(late, 0), "2026-11-17");
    assert_eq!(civil_date(late, 14 * 3600), "2026-11-18");
}

#[test]
fn the_machines_own_offset_is_a_whole_number_of_minutes_inside_a_day() {
    let offset = local_utc_offset(1_781_524_800 * SECOND_NANOS);
    assert_eq!(offset % 60, 0, "{offset}");
    assert!(offset.abs() <= 14 * 3600, "{offset}");
}
