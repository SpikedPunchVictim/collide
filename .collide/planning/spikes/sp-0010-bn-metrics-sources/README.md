# S8: metrics sources

Spike S8 for Collide: which per-process and per-run metrics the macOS host can give, at what cost, and what each one
cannot attribute. The findings and recommendations are in `record.md`. This file says how to run things.

## Run everything

    ./run-all.sh

It builds the sampler, runs every measurement, prints the results, and saves each one in `out/`. It takes about 12
minutes. Two parts need outside tools and are skipped with a message when the tool is missing:

- GPU (`04-gpu.py`) needs an Ollama server with `llama3.1:8b` pulled. Skip it with `S8_SKIP_GPU=1`.
- Telemetry (`telemetry/run_sessions.py`) runs nine small `claude -p` sessions on the haiku model, about three cents
  in all. It needs a logged-in `claude`. Skip it with `S8_SKIP_TELEMETRY=1`. The analysis of earlier recordings in
  `out/telemetry/` still runs.

Other needs: Python 3 (stdlib only), cargo, node, and Docker Desktop running. Nothing is installed. The run starts one
container with a unique name and a few small servers, and removes all of them at the end. It does not read or write
anything outside this folder, and it does not touch any real project.

## Files

| File | What it measures |
| --- | --- |
| `sampler/` | The candidate sampler, std-only Rust: `s8-sampler --interval-ms N (--root PID \| --all) --out FILE`. |
| `workload/` | A synthetic workload with the process shape of an Orchard iteration run (see `record.md`, "Orchard census"). It reaps every child with `wait4` and writes the exit totals as ground truth. |
| `01-overhead-miss.py` | Sampler CPU at 1000, 500, 250, 100 and 50 ms, and the share of short-lived processes missed, against `wait4`. |
| `02-diskio.py` | Per-process disk bytes through `proc_pid_rusage`, and the mach-time CPU units. |
| `03-nettop.py` | `nettop -P` per process: cost, delivery, closed connections, short flows. |
| `04-gpu.py` | Per-process GPU time and device utilization under Ollama. |
| `05-docker-detached.py` | Docker API stats, and detached host processes, through a prototype `metrics.processes` hook (`hooks/`). |
| `lib/sleepclock.py` | Sleep detection from two clocks. `selftest` runs in `run-all.sh`; `sleep-check.sh` is the owner step. |
| `07-store-growth.py` | SQLite growth per hour for process samples. |
| `08-report-tail.py` | Tail lag and CPU for a report file, at several line rates. |
| `telemetry/` | OTLP receiver, `claude -p` runs, and the comparison of telemetry with stream-json. |
| `measure-attach.sh` | Owner step: samples a real run beside it (see below). |

The numbered files match the section order in `record.md`, not the order in the brief.

## Owner steps (not run by the spike)

Both were prepared but not run by the spike, so that nothing real was touched and the machine was never put to
sleep. On 2026-11-14 the attach was run beside three live runs, one with its lane containers (record, "Real Orchard
runs"); the sleep test is still open.

**Sleep detection.** `./sleep-check.sh` starts a watcher. Put the Mac to sleep by hand for about a minute, wake it,
and read the line `slept N s`. It also prints the OS's own record to compare with.

**A real iteration run.** Start an Orchard iteration run (or a lane test) as usual. Find the pid of its top process,
then in this folder run `./measure-attach.sh PID [DOCKER_FILTER]`. It runs two read-only samplers (1 s and 50 ms) and
one helper for network, GPU, sleep and Docker, and prints a report when the run ends. It only reads. It never signals
any process other than the ones it started. Add the report to `record.md`, section "Real Orchard runs".

## Notes

- Outputs go to `out/` and scratch files to `work/`. Both, with `target/` and `node_modules/`, are git-ignored.
- Results are labelled [M] measured here, [I] inferred from measurements, [U] unmeasured.
