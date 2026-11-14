---
id: sp-0010-bn
alias: S8
kind: spike
status: results
title: "S8: metrics sources"
owners: []
links: []
---

# S8: metrics sources

Plan reference: Stage 1 item "S8 metrics sources", and 0.13 Metrics (the Sources table, "Sleep is not work",
"Volume and retention", "Claude Code's own telemetry"), 0.5 Hook protocol (report files), and the tool-call time
breakdown in the run view (part K).

Every figure is labelled [M] measured here, [I] inferred from measurements, or [U] not measured. Scripts and raw
outputs are in this folder. `run-all.sh` reproduces everything (see "Reproduction").

## Versions and conditions

- Machine: MacBook with Apple M2 Pro, 12 cores, 16 GB, macOS 26.3.1 (arm64). Date: 2026-11-14.
- Tools: Rust 1.98.0 (sampler, std only, no crates), Python 3.9.6 (stdlib only), node 24.18.0, Docker Desktop 29.9.2,
  Ollama 0.40.15 with `llama3.1:8b`, SQLite 3.51.0, Claude Code 2.1.325 (the plan's 0.13 cites 2.1.323).
- The machine was busy throughout: load average 7 to 33 at the start of runs, from other sessions. This is realistic:
  the load average recorded in Orchard's own timing log has a median of 10.5 (see "Orchard census"). Figures taken on a
  busy machine are upper bounds for CPU cost. Where a number moved with load, the range is given.
- The workload is synthetic, with the process shape of an Orchard iteration run (see "Orchard census"). A real run
  was not measured by the spike. `measure-attach.sh` is prepared for it (see "Left unmeasured").
- Claude spend: about $0.03 for the nine recorded `claude -p` runs (haiku, $0.0010 to $0.0059 each) [M]. A few earlier
  probe runs were not tallied; the total is well under $0.25 [U for the exact figure].
- Nothing was installed. Nothing outside this folder and the session scratch space was written. `claude -p` reads its
  own credentials as usual. One container and a few servers were started and removed (checked: no container with the
  spike's label remains, no process of the spike is running).

## Summary

**Chosen interval: 1 s.** The sampler (std-only Rust over libproc) costs 0.11% of one core at 1 s, 0.37% at 250 ms,
0.91% at 100 ms and 1.80% at 50 ms (mean of three runs) [M]. Only 1 s and 250 ms leave a wide margin under the 1%
criterion; 100 ms is just under it and 50 ms is over. Sampling the whole machine (360 processes) at 1 s costs 0.24%
[M]. The success criterion "S8 names an interval whose overhead is below 1% of one core, and states each resource's
granularity and what it cannot attribute" is met: see the table in "Granularity and what cannot be attributed".

Answers in one place:

- **Short processes are mostly missed, at every interval.** Of a workload in which 54% of processes live under 10 ms,
  1 s sampling sees 0% of those, 4% of those living 10-100 ms, and all processes living over 1 s. Even 50 ms sampling
  sees only 10% and 47% of those two classes [M]. So the exact totals must come from the shim's `wait4` exit totals
  (0.13), as the plan already says. Sampling gives the time series for processes that last more than a second.
- **Per-process disk, network and GPU are all available without root**, with these limits: disk reads count only
  uncached reads; network counters vanish when a socket closes, so `nettop` must be streamed; GPU time is per client
  process and the counter dies with the process. Details below.
- **`nettop` spins at 56-130% of a core when its stdin is `/dev/null`.** With an open pipe on stdin it costs about 1%.
  This is the one trap found. Spawn it with `stdin=PIPE` and keep the pipe open [M].
- **Telemetry traces are exact where stream-json is not, but only for queued parallel calls and permission waits.**
  For unqueued calls the stream is within 150 ms. For a queued parallel call it was wrong by 3.2-3.3 s [M].
- **Breakdown choices** (proposals): model time from telemetry spans, else from the stream with a 1 s gap threshold;
  a subagent's calls nested under it with the Agent row showing self time; share denominator is the sum of the
  table's rows, with exclusive wall-clock shares in a separate column. See "Breakdown timing choices".
- **Store growth** is 34 bytes per sample row [M]: 0.6 MB per hour for a 5-process tree at 1 s, 6 MB per hour for 50.
- **Report files:** a 50 ms poll tails 50,000 lines per second with no loss, p99 lag 52-55 ms. Proposed caps: 4 KB
  per line (unchanged), 100 lines per second sustained (burst 1000) and 16 MiB per file.
- **Real Orchard runs (three, attached part-way through):** the 1 s sampler missed 33-73% of the processes the
  50 ms reference saw, yet caught 93-100% of their CPU time; the processes it never saw used 0-4.3% [M]. On real runs
  the misses are many but cheap. It cost 0.08-0.09% of a core [M].
- **A lane run's containers did more work than its host processes:** 612 s of CPU in three lane containers against
  551 s in the host tree, so 53% of run C's CPU is invisible to a tree-only sampler [M]. Count lane containers always.
  See "Real Orchard runs".
- **Not run:** the sleep test (owner step, `sleep-check.sh`; a watcher waited 60 minutes on 2026-11-14 and saw no
  sleep).

## Sampler overhead and missed processes

Method [M]. The synthetic workload runs a fixed 7 steps (183 processes per run, three levels deep: launcher, shell,
step, children). Five samplers (1000, 500, 250, 100, 50 ms) attach to the same run at once, so each sees exactly the
same processes. Truth is `wait4` on every child the workload starts (CPU, lifetime, exit time). Each sampler's own CPU
is read twice, from its own `getrusage` and from `wait4` on the sampler process; the two agree within 0.03 points.
Three runs, 549 processes in all. The samplers share the machine, so each figure is slightly high.

CPU of the sampler, mean of three runs (range in brackets):

| interval | CPU, % of one core | tick work p50 | tick work p99 |
|---|---|---|---|
| 1000 ms | 0.110 (0.103-0.120) | 1.2 ms | 9-15 ms |
| 500 ms | 0.193 (0.182-0.211) | 1.0 ms | 4-6 ms |
| 250 ms | 0.373 (0.359-0.391) | 1.0 ms | 4-5 ms |
| 100 ms | 0.905 (0.882-0.920) | 0.9 ms | 4-7 ms |
| 50 ms | 1.801 (1.751-1.879) | 0.9 ms | 4-5 ms |

Cost is linear in ticks per second: about 1 ms of CPU per tick for this tree. Whole machine (`--all`, about 360
processes) [M]: 0.24% at 1 s (2.0 ms per tick: 0.12 ms listing pids, 0.62 ms `PROC_PIDTBSDINFO`, 1.70 ms
`proc_pid_rusage`, 0.07 ms writing) and 0.89% at 250 ms. The rusage call is about 4.7 us per process.

Share of the workload's processes seen at least once, by lifetime (spawn to `wait4` return):

| lifetime | procs | 1000 ms | 500 ms | 250 ms | 100 ms | 50 ms |
|---|---|---|---|---|---|---|
| under 10 ms | 297 | 0.0% | 1.3% | 2.0% | 3.4% | 9.8% |
| 10-100 ms | 118 | 4.2% | 6.8% | 11.0% | 31.4% | 46.6% |
| 0.1-0.25 s | 20 | 5.0% | 20.0% | 55.0% | 100% | 100% |
| 0.25-0.5 s | 9 | 55.6% | 77.8% | 100% | 100% | 100% |
| 0.5-1 s | 42 | 69.0% | 100% | 100% | 100% | 100% |
| 1-5 s | 63 | 100% | 100% | 100% | 100% | 100% |
| all | 549 | 18.8% | 23.3% | 26.2% | 33.0% | 39.7% |

Share of CPU time the samples captured (the part of a process's CPU that falls before its last sample): 60% at 1 s,
75% at 500 ms, 85% at 250 ms, 93% at 100 ms, 95% at 50 ms. The rest is CPU spent in processes never seen and in the
last interval before exit. For the 63 processes over 1 s, 72% at 1 s [M].

Reading:

- A miss is a process, not necessarily CPU. At 1 s, 40% of this workload's CPU time falls outside the samples, because
  its CPU is spread over many 0.1-1 s node and compiler processes. For a workload of long-running processes the
  share outside the samples is smaller.
- Shortening the interval does not fix this. Going from 1 s to 50 ms costs 16 times the CPU and still misses 60% of
  processes. `wait4` exit totals (shim) are exact for every child, so sampling's job is only the time series.
- Exited-between-ticks CPU can be recovered for children the sampler's own process reaps; it cannot for others.
  `proc_pid_rusage` does work on a zombie (not yet reaped) process and returns its final CPU and exit time [M], but the
  window is the parent's `wait4` latency, normally microseconds. Not usable as a fix.
- Event-based exit capture (kqueue `NOTE_EXIT`) needs a pid to register on, so a process that is born and dies between
  two ticks is still missed [I; not run].

**Recommendation.** 1 s for the live time series of the run's tree (not the whole machine). Offer 250 ms as a
"detailed" setting (0.37%). Do not go to 100 ms or below. State in the UI that per-process rows are samples and that
the exact totals come from the shim.

## Per-process disk I/O and the mach-time units (`02-diskio.py`)

A child does known I/O; the parent reads `proc_pid_rusage` (`RUSAGE_INFO_V4`, 296 bytes) while it runs and compares
with `wait4`.

| case | expected bytes | `diskio_byteswritten` | `diskio_bytesread` | `logical_writes` | `ru_oublock`/`ru_inblock` |
|---|---|---|---|---|---|
| write 128 MiB, fsync | 134,217,728 | 134,217,728 | 299,008 | 135,385,088 | 0 / 0 |
| write 64 MiB, no fsync, delete | 67,108,864 | 67,108,864 | 212,992 | 67,637,248 | 0 / 0 |
| read 128 MiB, cached | 134,217,728 | 0 | 0 | 0 | 0 / 0 |
| read 128 MiB, `F_NOCACHE` | 134,217,728 | 0 | 134,217,728 | 0 | 0 / 0 |
| 1000 small files, 2,048,000 bytes | 2,048,000 | 2,048,000 | 0 | 8,470,528 | 0 / 0 |

[M] Findings:

- Bytes written are exact, even without fsync and even for a file deleted before it flushed. `logical_writes` is page
  rounded (4x for the small files) and is the wrong field to show.
- Reads count only what reaches the disk. A cached read counts 0. Builds that re-read warm files will show almost no
  read bytes. That is a blind spot to state, not a bug.
- The counters are live: the 20 ms polls saw them move 2 to 4 times during each case that does I/O (0 for the cached
  read, which has nothing to count), so sampling at 1 s gets a time series, not only a total at exit.
- `ru_oublock` and `ru_inblock` from `wait4` are always 0 on macOS. The shim's exit totals cannot give disk bytes.
  Disk bytes need the sampler (or one final `proc_pid_rusage` read before reaping).
- Units: `user_time` and `system_time` are mach ticks. On this machine `mach_timebase_info` is 125/3 (1 tick =
  41.67 ns). Read as nanoseconds the child's 0.511 s would show as 0.012 s (a 42x error); converted it matches
  `wait4`'s 0.513 s. The sampler converts with `mach_timebase_info`, never a constant (an Intel Mac is 1/1).

## Network per process (`03-nettop.py`)

`nettop -P -x -J bytes_in,bytes_out` (no root). Parsed by header, not column position, because the column layout
depends on option order (with `-P -L n -x -J` the `time` column is absent; with `-x -J ... -P -L n` it comes first).

[M] Findings:

- One-shot `nettop -P -L 1`: 28-46 ms wall, about 30 ms CPU, 50-55 processes listed, 2 KB of CSV, 0.1 ms to parse.
  At a 1 s poll that is about 3% of a core, too much. Stream instead.
- Streaming `nettop -P -L 0 -s 1`: about 1% of a core (1.3-1.6% in three runs; 0.7-1.1% at `-s 5`).
- **Trap:** with stdin at end-of-file (for example `stdin=/dev/null`) nettop spins at 56-134% of a core. With stdin an
  open pipe that is never written, the cost is the 1%. A reader that spawns nettop with `DEVNULL` pays a core.
- Delivery: over a pipe the output is block-buffered (4 KB), so the first 1 s block arrived after 7-8 s. Over a pty it
  arrived every second. For a live view, use a pty, or accept batched delivery and stamp each block by its own `time`
  column (present only in the `-x -J ... -P` order).
- Counters are the sum over currently open sockets. A 64 MiB loopback transfer showed 67,108,993 bytes in the client row
  and 83 in the server row while the connection was open (loopback bytes are counted, once per side). After the client
  exited its row was gone and the server's showed 0. A one-shot snapshot after close shows nothing.
- A continuous stream does capture short flows: 20 clients of 1 MiB each, connection open about 0.1 s, were all listed
  by a 1 s stream, and the bytes seen were 100% (20,974,080 of 20,971,520, the extra being protocol bytes). So per
  process totals are the sum of the deltas of a running stream, which means nettop must run for the whole span.
- Rows are `name.pid`. The name is the kernel's, cut to 16 characters ("Discord Helper "). With `-P` there are no
  per-connection rows: no remote address, so it cannot say whether bytes went to a model API or a package registry.

**Recommendation.** Run one streaming nettop per host while any `full` run is live, with `stdin=PIPE` held open,
read through a pty, and attribute rows to a run by the pids in its tree. Network is process-level only.

## GPU (`04-gpu.py`)

[M] Two sources, both without root, both measured under Ollama `llama3.1:8b` (hosted run, GPU 100%):

- `ioreg -r -c AGXDeviceUserClient -a` gives one entry per GPU client with `AppUsage[].accumulatedGPUTime` in
  nanoseconds, and `IOUserClientCreator` = "pid N, name". 54 clients were listed. Cost: 30 ms wall, 22 ms CPU, plist
  parse 3 ms per call.
- `ioreg -r -d 1 -c IOAccelerator -a` gives `PerformanceStatistics`: "Device Utilization %" (also Renderer and Tiler),
  allocated and in-use memory. Cost: 35 ms wall, 26 ms CPU, parse 6 ms.

Under a generation (129 tokens in 4.97 s, 26 tok/s): the runner `llama-server` (pid 63305; the process with the largest
GPU delta, not the Ollama app) gained 3.60 s of GPU time over the 14.1 s request. Per-second shares while generating
were 0.37, 0.75, 0.71, 0.70, 0.73 (sum 3.26 s; the first and last partial seconds account for the rest), and device
utilization was 92-94% on those seconds, 0-19% around them. Idle, the display (`WindowServer`) and an editor helper
account for 0.05-0.15 s of GPU per second each, so a "GPU time" column on an idle desktop is not zero.

Limits [M]: the counter is cumulative busy time of the client, so a delta needs two reads while the process lives; when
it exits its counter goes with it. Time is per client process, not per model or request. Several clients overlap on one
GPU, so shares of one second can sum to more than one. Device utilization is whole device, never per process. Memory is
device-level only here.

**Recommendation.** The two calls cost 48 ms of CPU together: 4.8% of a core at 1 s, so too much. Poll both at 5 s
(about 1%; 10 s halves it), and only when a process in the run's tree or its pack declares a local model. In v0,
Claude's inference is remote, so GPU is a `full` setting for the first local-model adapter, as 0.13 already says. Show
per-process GPU time as "GPU busy seconds" with the device utilization beside it.

## Docker and detached processes (`05-docker-detached.py`)

Setup: a lane shaped like Orchard's: a labelled container (node server with a database-like load) and a host server
started by a short-lived launcher, so the server is reparented to launchd. The prototype `metrics.processes` hook is
`hooks/metrics-processes.py`; it reads the lane record and prints identities only.

Hook output [M, 43-55 ms]:

    {"v": 1, "containers": [{"id": "<id>", "role": "database-or-service"}], "processes": [{"pid": 50665, "role": "server"}]}

[M] Findings:

- Docker API over the unix socket, `GET /containers/<id>/stats?stream=false&one-shot=true`: 8-14 ms, 1.8 KB, no
  previous-CPU fields, so CPU is the delta of `cpu_total_ns` between two reads. With `one-shot=false` the call takes
  1.0-2.0 s (the daemon waits for a second sample). The `docker stats --no-stream` CLI takes 2 s wall and 30 ms CPU:
  do not shell out to it.
- Fields: CPU total in ns, memory usage, `blkio` read and write bytes, network rx and tx per interface, pid count. The
  `read` timestamps advance 1.00 s per call: the daemon samples once a second whatever the poll rate, so poll at 1 s or
  slower. Ten polls cost 15-16 ms of client CPU.
- A 9.3 s window showed container CPU 19-24% of a core (the workload burns about 150 ms per second), memory 61-84 MiB,
  block write +37.7 MB, pids 11.
- Container processes are invisible on the host. Host processes that appeared while the container ran were the
  harness's own (`sleep`, `node` servers started by the workload, `docker` client calls, an unrelated `caffeinate`);
none was the container's `node`. Only the Docker API sees inside.
- The detached host server had ppid 1, so a walk of the launcher's process tree misses it. Given its pid by the hook,
  `proc_pid_rusage` read it directly (cpu +362 to +499 ms over five 100 ms requests; footprint 28.5-28.8 MiB).
- Hook shape as prototyped (`v`, `containers`, `processes`, optional `start_usec` for pid reuse) gives the data both
  readers need. The hook never reports numbers, only identities.

Cannot attribute: processes inside a container, one from another (the API is per container); a process whose pid was
never reported, or that a pack starts outside its lane record.

## Sleep detection (`lib/sleepclock.py`, `sleep-check.sh`)

[M] The two clocks and the formula `slept = Δ(mach_continuous_time) − Δ(mach_absolute_time)`:

- Awake for 5 s: uptime +5.0046 s, continuous +5.0048 s, wall +5.0049 s; the formula gives 0.19 ms (0.05 ms in another
  run). One reading of both clocks plus wall time costs 1.0 us.
- Since boot (8 days 15 h), the continuous clock minus the uptime clock is 30,364 s (8.43 h), which is this machine's
  total sleep; the continuous clock matches wall time since `kern.boottime` within 9 s. The OS's own
  `kern.sleeptime` to `kern.waketime` gives the last sleep as 16.7 s, inside the total. So the two clocks do diverge by
  real sleep [M], but this is not a mid-run test.
- Injected readings (math only): a 60 s sleep gives "slept 60.0 s"; a 40 ms late tick gives 0, below a 2 s threshold
  [M, math only].
- Dark wake (maintenance with the lid closed) runs the CPU, so the uptime clock advances through it and it is not
  counted as sleep [I].

Not done: putting the machine to sleep mid-run (forbidden for the spike) [U]. `sleep-check.sh` is the owner's two
minute step (see "Left unmeasured").

**Recommendation.** Tick at the sampler interval, compute the formula on each tick, and record a sleep interval when it
exceeds 2 s (two sampler ticks at 1 s; the late-tick noise measured here is under 0.2 ms awake, but a busy machine
delays ticks by tens of ms, so the threshold must be well above that). The span's active time subtracts it.

## Store growth (`07-store-growth.py`)

Sampler rows written to SQLite one transaction per tick: a process table (pid, start time, parent, name) and a sample
table `WITHOUT ROWID` keyed by (process, time) with CPU, resident size, footprint, disk read and written bytes.

Real recordings: 23% (1 s) and 43% (250 ms) of rows repeat the CPU and disk counters of the previous tick, so a writer
that skips unchanged rows saves that share [M]. Synthetic hour-long recordings (3600 ticks, per-row figure from the
growth over the second half, after `VACUUM`) [M]:

| live processes | 1000 ms, MB per hour | 250 ms, MB per hour | bytes per row | insert CPU per tick (1 s) |
|---|---|---|---|---|
| 5 | 0.62 | 2.49 | 34.6 | 0.8 ms |
| 20 | 2.46 | 9.80 | 34.1 | 1.3 ms |
| 50 | 6.15 | 24.48 | 34.2 | 1.9 ms |

A skip-unchanged writer at the synthetic 70% change rate writes about 30% fewer rows. Retention [I]: a 20-process
tree at 1 s for 4 hours a day for the plan's default 90 days is about 0.9 GB; a 50-process tree for 8 hours a day is
about 4.4 GB. The sample table is the large one; thinning rows older than a week to 10 s would cut it by 10x.

## Report-file tail lag and caps (`08-report-tail.py`)

A writer appends 149-byte JSON lines (one `os.write` each, `O_APPEND`) at a fixed rate; a tailer reads them back by
polling at 250 ms or 50 ms, or by waking on kqueue vnode events. Lag is the read time minus the line's own `t` [M].
Python parses every line, so the CPU is an upper bound for Collide's. Five seconds per cell; no line was lost or
torn in any cell (every `seq` arrived once, in order).

| lines per second | tailer | lag p50 | lag p99 | lag max | tailer CPU |
|---|---|---|---|---|---|
| 100 | poll 250 ms | 131 ms | 252 ms | 260 ms | 0.08% |
| 100 | poll 50 ms | 26 ms | 57 ms | 68 ms | 0.26% |
| 100 | kqueue | 0 ms | 1 ms | 1 ms | 0.82% |
| 1,000 | poll 50 ms | 26 ms | 53 ms | 61 ms | 0.47% |
| 1,000 | kqueue | 0 ms | 0 ms | 3 ms | 2.90% |
| 10,000 | poll 50 ms | 25 ms | 53 ms | 55 ms | 2.82% |
| 10,000 | kqueue | 0 ms | 4 ms | 13 ms | 7.17% |
| 50,000 | poll 50 ms | 22 ms | 52 ms | 55 ms | 10.98% |
| 50,000 | kqueue | 0 ms | 1 ms | 19 ms | 21.04% |

A line written in two pieces is read without its newline first; the tailer must hold the partial tail until the
newline (the test shows the first read ends mid-line). Lag is bounded by the poll interval whatever the rate.

Options and trade-offs:

- Poll every 50 ms: simple, boring, p99 lag about the poll interval, CPU grows with line rate (about 0.2% per 1000
  lines per second in Python). Recommended while a run is live.
- kqueue: zero lag, but more CPU at every rate (a wake per write: 2-6x poll) and one watcher per file. Worth it only if
  the UI needs sub-50 ms report lines, which nothing in the plan does.
- Poll 250 ms: cheapest, p99 lag 250 ms. Acceptable for background runs.

**Proposed caps (0.5 says "each span's file has a size and rate cap, set by S8")** [I]. The tailer is not the limit:
it kept up at 50,000 lines per second. The limits protect the store and the view. 1000 lines per second is 150 KB per
second, 540 MB per hour of raw lines.

- Line: 4 KB, as in the plan. Longer lines are counted and dropped.
- Rate: 100 lines per second sustained per file, with a burst of 1000 (a token bucket). About 54 MB per hour at the
  sustained rate. Lines above it are counted on the span, not stored, per the plan's "counted, not hidden".
- Size: 16 MiB per file (about 110,000 lines of this size). At the cap the span is marked "report truncated", and the
  tailer keeps counting.
- Tail by polling every 50 ms, and hold a partial last line.

## Claude Code telemetry against stream-json (`telemetry/`)

Method [M]. Nine real `claude -p` runs on the haiku model (Claude Code 2.1.325), each with the exporters pointed at a
local OTLP receiver (`http/json`, logs, metrics and traces, export interval 1 s): 3 serial, 2 with two parallel Bash
calls, 2 with one subagent, 2 with two parallel subagents. Traces need `CLAUDE_CODE_ENHANCED_TELEMETRY_BETA=1`
(`OTEL_TRACES_EXPORTER=otlp`); it is a beta flag. The stream-json output was stamped on arrival with
`--include-partial-messages`. The commands inside tool calls are `perl` scripts that print their own start and end
wall-clock time, the ground truth. The calling session's `CLAUDE*`, `OTEL_*` and `ANTHROPIC_*` variables were removed
first. 15 Bash calls carry a command truth; 30 tool calls in all carry telemetry.

What telemetry gives:

- Log events: `tool_result` (with `duration_ms`), `tool_decision`, `api_request`, and others.
- Trace spans: `claude_code.interaction` (one per prompt, and one per background-agent notification),
  `claude_code.llm_request` (model, tokens, `ttft_ms`, `llm_request.context` of `interaction` or `tool`),
  `claude_code.tool` with children `tool.blocked_on_user` and `tool.execution`. Subagent spans carry `agent_id` and
  their parent id is the Agent tool's `tool.execution` span, so nesting is exact.

Findings [M]:

- **`tool_result.duration_ms` equals the `tool.execution` span** to within 0-6 ms (n=30, median -1 ms). The log
  record's time is the span end plus 0-26 ms (p50 2 ms). Records reached the receiver 47-1217 ms after their time
  (p50 1005 ms at a 1 s export interval). Timings are exact; delivery is up to a second late.
- **Both include the spawn.** The span exceeds the command's own printed time by 33-150 ms for later calls (median about
  120 ms). The first Bash call of a session carries about 0.9-1.4 s of start-up (command 500 ms, span 1372-1521 ms in
  all three serial runs; shell start). So "tool time" is the whole call, as the plan's 0.13 says.
- **Stream-json timing, unqueued calls** (n=28): the `tool_use` line arrives 3-136 ms (p50 34) before the execution
  span starts (the permission check, 1-60 ms of `tool.blocked_on_user`, falls in between); the `tool_result` line
  arrives 3-59 ms (p50 13) after the span ends. Duration (use to result) overestimates the span by 8-149 ms (p50 54).
  Good enough for a table of seconds; poor for calls of tens of ms (a 36 ms Read showed 127 ms).
- **Stream-json timing, queued parallel calls** (n=2): Claude Code runs Bash calls one at a time even when two are
  requested in one message. The second call's `tool_use` line arrived 3.2-3.3 s before its execution began, and the
  stream duration was 6.3-6.5 s against a true 3.06-3.16 s: wrong by the wait for the first call. The spans give
  each call's own time, and the wait shows as the gap before the span.
- **Permission and hook waits** are spans (`tool.blocked_on_user`). In stream-json they are inside the call's
  duration with nothing to separate them. In these runs the permission was automatic (1-60 ms); a human wait is
  untested [U] but the span would carry it.
- **Model time:** a response's `message_start` arrives after the first token, so `message_start` to `message_stop`
  undershoots the `llm_request` span by 439-1009 ms (the span's `ttft_ms`, p50 577 ms, is the missing part; the
  stream's `message_start` arrival equals span start plus `ttft_ms` within 1-37 ms). Using "previous input line to
  `message_stop`" instead is within -16 to +119 ms (median about 20 ms) for requests that follow a tool result, and
+100-270 ms for the first request, which includes start-up. For a turn started by a background-agent notification there
is no input line, and the estimate is wrong by 1.3-2.6 s.
- **Subagents.** A foreground Agent call's `tool.execution` span covers its whole run (9.56 s and 5.31 s in the two
  runs, equal to the nested model and tool spans plus 0.2-0.35 s of its own). A background Agent call returns in 6-44
  ms and its nested spans extend past it, under later `interaction` spans. Subagent messages in the stream carry
  `parent_tool_use_id`, so nesting is available there, but not their timing.
- Spend: $0.0010-$0.0059 per run, $0.026 for nine.

**Answer to the plan's question** ("does it give tool timings more precise than tailing stream-json?"): yes, for four
things: calls queued behind another call (3.2 s error), permission and hook waits, model time (exact, with
time-to-first-token), and subagent nesting with timing. For serial calls the stream is within 150 ms. The plan's
rule "headless phases keep the stream unless S8 shows better timings" is met for headless phases that run parallel
tool calls or care about waits.

**Recommendation.** For breakdown tables use telemetry spans when the receiver has them for the phase, and label them
"exact"; otherwise stream timing, labelled "approximate". Costs: the traces flag is a beta (an upstream change could
break it, so the stream stays the fallback), and records arrive up to a second late, so a live view still shows
stream timing and is corrected when the spans arrive.

## Breakdown timing choices

These settle three open choices in the run view's tool-call breakdown (part K). All are proposals from small samples
(nine runs), labelled [I].

**1. Model-time gap threshold.** When only the stream is available, model time for a response is the time from the
previous input line (a tool result or the prompt) to `message_stop`, plus nothing for `message_start` (which is after
the first token). Measured client overhead between a tool's end and the next request's start: 13-185 ms (p50 38 ms,
n=20); the first request of a run adds 100-270 ms of start-up. Rule: a gap of up to 1 s between the previous input and
the next response is model time. A longer gap with no tool, hook or wait explaining it is shown as "unattributed
(hooks, start-up, waits)", never as model time. The 1 s is about five times the largest overhead seen. In S9 the time
outside the model was 2.4-4.0 s with the user's hooks (the plan's 0.13), which this rule would correctly not call model
time. Turns started by a background-agent notification have no input line: use the span, or mark "not reported".

**2. Nested subagent calls.** The Agent row shows self time (its span minus the time covered by its own nested model and
tool spans: 0.2-0.35 s in the two foreground runs), and the nested calls appear under their own tool rows, marked "in
subagent". Counting the Agent span's full duration and the nested rows both double-counts (the foreground subagent
runs summed to 144-158% of wall time). A background Agent's span is short and its nested spans outlive it; nest them by
`agent_id` and parent id, never by time containment.

