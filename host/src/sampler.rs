//! The sampler (0.13, Metrics; Stage 3b for the tick, Stage 13 for the resource sources). One tick a second reads the
//! clocks and records resource samples tied to the run's spans.
//!
//! **Sleep is not work.** On macOS the uptime clock stops during sleep and the continuous clock does not, so on each
//! tick `slept = Δcontinuous − Δuptime` ([`crate::clock::sleep_gap`]). A gap over [`SLEEP_THRESHOLD`] (two ticks) is
//! recorded as a sleep interval and counted as waiting, not active time. The tick takes its clocks as a value
//! ([`crate::clock::Clocks`]), so a test injects a gap and never waits.
//!
//! The resource sources are measured by S8 and cost 0.11% of one core at 1 s for a run's tree:
//!
//! | source | how | stage |
//! |---|---|---|
//! | process tree | `proc_pid_rusage` per pid, CPU in mach time units converted with `mach_timebase_info`, never a constant | 13 |
//! | network | one streamed `nettop`, read through a pty, stdin held open; a process's total is the sum of the stream's deltas | 13 |
//! | GPU | `ioreg` device utilization and memory, every [`GPU_POLL`], only when a run declares a local model | 13 |
//! | containers | the Docker API one-shot, never the CLI | 13 |
//!
//! | tag | function | what it does |
//! |---|---|---|
//! | `sampler: process tree` | [`sample_process_tree`] | CPU, memory and disk I/O of every process of the tree |
//! | `sampler: network` | [`NetworkStream`] | per-process network deltas from the streamed `nettop` |
//! | `sampler: gpu` | [`poll_gpu`] | device utilization and memory |
//! | `sampler: containers` | [`sample_containers`] | CPU, memory and I/O of the run's lane containers |

use crate::clock::{ClockPair, Clocks, sleep_gap};
use std::io;
use std::time::Duration;

/// The tick: one second (S8).
pub const TICK: Duration = Duration::from_secs(1);

/// A gap over this is a sleep interval: two ticks (0.13).
pub const SLEEP_THRESHOLD: Duration = Duration::from_secs(2);

/// How often the GPU is polled while a run declares a local model: each `ioreg` call costs about 25 ms of CPU.
pub const GPU_POLL: Duration = Duration::from_secs(5);

/// A stretch the machine slept, as the tick measured it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SleepInterval {
    /// How long the machine slept.
    pub length: Duration,
}

/// What one tick found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tick {
    /// The clocks at this tick, which the next tick measures from.
    pub now: ClockPair,
    /// A sleep over [`SLEEP_THRESHOLD`] since the last tick, if there was one.
    pub sleep: Option<SleepInterval>,
}

/// The sampler's memory between ticks: the clocks at the last one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sampler {
    last: ClockPair,
}

impl Sampler {
    /// A sampler that measures its first tick from `first`.
    pub fn new(first: ClockPair) -> Self {
        Self { last: first }
    }

    /// The clocks at the last tick.
    pub fn last(&self) -> ClockPair {
        self.last
    }

    /// Reads both clocks, compares them with the last tick's and returns what changed. The sampler's `last` becomes
    /// this tick's clocks.
    pub fn tick(&mut self, clocks: &dyn Clocks) -> Tick {
        let now = ClockPair::read(clocks);
        let gap = sleep_gap(&self.last, &now);
        self.last = now;
        Tick {
            now,
            sleep: (gap > SLEEP_THRESHOLD).then_some(SleepInterval { length: gap }),
        }
    }
}

/// One process's resources at one tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessSample {
    pub pid: u32,
    /// CPU time used so far, in nanoseconds (converted from mach time units).
    pub cpu_nanos: u64,
    pub resident_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
}

/// The resources of every process in the tree of `root_pid`, as [`crate::stop::find_tree`] finds it.
pub fn sample_process_tree(root_pid: u32) -> io::Result<Vec<ProcessSample>> {
    let _ = root_pid;
    todo!("sampler: process tree")
}

/// Network bytes a process moved since the last read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NetworkDelta {
    pub pid: u32,
    /// `nettop` cuts a name to 16 characters.
    pub name: String,
    pub bytes_in: u64,
    pub bytes_out: u64,
}

/// The one streamed `nettop`, which must run for the whole span, because a closed socket's counters vanish.
#[derive(Debug)]
pub struct NetworkStream {
    _private: (),
}

impl NetworkStream {
    /// Starts `nettop` on a pty, with stdin held open: at end of file it spins at 56 to 134% of a core.
    pub fn start() -> io::Result<NetworkStream> {
        todo!("sampler: network")
    }

    /// The deltas the stream wrote since the last call.
    pub fn deltas(&mut self) -> io::Result<Vec<NetworkDelta>> {
        todo!("sampler: network")
    }
}

/// The device's GPU use at one poll.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GpuSample {
    /// Device utilization, in percent.
    pub utilization_percent: u8,
    pub memory_bytes: u64,
}

