---
id: sp-0004-vu
alias: S4
kind: spike
status: concluded
title: "S4: TS hooks"
owners: []
links: []
---
# S4: TS hooks

- **Date:** 2026-11-13
- **Spike:** Stage 1, S4. Plan revision 9 (frozen); this record only proposes changes and does not edit the plan.
- **Reproduction:** `.collide/planning/spikes/sp-0004-vu-ts-hooks/` (its `README.md` lists the scripts and what each prints). Every script
  was rerun from a clean copy of that directory (no `work/`, `node_modules/` or `.pnpm-store/`) and passed.

Labels: **measured** (the observation is named), **inferred** (reasoned from measurements or documented behaviour,
not observed here), **unmeasured** (not checked).

## Versions and conditions

| Item | Version |
| --- | --- |
| macOS | 26.3.1 (25D771280a), arm64, 12 CPUs |
| Node | v24.18.0 (Homebrew `node@24`) |
| TypeScript (spike-local, the one actually run) | 7.0.2, the native Go compiler: `bin/tsc` is a Node shim that `execve`s a 23 MB Mach-O binary |
| `@types/node` | 24.19.2 |
| git | 2.50.1 (Apple Git-155), SHA-1 repos |
| pnpm, gh, claude | 11.17.0, 2.96.0, 2.1.323 |
| rustc, cargo | 1.98.0 in `~/.cargo/bin` (not on the terminal's PATH) |

**The machine was loaded throughout.** The load average was 20–25 at the start and 11–15 later. The cause is four
`node --test` processes started on Nov 11, orphaned (ppid 1) and each using 98–100 % CPU (pids 52065, 62786,
68470, 95021), plus SquirrelDisk (~370 %). They are not this spike's processes and were left alone. Every timing
below is therefore an upper bound. Spreads are reported, and the comparisons between variants are more reliable
than the absolute numbers.

**`~/.zshenv` appeared during the spike.** At 08:53:32 another process created it (21 bytes, it sources
`~/.cargo/env`). The first check in this run found no such file. This decides whether `cargo` is on the captured
PATH (item 6).

## Recommendation: materialization and SDK linking

1. **Lock value = the git tree id of `<ref>:<pack path>`.** That is the repo's own content hash, in its object
   format (SHA-1 or SHA-256). `collide init` writes it with `git rev-parse <ref>:<path>`.
2. **Resolve the tree and compare it to the lock before reading anything.** Refuse on a mismatch.
3. **Cache key = the tree id, not the ref.** Use `~/.cache/collide/packs/<tree id>/`. The entry is immutable and
   needs no invalidation, and any ref or line with the same pack content reuses it.
4. **Extract by reading git objects**, without `git archive`:
   - list the files with `ls-tree -r -z <tree>` and read their content with `cat-file --batch` (or the same
     reads through an in-process git library);
   - write each file with mode 644 or 755;
   - **refuse** symlinks (mode 120000) and submodules (160000);
   - **refuse** any `node_modules` path, and any `package.json`, inside the pack (v0 packs have no npm
     dependencies, 0.6).
5. **Re-hash the extracted directory as a git tree and require it to equal the tree id.** This costs under 2 ms.
6. **Entry layout:**

   ```
   packs/<tree id>/
     pack/                          read-only, verified copy of the pack directory
     package.json                   {"private":true,"type":"module"}  (written by Collide)
     node_modules/@collide/pack-sdk -> symlink to Collide's compiled SDK (JS + .d.ts)
     .collide-verified              marker, written last
   ```

7. **Publish atomically.** Build the entry in `packs/.staging-<tree>-<pid>`, then `rename` it into place. If the
   rename loses a race to another loader, use the winner's entry.
8. **SDK:** ship it compiled, link it by symlink, and never ship `.ts`. The symlink should point to a versioned SDK
   copy that Collide keeps outside the app bundle, for example `~/.cache/collide/sdk/<version>/` (copy works
   equally well).
9. **Hooks run as** `node <entry>/pack/hooks/<x>.ts`. The run gets:
   - no flags;
   - cwd as in 0.5;
   - JSON on stdin;
   - `COLLIDE_RESULT` and `COLLIDE_REPORT`;
   - a base environment built from the captured login-shell environment (item 6).

   Optionally, set `NODE_COMPILE_CACHE` (item 4).

Why each part (labels in the item sections below):

- Steps 4 and 5: `git archive` changed the content or let a symlink through in every hazard tried (measured).
- Step 4, the refusals: a committed `node_modules` shadows Collide's SDK (measured). A pack `package.json` would
  change the module type (inferred).
- Step 6, the `package.json`: without Collide's file, a hook's ESM depends on whatever `package.json` happens to be
  above the cache (measured).
- Step 8, a compiled SDK: Node's `node_modules` check uses the realpath, so a `.ts` SDK works only by accident of
  layout (measured).
- Step 8, the location: the app bundle can move. Translocation or an update would leave dangling links, and a
  versioned directory lets `requires.sdk` be checked per entry (inferred).

## Results

### 1. Materialize a pack from a git snapshot

The scratch repo is `git init`-ed by the script. The pack is `.collide/packs/hello` (`pack.yaml`, `hooks/hello.ts`
importing `@collide/pack-sdk` and `./lib/greet.ts`, three more hooks, and an executable `hooks/version.sh`). The
lock is committed in `.collide/packs.lock`. The git used ignores global and system config.

| Check | Result | Label |
| --- | --- | --- |
| Lock = `rev-parse main:.collide/packs/hello` | `bf25c36f…` | measured |
| Three methods (raw objects, `git archive REF:PATH`, `git archive REF -- PATH`), with worktree, staged and untracked edits present | all three re-hash to the lock; no edit reaches the snapshot; 7 files; exec bit kept | measured |
| Committed change without a lock update | refused before extraction (`stage: lock`) | measured |
| `.git/info/attributes` with `eol=crlf` and `export-ignore` (local, never committed) | raw: hash ok. Both `git archive` forms: different content, caught only by the re-hash | measured |
| `export-subst` in `.git/info/attributes` | raw and `archive REF:PATH`: ok. `archive REF -- PATH`: content changed (hash mismatch) | measured |
| Committed `.gitattributes` with `export-ignore` inside the pack | raw: ok (8 files). Both archive forms drop files (mismatch) | measured |
| Committed symlink `hooks/escape.ts -> ../../../../outside.ts` | raw: refused. `archive REF:PATH`: **extracts the symlink and the hash matches**, a silent escape out of the cache entry | measured |
| Miss: total / resolve / extract / verify | ~110 ms / 55–65 ms (two git spawns) / ~40 ms / 1.4–2.4 ms | measured (under load) |
| Re-hash cost, 7 files, 20 runs | median 0.35 ms, max 1.4–3.5 ms | measured |
| Hit (marker present) | 56–61 ms, all of it the two git spawns of resolve | measured |
| In-process git (gix or git2) would remove ~25–30 ms per spawn | | inferred |
| Two processes loading one tree at once, 5 rounds | each round: one `miss` and one `lost-race`, 0 staging dirs left | measured |
| Pack dir made read-only; hooks still run | yes | measured |

Not covered here: `git archive` with a `--worktree-attributes` flag, and `tar` extraction hardening. The raw method
needs neither.

### 2. The SDK and how it is linked

Stand-in `@collide/pack-sdk` 0.1.0 (`.collide/planning/spikes/sp-0004-vu-ts-hooks/sdk/`):

- built by tsc 7.0.2 into `index.js` + `index.d.ts` + `package.json`, with zero runtime dependencies (its only
  import is `node:fs`) (measured);
- `defineHook` reads stdin JSON, gives the handler `ctx.config`, `ctx.state`, `ctx.dataDir`, `ctx.report`
  (`span`, `metric`, `progress`) and `ctx.ok`, `ctx.fail` and `ctx.refuse`;
- it writes the result file through a temporary file and a rename;
- it appends one report line per `appendFileSync`, at most 4 KB.

| Variant | Result | Label |
| --- | --- | --- |
| Symlink, compiled SDK | ok. `import.meta.url` of the SDK is its realpath (`app/Resources/pack-sdk/index.js`), outside `node_modules` | measured |
| Copy, compiled SDK | ok, loaded from `<entry>/node_modules/@collide/pack-sdk/index.js` | measured |
| Symlink, `.ts` SDK | ok: stripped, because the realpath is outside `node_modules` | measured |
| Copy, `.ts` SDK | `ERR_UNSUPPORTED_NODE_MODULES_TYPE_STRIPPING` | measured |
| The pack's own `.ts` (entry outside `node_modules`) | stripped, ok | measured |
| Pack whose realpath is under `node_modules` | the pack's own `hello.ts` is refused with the same error | measured |
| `node_modules` path whose realpath is elsewhere | ok, so Node checks the realpath | measured |
| No Collide `package.json`, typeless ancestor `package.json` | runs, with a `MODULE_TYPELESS_PACKAGE_JSON` warning on stderr naming the ancestor | measured |
| No Collide `package.json`, ancestor `{"type":"commonjs"}` | fails: "Cannot use import statement outside a module" | measured |
| Ancestor commonjs + Collide's `<entry>/package.json` `type: module` | ok | measured |
| Pack commits `node_modules/@collide/pack-sdk` | **the pack's copy wins** (`sdk=9.9.9-shadow`) | measured |
| A `package.json` inside the pack overrides Collide's `type` | Node uses the nearest `package.json` | inferred |
| Successful runs print no stderr (no type-stripping warning on 24.18) | 0 bytes | measured |

### 3. Running a hook as Collide would

The executor stand-in is `lib/collide-run.mjs`. It runs the hook with:

- argv only (`node <entry>/pack/hooks/hello.ts`) and cwd set to a separate workspace dir;
- an environment of `PATH`, `HOME`, `COLLIDE_RUN`, `COLLIDE_SPAN`, `COLLIDE_RESULT`, `COLLIDE_REPORT` and
  `COLLIDE_PACK_DATA`;
- 0.5-shaped JSON on stdin.

It judges by the 0.5 rules.

| Check | Result | Label |
| --- | --- | --- |
| Result file | `{"status":"ok","message":"","outputs":{"text":"hi, T-1 and T-2","lanePool":3,"cwd":"<workspace>",…}}`: config from stdin, cwd honoured, relative `.ts` import resolved | measured |
| Report file | 4 lines: `span.start`, `metric` with `parent`, `span.end` `ok`, `progress`. All have `v:1` and an ISO `t`, ids `<pid>-<n>`, none over 4 KB, none unclosed | measured |
| `COLLIDE_REPORT_PARENT=4121-1` | the top span carries `parent: "4121-1"` | measured |
| `ctx.fail` | verdict `fail`, exit 0, message kept | measured |
| Handler throws | stack on stderr, result file `error` with the message, exit 1, verdict `error` | measured |
| `process.exit(0)` before any verdict | no result file, verdict `error` | measured |
| Handler returns without a verdict | the SDK's code writes `error` and exits 1, but no fixture hook exercises it | unmeasured |

### 4. Cold start

`lib/bench.mjs` runs each variant round-robin, after 2 warm-up runs. Wall time is measured from spawn to close.
The reference run had 30 runs per variant, at load 12.65 → 11.91. The clean-copy rerun had 10 runs, at load
22.6 → 21.4, and agrees within a few ms on every median except "SDK copy" (133 ms).

| Variant | Median ms | Min | p90 | Max | IQR | Label |
| --- | --- | --- | --- | --- | --- | --- |
| `node -e 0` | 64.7 | 61.5 | 76.7 | — | 4.3 | measured |
| `node empty.js` | 65.8 | | | | | measured |
| `node empty.ts` | 101.7 | | | | | measured |
| hook + SDK, symlink | 122.8 | 113.7 | 131.9 | 338.9 | 8.4 | measured |
| hook + SDK, copy | 123.3 | | | | | measured |
| hook, 1567 tasks (493 KB stdin) | 125.9 | | | | | measured |
| hook + SDK, `NODE_COMPILE_CACHE` warm | 81.1 | 77.4 | | | 5.9 | measured |

- Type stripping costs ~36 ms over plain JS. The SDK, the stdin read and the hook add ~21 ms more. Symlink and copy
  cost the same. Input size barely matters (measured).
- A warm compile cache removes ~42 ms, bringing a TS hook within ~16 ms of `node -e 0` (measured).
  `NODE_COMPILE_CACHE` is inherited by any Node child the hook starts, which would then write into Collide's cache
  directory (inferred).
- An unloaded machine should be faster in absolute terms (inferred). The first run at load 20–25 gave 161 ms for
  `node -e 0` and 292 ms for a hook, so absolute numbers here are only good as upper bounds.

### 5. `tsc --noEmit` over the materialized pack

The tsconfig uses:

- `strict` and `noEmit`;
- `module` and `moduleResolution` `nodenext`;
- `allowImportingTsExtensions`, `erasableSyntaxOnly` and `verbatimModuleSyntax`;
- `types: ["node"]`.

| Check | Result | Label |
| --- | --- | --- |
| Good pack, SDK symlink, `@types/node` | exit 0. 7-run median 384 ms (365–398) in the quieter run; 585 ms at load ~20. 184 files; tsc's own total 0.29–0.38 s | measured |
| Good pack, SDK copy | exit 0 | measured |
| Without `@types/node` | exit 1: TS2591 (`process`) ×2, plus a follow-on TS2345 | measured |
| Deliberate type error (`ctx.config.lanePool satisfies string`) | exit 1, TS1360 | measured |
| Type imported without `type` | TS1484 (`verbatimModuleSyntax`) | measured |
| `enum` | TS1294 (`erasableSyntaxOnly`) | measured |
| The same two at run time without tsc | "does not provide an export named 'HookInput'"; `ERR_UNSUPPORTED_TYPESCRIPT_SYNTAX`. tsc catches both before a run does | measured |
| Which SDK `.d.ts` tsc read (`--listFiles`) | the symlink's realpath | measured |
| SDK build (declaration emit) with tsc 7 | ok, ~0.2 s | measured |

**Verdict: `tsc --noEmit` behaved as the plan assumes.** TypeScript 7.0.2 type-checks the stripped-TS dialect
correctly and fails on a type error. There is no reason to pin 5.x for the check (measured).

One difference does matter: the `typescript` 7 package's JS export is only `{ version, versionMajorMinor }`
(measured). Anything in Collide that needs the classic compiler API (`ts.createProgram` and so on) would need a
5.x/6.x package instead (inferred). If Collide only runs the CLI, pin `typescript` 7.0.x exactly. The native
binary is per platform (26 MB package for darwin-arm64, plus a 3.5 MB shim package), and `@types/node` is 2.6 MB
(measured).

### 6. Finder environment

The capture script is `lib/env-capture.mjs`:

- the shell is `$SHELL`, else `getpwuid`;
- it runs `<shell> -l -c` or `<shell> -l -i -c` with `printf <nonce BEGIN>; /usr/bin/env -0; printf <nonce END>`;
- stdin is `/dev/null` and the timeout is 10 s;
- it parses the NUL-separated block between the markers.

Then it resolves `node`, `claude`, `pnpm`, `gh`, `git` and `cargo` on the captured PATH and runs each with
`--version` under exactly that environment.

| Launch / mode | Time (median) | Pollution | node | claude | pnpm | gh | git | cargo | Label |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Simulated launch env (`env -i HOME PATH=/usr/bin:/bin:/usr/sbin:/sbin`): 3 vars, no `SHELL` (getpwuid → `/bin/zsh`) | — | — | no | no | no | no | yes | no | measured |
| that launch, `-l -c` | 39.5–44 ms | 0 B | yes | **no** | yes | yes | yes | yes | measured |
| that launch, `-l -i -c` | 366–949 ms | 0 B | yes | yes | yes | yes | yes | yes | measured |
| Real `.app`, started by a launchd job in `gui/<uid>` (a Finder/Dock proxy): 16 vars, `PATH=/usr/bin:/bin:/usr/sbin:/sbin`, `SHELL` set, ppid 1 | — | — | no | no | no | no | yes | no | measured |
| that app, `-l -c` | 295 ms | 0 B | yes | **no** | yes | yes | yes | yes | measured |
| that app, `-l -i -c` | 882 ms (753–1077) | 0 B | yes | yes | yes | yes | yes | yes | measured |
| Real `.app` via `env -i … open` | 16 vars, same PATH; `-l -c` 298 ms, claude no; `-l -i -c` 1078 ms (max 2.6 s), all found | | | | | | | | measured |
| Real `.app` via `open` from a terminal | 71 vars incl. `CLAUDECODE` and the terminal's PATH. `-l -c` then also finds claude, through the inherited PATH | | | | | | | | measured |

- **`claude` is found only with `-i`**, because `~/.local/bin` is added in `~/.zshrc`. The other tools come from
  `~/.zprofile` (measured).
- **cargo** is found by both modes only through `~/.zshenv`. With a `ZDOTDIR` shim that reads `.zprofile` and
  `.zshrc` but not `.zshenv`, cargo is found by neither mode, and node, pnpm, gh and claude are unchanged
  (measured). Before 08:53 today no capture method would have found cargo (inferred from the shim).
- **Delimiting:** the synthetic rc files printed 66–89 bytes before the marker, 25 bytes on stderr in `-i`, and a
  decoy `BEGIN` marker. Nonce markers + `env -0` still parsed 1/1, and a multi-line value came through intact
  (measured). The user's real rc files produced 0 bytes of pollution (measured).
- **`open` from a terminal is not a valid Finder test.** `open` forwards the caller's environment to the app
  (measured), and the capture inherits it (measured: the terminal PATH let `-l -c` find claude). So `CLAUDECODE`
  and other terminal variables would reach the captured env too (inferred).
- `launchctl getenv PATH` is empty, so there is no user-domain PATH override (measured).
- The capture inside a launched app was slower than in the simulation (295 vs 40 ms for `-l -c`), with the same rc
  files. The cause was not isolated: load, or startup files reacting to the extra launchd variables (unmeasured).

## Proposed plan changes (proposals only; the plan was not edited)

| Plan section | What S4 found | Proposed change |
| --- | --- | --- |
| **0.6**, "A git snapshot has no `node_modules` (R2), so the SDK cannot come from the repo" | **Contradicted.** A snapshot can contain a committed `node_modules/`, and it shadows Collide's SDK (§2) | "A pack tree that contains a `node_modules` path, or a `package.json`, is refused at load. Collide supplies the SDK and the module type." |
| **0.6**, "materializes the pack's snapshot into `~/.cache/collide/packs/<lock-hash>/` … links its own bundled SDK into that directory's `node_modules`" | Confirmed. Sharpened by §1 and §2 | Name the layout: `pack/` (read-only) plus Collide's `package.json` `{"type":"module"}` and `node_modules/@collide/pack-sdk` one level up. The SDK is compiled and linked by symlink to a versioned copy outside the app bundle. Add: "Node checks the realpath; a `.ts` SDK is not shipped even though a symlinked one would strip." |
| **0.6**, cold start "0.08–0.09 s over 3 runs" (R1, inline TS) | A hook with the SDK: 123 ms median vs 65 ms for `node -e 0`, on a loaded machine; 81 ms with a warm `NODE_COMPILE_CACHE` (§4) | Record the S4 numbers. Optionally, give hooks `NODE_COMPILE_CACHE=~/.cache/collide/node-compile`, noting that Node children inherit it. |
| **0.6**, "`collide pack test` runs `tsc --noEmit`"; **Stage 12**, `collide pack test` | Confirmed with TS 7.0.2 (§5). Without `@types/node` the check fails on `process` | Collide bundles `tsc` (TS 7.0.x, pinned exactly; ~30 MB per platform) and `@types/node`, and generates the tsconfig of §5. Use TS 5.x/6.x only if a component needs the compiler's JS API. |
| **0.7**, "`.collide/packs.lock` pins each pack by content hash"; "verified against the lock hash" | §1 | "Content hash" = the git tree id of `<ref>:<pack path>`, in the repo's object format. Materialize by reading git objects, never `git archive`. Re-hash after extraction. Refuse symlinks and submodules. |
| **Stage 4**, "Snapshot materialization (S4), keyed by ref so that lines (Stage 14) reuse it" | **Contradicted.** A ref moves and its cache entry would go stale. A tree-keyed entry is immutable, and lines reuse it whenever the content matches | "…keyed by the pack's tree id, which lines share whenever their pack content matches; staging dir + atomic rename; marker written last." Add cache garbage collection (unmeasured). Consider an in-process git library for the resolve (~56–61 ms per load is git spawns; inferred saving). |
| **Stage 4**, `@collide/pack-sdk` v0 | §2, §3 | The SDK writes the result through tmp + rename, and turns a handler that returns without a verdict, or throws, into `error` + exit 1. A hook that exits before a verdict leaves no file, which 0.5 already treats as `error` (measured). |
| **0.5**, "Env: `COLLIDE_*` plus role, phase and experiment templates" | The base environment is not named. From a Finder/launchd launch it is `PATH=/usr/bin:/bin:/usr/sbin:/sbin`, where `node` is not found (§6) | "The base env is the captured login-shell env (0.2), minus the adapter's declared variables." |
| **0.2** (process owner) and **Stage 2/3**; **Stage 1 S4** success line "finds `node`, `claude`, `pnpm`, `gh`" | Met only with `-l -i -c`: `-l -c` misses claude (§6) | The process owner, whichever S3 picks, captures the env at its own start and on a "reload shell env" command. The capture: `$SHELL` (else getpwuid) with `-l -i -c`, nonce markers + `env -0`, stdin `/dev/null`, a timeout with fallback to `-l -c` and then to the launch env, plus a user PATH override in settings. Start the capture from a fixed minimal base (HOME, USER, LOGNAME, SHELL, TMPDIR, system PATH) so the result does not depend on how Collide was started (inferred). Budget 0.3–1.1 s, once, off the UI path. |

## Not measured, or could not be checked

- A real Finder double-click. A launchd job in the gui domain running `open` stood in for it. The Dock, Spotlight
  and login items were not tried.
- rc files that block, prompt or need a tty under `-i` (oh-my-zsh update prompts, instant-prompt themes). The
  timeout and fallback are designed for them but were not exercised.
- Shells other than zsh (bash, fish).
- SHA-256 repos (the code takes the object format into account; not run).
- Large packs and many files, re-hash cost at scale, and cache garbage collection.
- An SDK update while cache entries exist (relinking and `requires.sdk` checks); app translocation and moving the
  app.
- Node 22.18–23.x (the `requires.tools` floor), `--preserve-symlinks`, Windows and Linux.
- First-ever cold disk cache, and an unloaded machine.
- How `ctx.dataDir` reaches a hook: the stand-in used `COLLIDE_PACK_DATA`. The plan does not name the variable.
- The cause of the slower capture inside a launched app.