**3. Share denominator.** Options, with the data of the nine runs:

| option | what share means | serial runs | parallel agents | pros | cons |
|---|---|---|---|---|---|
| A. sum of the table's rows (recommended) | part of all work done | rows add to 100% | model 67%, Bash 33% | rows always add to 100%; matches cost | exceeds wall time under parallelism |
| B. wall-clock time | part of elapsed time | rows add to 93-97% | model 191%, Bash 93% (sum 285%) | intuitive | over 100% when calls overlap |
| C. exclusive (each instant split among what runs) | part of elapsed time | 100% with an "other" row | model 90%, Bash 4% | adds to 100% of wall | needs overlap math; hides tool time that ran behind the model |

Recommendation: A for the "share" column, because rows then add up and the plan already shows wall-clock time and
summed tool time apart. Add C as a separate "of wall" column. Do not show B. In serial runs the three agree to within
5 points: model 45-46% and Bash 46-48% of wall, 5-7% of wall with no span running (start-up and shutdown),
permission checks 1-3%.

## Granularity and what cannot be attributed

| resource | source | granularity | cannot attribute or see |
|---|---|---|---|
| CPU | `proc_pid_rusage`, 1 s samples | per process, ns after unit conversion; one point per interval | processes shorter than the interval (96-100% of those under 100 ms at 1 s); CPU between a process's last sample and exit; threads |
| CPU totals | shim `wait4` | exact, per child, includes descendants it waited for | orphans; the tree's peak memory (it is the largest single process's); disk bytes (always 0) |
| memory | `proc_pid_rusage` | resident and footprint per process, at the instant | peaks between samples |
| disk bytes | `proc_pid_rusage` | per process; bytes written exact; live | cached reads (count 0); which file; `ru_oublock` is 0 |
| network | `nettop -P` stream | per process, bytes in and out; 1 s blocks | remote address or purpose (no per-connection rows with `-P`); anything before the stream started; closed-socket counters in a one-shot snapshot |
| GPU time | AGX clients via `ioreg` | per client process, cumulative ns; 30 ms per call | per model or request; processes that exited between reads; sharing on one GPU |
| GPU device | IOAccelerator | whole device, percent | per process |
| containers | Docker API | per container, 1 s step, ns CPU, bytes | processes inside a container; one container's processes from another's |
| detached host processes | pack hook plus libproc | per pid the hook names | pids it does not name; anything not in the lane record |
| sleep | uptime vs continuous clock | one tick (1 s) | a dark wake; sleep within one tick |
| tool timings | telemetry spans | exact, ms | calls when the receiver was off (then stream timing, within 150 ms or 3.3 s if queued) |