/// Reads the device's utilization and memory. The caller polls it every [`GPU_POLL`], and only while a run in the tree
/// declares a local model.
pub fn poll_gpu() -> io::Result<GpuSample> {
    todo!("sampler: gpu")
}

/// One lane container's resources at one tick.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerSample {
    pub id: String,
    pub cpu_nanos: u64,
    pub memory_bytes: u64,
    pub disk_read_bytes: u64,
    pub disk_write_bytes: u64,
}

/// The resources of the containers named by `ids`, which a pack's `metrics.processes` hook returns, from the Docker
/// API one-shot (8 to 14 ms a call; the CLI takes 2 s).
pub fn sample_containers(ids: &[String]) -> io::Result<Vec<ContainerSample>> {
    let _ = ids;
    todo!("sampler: containers")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::clock::{Clock, Reading};

    struct Fixed;

    impl Clocks for Fixed {
        fn read(&self, clock: Clock) -> Reading {
            Reading { clock, nanos: 1 }
        }
    }

    #[test]
    fn a_sleep_is_a_gap_over_two_ticks() {
        assert_eq!(SLEEP_THRESHOLD, TICK * 2);
    }

    #[test]
    fn a_new_sampler_measures_from_the_clocks_it_was_given() {
        let first = ClockPair::read(&Fixed);
        assert_eq!(Sampler::new(first).last(), first);
    }

    /// Clocks a test sets: the uptime and the continuous reading, in nanoseconds.
    struct Set(std::cell::Cell<(u64, u64)>);

    impl Clocks for Set {
        fn read(&self, clock: Clock) -> Reading {
            let (uptime, continuous) = self.0.get();
            let nanos = match clock {
                Clock::Uptime => uptime,
                _ => continuous,
            };
            Reading { clock, nanos }
        }
    }

    const SECOND: u64 = 1_000_000_000;

    /// One tick after the clocks moved from `(0, 0)` to `(uptime, continuous)` seconds-in-nanoseconds.
    fn tick_after(uptime: u64, continuous: u64) -> (Tick, Sampler) {
        let clocks = Set(std::cell::Cell::new((0, 0)));
        let mut sampler = Sampler::new(ClockPair::read(&clocks));
        clocks.0.set((uptime, continuous));
        (sampler.tick(&clocks), sampler)
    }

    #[test]
    fn an_awake_tick_finds_no_sleep() {
        assert_eq!(tick_after(SECOND, SECOND).0.sleep, None);
    }

    #[test]
    fn a_gap_under_the_threshold_is_no_sleep() {
        assert_eq!(tick_after(SECOND, 2 * SECOND).0.sleep, None);
    }

    #[test]
    fn a_gap_of_exactly_the_threshold_is_no_sleep() {
        assert_eq!(tick_after(0, 2 * SECOND).0.sleep, None);
    }

    #[test]
    fn a_gap_over_the_threshold_is_a_sleep_of_that_length() {
        let sleep = tick_after(SECOND, 11 * SECOND).0.sleep;
        assert_eq!(
            sleep,
            Some(SleepInterval {
                length: Duration::from_secs(10)
            })
        );
    }

    #[test]
    fn clocks_that_went_backwards_find_no_sleep() {
        let clocks = Set(std::cell::Cell::new((10 * SECOND, 10 * SECOND)));
        let mut sampler = Sampler::new(ClockPair::read(&clocks));
        clocks.0.set((SECOND, SECOND));
        assert_eq!(sampler.tick(&clocks).sleep, None);
    }

    #[test]
    fn a_tick_makes_its_clocks_the_next_ones_start() {
        let (tick, sampler) = tick_after(SECOND, 11 * SECOND);
        assert_eq!(sampler.last(), tick.now);
    }

    #[test]
    fn the_sleep_is_counted_once() {
        let clocks = Set(std::cell::Cell::new((0, 0)));
        let mut sampler = Sampler::new(ClockPair::read(&clocks));
        clocks.0.set((SECOND, 11 * SECOND));
        assert!(sampler.tick(&clocks).sleep.is_some());
        clocks.0.set((2 * SECOND, 12 * SECOND));
        assert_eq!(sampler.tick(&clocks).sleep, None);
    }

    #[test]
    #[should_panic(expected = "sampler: process tree")]
    fn sampling_the_process_tree_is_a_stub_tagged_for_stage_13() {
        let _ = sample_process_tree(1);
    }

    #[test]
    #[should_panic(expected = "sampler: network")]
    fn the_network_stream_is_a_stub_tagged_for_stage_13() {
        let _ = NetworkStream::start();
    }

    #[test]
    #[should_panic(expected = "sampler: gpu")]
    fn polling_the_gpu_is_a_stub_tagged_for_stage_13() {
        let _ = poll_gpu();
    }

    #[test]
    #[should_panic(expected = "sampler: containers")]
    fn sampling_containers_is_a_stub_tagged_for_stage_13() {
        let _ = sample_containers(&[]);
    }
}
