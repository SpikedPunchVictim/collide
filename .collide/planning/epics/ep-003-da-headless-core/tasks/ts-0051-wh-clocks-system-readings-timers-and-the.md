---
id: ts-0051-wh
kind: task
status: active
title: "Clocks: system readings, timers and the sleep gap"
owners: []
links: []
---

## Goal

0.2's clock table behind the `Clocks` trait of `host/src/clock.rs`: real readings of the uptime clock (stops in sleep), the continuous clock (counts sleep) and wall time; timers that refuse a reading from another clock; and the sleep gap between two paired readings. Replaces the stubs tagged `clock: system readings`, `clock: timer` and `clock: sleep gap`.

## Success criteria

- A sleep gap injected through fake clocks counts as waiting, not active time, and a work timeout counts uptime only (Stage 3a criterion).
- A timer started on one clock and read with a reading of another is `ClockMismatch`. Work timeouts use uptime, the cache countdown uses continuous time, and the usage-limit reset uses wall time, as the table says.
- Real readings never go backwards over repeated reads, and uptime and continuous time advance together while the machine is awake (a short real test, no sleep needed).
- No stub tagged `clock:` remains.

## Tests

Write first: fake-clock tests for the gap and for each timer kind, the mismatch refusal, and one real-clock test of monotonic readings.

## Notes

- IMPLEMENTATION_PLAN.md, Stage 3a (Clocks and disk); 0.2 (Clocks, the table with the macOS calls for each clock); 0.13 (sleep intervals counted as waiting).
- Area: `host` (`clock.rs`). The owner's real sleep test is Stage 3b, not here.
- Stage 3a item (revision 13 scope, split 2026-11-15). Items of Stage 3a run one at a time, in id order, because they share `host` and the generated files.

### Built

- `host/src/clock.rs`: the three stubs are filled, and no tag `clock:` remains (`pnpm check:stubs` lists none of them; the entries stay in `stub-tags.json`). The types and signatures are as outlined.
  - `SystemClocks` reads the uptime clock with `clock_gettime(CLOCK_UPTIME_RAW)`, the continuous clock with `clock_gettime(CLOCK_MONOTONIC_RAW)` (both in one function, `monotonic_nanos`, which holds the only `unsafe` block, with its `// SAFETY:` comment), and the wall clock from `SystemTime` in nanoseconds since the Unix epoch. A failing `clock_gettime` stops with a message rather than return a zero a timeout would trust. A wall clock set before 1970 reads 0 and one past 2554 reads `u64::MAX`; a reading never panics.
  - `Timer::start` refuses a reading of another clock (`ClockMismatch { expected, got }`). `is_over` is "the whole length has passed" (a zero length is over at once), `remaining` is the length less what has passed, zero once over. Both refuse a reading of another clock. A reading from before the start (the wall clock set back) counts as no time, and elapsed time is compared as a span, so `Duration::MAX` cannot overflow.
  - `sleep_gap(earlier, later)` is `Δcontinuous − Δuptime`, never below zero.
  - The test of the stub (`the_system_clocks_are_a_stub_tagged_for_stage_3a`) is removed; the other 8 unit tests of the file are unchanged.
- `host/Cargo.toml`: `libc = "0.2"` under `[target.'cfg(target_os = "macos")'.dependencies]`. `Cargo.lock` gains the `libc` line of `collide-host` (the crate was already locked at 0.2.191 for other dependencies).
- Tests: `host/tests/clock.rs` (new, 21). Fake clocks (`awake` moves all three, `sleep` moves the continuous and wall clocks only): the gap is zero awake, is the whole sleep when one sleep sits in the middle, adds two sleeps, does not depend on where the clocks started, and is zero for pairs in the wrong order or an uptime a few ns ahead (6). A work timeout on uptime ignores an hour of sleep and ends after enough uptime; the cache countdown on the continuous clock and the limit reset on the wall clock count sleep; a zero-length timer is over at once; a reading before the start; a `Duration::MAX` timer; a timer names its clock (8). The mismatch refusal at `start`, and at `is_over` and `remaining`, for every ordered pair of different clocks (2). Real clocks: each reading carries its clock, 1000 repeated reads of uptime and of continuous time never go backwards, a 5 ms real sleep moves both clocks by at least 5 ms with a gap under 50 ms, and the wall reading is a plausible Unix time (4).
- `checks/tests/real_tree.rs`: the three `clock:` tags move from the host outline list (at least one stub each)
  to `FILLED_TAGS`, a new test that each filled tag stays registered under its stage and counts no stub. Later
  Stage 3a items move their tags the same way.

### Decisions for the owner

- **`libc` is added to `host`, for macOS only.** It is the standard binding for `clock_gettime` and was already in the lockfile. The alternative is declaring `extern "C"` for `clock_gettime` by hand (no dependency, but the constants and the `timespec` layout become ours to keep right), or `mach_absolute_time` with its timebase (the same uptime clock, plus a conversion).
- **`clock_gettime` with `CLOCK_UPTIME_RAW`, not `mach_absolute_time`.** The table names both for uptime and they are the same clock. `clock_gettime` gives nanoseconds directly with no timebase ratio, and both macOS clocks go through one function. The alternative is `mach_absolute_time` times the timebase, which is what the table's first name says.
- **Other systems compile but are not real.** Off macOS the uptime and the continuous clock come from one `Instant`, so `sleep_gap` is always zero there. The plan ships macOS first; the alternative is `compile_error!` off macOS, or Linux's `CLOCK_BOOTTIME` and `CLOCK_MONOTONIC` pair (a real second platform, its own item).
- **A reading before a timer's start is "no time passed", and a mismatch is an error, not a panic.** Setting the wall clock back must not end a limit-reset timer early or crash the daemon. The alternative is to treat a backwards reading as an error of its own.
- **`sleep_gap` returns the raw gap, with no threshold.** The 2 s rule of 0.13 (a gap over two sampler ticks is a sleep interval) belongs to the sampler, which already has `SLEEP_THRESHOLD`. The alternative is to apply it here and return `Option<Duration>`.
- **The fakes live in the test file.** `host/tests/clock.rs` holds `FakeClocks`, since no other item needs one yet. The alternative is a `pub` fake in `collide_host::clock` or in `host/tests/common`, which the sampler and daemon tests would share; move it when the second user arrives.
- Owner decisions (2026-11-15): the recommendations above are accepted as built.

### Not done, by design

- The sampler tick that reads a `ClockPair` each second, records a sleep interval over 2 s and subtracts it from active time: Stage 3b (tag `sampler: tick and sleep`, 0.13; `host/src/sampler.rs`).
- The work timeouts, the cache countdown and the limit reset that use these timers: the engine (Stage 8) and the usage and cache items of Stage 3b.
- The owner's real sleep test (`sleep-check.sh`): Stage 3b.
- A real clock for Linux or Windows: not before a second platform is a target.