## Orchard census (sanitized)

Read-only look at the private Orchard repository's scripts and its own timing log (4186 parsed lines, 321 logged runs).
No command was run in the repository or a copy of it. Names are generic here; raw notes are in git-ignored `work/`.

- Timed calls by kind (seconds; n, median, 90th percentile, maximum, share under 1 s):

| kind | n | median | p90 | max | under 1 s |
|---|---|---|---|---|---|
| checks over graph edges | 1387 | 13 | 188 | 1310 | 9% |
| per-section runs | 575 | 11 | 38 | 396 | 15% |
| preflight | 178 | 44 | 76 | 481 | 0% |
| per-row jobs | 1711 | 2 | 9 | 300 | 1% |
| mutation runs | 42 | 80 | 154 | 310 | 0% |
| builds | 140 | 1.5 | 7.7 | 69 | 25% |
| type checks | 153 | 3 | 5.1 | 8.9 | 7% |

- A logged run's total timed seconds: median 100 s, p90 1331 s, maximum 5291 s (88 minutes).
- Machine load average recorded in the log lines: median 10.5. This is why the spike's runs were taken on a busy
  machine on purpose.
- Process shape, modelled in `workload/`: a launcher runs a shell, which runs a step; each step spawns node and compiler
  children (a mix of many processes under 100 ms and a few of 1-5 s), writes many small files and one large file, runs a
  test suite against a host server, and a lane brings up a container plus a host server started by a launcher that
  exits, so the server has ppid 1.
