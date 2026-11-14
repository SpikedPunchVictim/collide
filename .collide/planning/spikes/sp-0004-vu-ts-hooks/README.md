# Spike S4: TS hooks

Throwaway reproduction for `.collide/planning/spikes/sp-0004-vu-ts-hooks/record.md`. A pack is loaded from a git snapshot into a
content-keyed cache, a Collide-supplied SDK is linked into it, and hooks run with Node's type stripping.

Requirements: macOS, Node >= 22.18 (measured on 24.18.0), git, pnpm. The first script that needs TypeScript runs
`pnpm install --frozen-lockfile` into `node_modules/` with a spike-local store (`.pnpm-store/`), which needs the
network once. Nothing is installed globally. Every script writes only under `work/<script>/` and recreates it.

| Script | Measures (plan item) | Prints |
| --- | --- | --- |
| `scripts/01-materialize.sh` | 1: snapshot to `cache/packs/<tree id>/`, lock check, uncommitted edits, `git archive` hazards | one JSON line per method and hazard, with timings |
| `scripts/02-link-sdk.sh` | 2: SDK symlink vs copy, compiled vs `.ts` SDK, `node_modules` realpaths, module type, shadowing | one line per variant: verdict, where the SDK loaded from, Node error codes |
| `scripts/03-run-hook.sh` | 3: run as Collide would; result file, verdicts, report lines | the result file, every report line and its check, one line per verdict case |
| `scripts/04-cold-start.sh [RUNS]` | 4: cold start vs `node -e 0` | a table: median, min, p10, p90, max, IQR per variant (default 20 runs) |
| `scripts/05-tsc.sh` | 5: `tsc --noEmit` against the SDK `.d.ts`, deliberate type error | exit code, time and TS error codes per check; `--listFiles` SDK path; timing |
| `scripts/06-finder-env.sh [RUNS]` | 6: login-shell env capture from an `env -i` launch | per mode: time, pollution bytes, captured PATH, which tools resolve |
| `scripts/07-app-bundle.sh` | 6: a real `.app` launched three ways (shell `open`, `env -i open`, launchd job) | per launch: env size, PATH, capture results |
| `scripts/all.sh` | runs 01 to 07 in order | |

Notes:

- 06 and 07 print variable names and `PATH` only, never other values. 06 and 07 run the user's real zsh startup
  files (that is the point). 06 also uses synthetic rc files through `ZDOTDIR` to test the delimiting.
- 07 launches a background app (no Dock icon) three times and bootstraps a one-shot launchd job in the user's gui
  domain; the job's plist stays in `work/07-app-bundle/` and the job is booted out when the probe finishes. The app
  is unregistered from LaunchServices at the end.
- Scratch repos ignore the user's global git config (`GIT_CONFIG_GLOBAL=/dev/null`).

Layout:

- `fixture/pack/`: the pack committed into each scratch repo (`pack.yaml`, `hooks/*.ts`, `hooks/lib/greet.ts`).
- `sdk/`: the stand-in `@collide/pack-sdk` source; scripts compile it with the spike's `tsc` into
  `work/<script>/app/Resources/pack-sdk/` (JS + `.d.ts`, zero runtime dependencies).
- `lib/pack-cache.mjs`: materialize (raw objects or `git archive`), git tree hashing without git, SDK linking.
- `lib/collide-run.mjs`: the hook executor stand-in (0.5) and the verdict and report checks.
- `lib/bench.mjs`, `lib/env-capture.mjs`, `lib/common.sh`: timing, env capture, shared shell helpers.