- What the census says about the sampler: most timed calls (median 2 s or more for every kind but builds) are long
  enough for 1 s sampling to see, so the time series is useful for them. The many short processes inside them are not.
  The census counts calls, not processes, so it cannot give the share of short processes; the synthetic workload's
  54% under 10 ms is an assumption, and `measure-attach.sh` is what measures the real share.

## Real Orchard runs (`measure-attach.sh`)

On 2026-11-14 two Orchard iteration runs were already going in the owner's other sessions. `measure-attach.sh` was
attached to the shell at the top of each, about 8 minutes after each started, and ran until each ended. It only read
process data; nothing was started in the repository and no process of the run was signalled. Raw output is in
git-ignored `out/attach-*/`. Run A is a review iteration; run B is a shorter iteration of a different kind.

| | run A | run B |
|---|---|---|
| sampled | 1,761 s | 402 s |
| 1 s sampler CPU | 0.091% of one core | 0.079% |
| 50 ms reference CPU | 1.86% | 1.53% |
| tick work, p50 / p99 (1 s) | 714 / 4,286 us | 690 / 7,747 us |
| processes seen by the 50 ms reference | 192 | 6 |
| ... missed by the 1 s sampler | 120 (62.5%) | 2 (33%) |
| CPU in the window, 50 ms reference | 813.8 s | 360.8 s |
| ... caught by the 1 s sampler | 787.4 s (96.8%) | 360.7 s (100.0%) |
| ... used by processes the 1 s sampler never saw | 16.1 s (2.0%) | 0.1 s (0.0%) |
| peak processes at one tick | 13 | 5 |
| process names | node 133, docker 44, psql 8, openssl 5, shells | node 2, Python, a shell, bash, git |
| network (tree, nettop) | 5 node processes, top one 202 MB in, 102 MB out | nothing seen [U: cause not found] |
| GPU | device up to 63%; no process of the tree had GPU time | device up to 44%; none in the tree |
| sleep gaps | none | none |
| Docker | not included (no filter given) | not included |

All [M], except where marked. The "CPU in the window" rows count from the first tick, so the 8 minutes before the
attach are not in them.

Run A's processes by lifetime, as the 50 ms reference saw them (a floor: a process is seen only from its first to its
last sample). The CPU column is each process's total since it started, so it includes the time before the attach
for the 5 processes already running:

| lifetime | processes | missed at 1 s | CPU s |
|---|---|---|---|
| under 0.1 s | 67 | 64 | 2.9 |
| 0.1-0.5 s | 74 | 54 | 18.0 |
| 0.5-1 s | 17 | 2 | 6.5 |
| 1-5 s | 9 | 0 | 22.1 |
| 5-30 s | 3 | 0 | 6.8 |
| 30 s or more | 22 | 0 | 778.9 |

What this changes [I]:

- **1 s is enough for CPU on real runs.** Long-lived processes (22 of 192) did 93% of run A's CPU. The short ones
  are many (141 under 0.5 s) but cheap (2.5%). The synthetic workload overstated the cost of missing them, as its census
  note warned. The `wait4` totals stay the truth; the time series at 1 s is close to it here.
- **The 50 ms reference is itself a floor.** It saw 67 processes under 0.1 s in run A; processes under 50 ms are
  missed by both, so the true miss share is higher than 62.5%, but their CPU is bounded by what the reference saw.
- **Docker work was outside the tree.** Run A ran 44 `docker` CLI processes; the work they started ran in containers,
  which the tree sampler cannot see. A lane filter (`DOCKER_FILTER`) is needed to count it.
- **A busy GPU is not the run's GPU.** The device reached 63% while no process of the tree had GPU time, so device
  utilization cannot be charged to a run.

### Run C: a review run with its lane containers

A third run, a review iteration that uses lanes, started in another session later the same day. The sampler was
attached about 3 minutes after it started, this time with a `docker ps` label filter that matches the lane containers
of that run's checkout, and ran until the run ended: 1,596 s. Two containers already existed; a third was created
during the window. All [M].

| host tree | run C |
|---|---|
| 1 s sampler CPU | 0.094% of one core (tick work p50 796 us, p99 5,687 us) |
| 50 ms reference CPU | 1.73% |
| processes seen by the 50 ms reference | 475 |
| ... missed by the 1 s sampler | 346 (72.8%) |
| CPU in the window, 50 ms reference | 551.3 s |
| ... caught by the 1 s sampler | 513.7 s (93.2%) |
| ... used by processes the 1 s sampler never saw | 23.7 s (4.3%) |
| peak processes at one tick | 13 |
| process names | node 285, bash 107, sh 39, docker 18, psql 8, dirname 7, openssl 4, shells |
| network (tree, nettop) | 5 node processes; top two 203 MB in / 102 MB out and 115 MB in / 440 MB out |
| GPU | device up to 41%; none in the tree |
| sleep gaps | none |

By lifetime (50 ms reference; CPU is each process's total since it started): under 0.1 s 254 processes (239 missed
at 1 s, 6.0 s CPU); 0.1-0.5 s 140 (103, 26.8 s); 0.5-1 s 28 (4, 11.8 s); 1-5 s 15 (0, 17.8 s); 5-30 s 19 (0, 83.1 s);
30 s or more 19 (0, 424.0 s).

The lane containers, from the Docker API (CPU, network and memory over the window; processes is the most seen at once):

| container | CPU | peak memory | network in / out | processes |
|---|---|---|---|---|
| lane database (Postgres) | 486.7 s | 259 MB | 357 / 380 MB | 18 |
| demo server | 90.0 s | 350 MB | 908 / 55 MB | 9 |
| second lane database, created during the run | 35.6 s | 303 MB | 62 / 6 MB | 17 |
| total | 612.4 s | | | |

Readings came every 2.18 s per container at the median (p95 2.5 s, max 5.4 s), not every 1 s.

What run C adds [I]:

- **Containers are half the run.** 612 s of 1,164 s CPU (53%) ran inside lane containers, mostly the database. A
  run view that shows only the host tree would show less than half the work and none of the database's. The lane
  filter is required, not an extra.
- **Short processes still cheap.** 394 processes lived under 0.5 s (32.8 s of CPU, 6% of the tree); 1 s still caught
  93% of the host CPU.
- **The 2.2 s Docker step is the helper's loop, not the API.** The helper sleeps 1 s, then runs the `docker ps` CLI
  and one stats call per container on every pass; the API call alone took 8-14 ms in the Docker section above. A
  product sampler should list a lane's containers through the API on a timer, not the CLI. Not measured separately.
- **Container startup is still unmeasured.** The second database was created about 77 s into the window and was read
  from then on; the first two were created before the attach.

## Recommendations

1. Sample the run's process tree at 1 s (0.11% of a core); offer 250 ms (0.37%); never go below 250 ms by default.
2. Treat sampling as the time series and the shim's `wait4` totals as the truth. Say in the UI that short processes
   are missed, and show the missed share when a reference exists.
3. Take disk bytes from `proc_pid_rusage`, never from `wait4`.
4. Run one streaming `nettop` per host for `full` runs, spawned with an open stdin pipe, read through a pty.
5. Poll GPU every 5 s and only when a local model is in play; label per-process time as "GPU busy seconds".
6. Use the Docker API one-shot at 1 s or slower, and the pack's `metrics.processes` hook for identities. Always
   include a run's lane containers: in run C they did 53% of the CPU. List them through the API, not the CLI.
7. Detect sleep with the two-clock formula on each sampler tick, threshold 2 s.
8. Plan for about 34 bytes per sample row (0.6-6 MB per hour for 5-50 processes at 1 s).
9. Tail report files by polling every 50 ms; caps of 4 KB per line, 100 lines per second (burst 1000), 16 MiB per file.
10. Use telemetry spans for the breakdown when present (exact), the stream otherwise (approximate); choices in
    "Breakdown timing choices".

## Proposed plan changes

Proposals only; this spike did not edit the plan.

- **0.13, Sources table, "Process sampling" row:** replace "S8 picks the interval" with 1 s (250 ms optional), tree
  only; add that disk bytes come from the sampler because `wait4` reports none (`ru_oublock` is 0); add "reads count
  only uncached I/O".
- **0.13, `nettop -P` row:** add "streamed, one per host, with stdin held open; spins at 56-130% of a core on EOF; a
  closed socket's counters are gone, so totals are the sum of the stream's deltas; per-process only".
- **0.13, GPU row:** add the measured cost (about 25 ms CPU per `ioreg` call, so poll at 5 s) and that the per-process
  counter ends with the process.
- **0.13, "Sleep is not work":** threshold 2 s on 1 s ticks; the formula is validated awake and against the machine's
  lifetime total; the mid-run test is an owner step.
- **0.13, "Volume and retention":** write "about 34 bytes per sample row, 0.6-6 MB per hour at 1 s for 5-50 processes".
- **0.13, "Claude Code's own telemetry":** headless phases that need exact timing may use the traces beta flag; state
  that `tool_result.duration_ms` equals the execution span, and that the stream's error is up to 150 ms unqueued and
  the full wait when queued.
- **Run view, tool-call time breakdown:** the three choices above (gap threshold 1 s; Agent self time with nested rows;
  share denominator is the sum of rows, with an exclusive "of wall" column).
- **0.5, report file caps:** 4 KB per line, 100 lines per second sustained with burst 1000, 16 MiB per file, and a
  50 ms poll.
- **Stage 1, S8 status:** results ready, including two real runs; one owner step remains (the sleep test, below).

## Left unmeasured

1. **The start of a real Orchard run** [U]. Three runs were measured (see "Real Orchard runs"), run C with its lane
   containers, but none from its start, so lane and container startup cost is not known. To cover it: start the run,
   find the pid of its top process with `ps` or `pgrep -f`, run `./measure-attach.sh PID DOCKER_FILTER` at once with
   the lane label filter, and add the printed report to that section. Why run B's network came out empty is not
   known.
2. **Sleep mid-run** [U]. `./sleep-check.sh`, about two minutes: it starts a watcher, the owner puts the Mac to sleep by
   hand for about a minute, wakes it, and reads "slept N s" next to the OS's own record. Nothing in the spike
   slept the machine. A watcher ran for 60 minutes on 2026-11-14 and saw no sleep, so this is still open.
3. Kqueue exit capture as a mitigation for missed processes [U]; a human permission wait in telemetry [U]; GPU under a
   local model with several clients at once [U]; behaviour on an Intel Mac, where the mach timebase is 1/1 [U].

## Reproduction

    cd .collide/planning/spikes/sp-0010-bn-metrics-sources
    ./run-all.sh          # about 12 minutes; S8_SKIP_GPU=1 and S8_SKIP_TELEMETRY=1 skip the two parts that need
                          # Ollama and a logged-in claude

It prints each measurement and saves it as `out/<part>.txt`. Raw samples are in `out/samples-r<N>-i<interval>.jsonl`
and `out/truth-r<N>.jsonl`; telemetry recordings are in `out/telemetry/`. The README lists each script. `out/`, `work/`
and `target/` are git-ignored.

The figures above come from these runs: 01 (three repeats, five intervals), 02, 03, 04 (one generation), 05, 06, 07, 08,
and the nine telemetry sessions with `telemetry/analyze.py`. The telemetry sessions and the GPU part were run once each
and are not repeated by `run-all.sh` unless the skip variables are left unset.
