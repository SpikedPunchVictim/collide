// One test's world: a temp root with a fixture repository and fake Claude, and the bridge started over it
// (`app/src-tauri/src/bin/collide-bridge/`). The bridge starts the daemon, so nothing here touches the real home, the
// real launchd or a real daemon.
//
// The test rule. Every process of a world has an environment of its own: the bridge's is `PATH` alone, and the bridge
// gives its daemon `env_clear()` plus `HOME` and `COLLIDE_DAEMON_ROOT` in the root. Fake Claude is first on the
// `PATH` that the daemon's login shell hands out. The root is below `/tmp` and is removed by its own path.
//
// The daemon decides who is asking from the process tree, and a test run inside a run of Collide has that run's
// marker in its parents. The bridge is therefore started the way `host/tests/common/detached.rs` starts a command: with
// a shell between that is gone before the bridge starts, so the bridge has no test process among its parents.
import { execFileSync, spawnSync } from 'node:child_process'
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  realpathSync,
  rmSync,
  statSync,
  writeFileSync,
} from 'node:fs'
import { request } from 'node:http'
import { connect } from 'node:net'
import { join } from 'node:path'
import { FAKE_SCRIPT } from './recordings.ts'

const REPO_ROOT = join(import.meta.dirname, '..', '..', '..')
const TARGET = process.env.CARGO_TARGET_DIR ?? join(REPO_ROOT, 'target')

/** The binaries `global-setup.ts` builds. */
export const BRIDGE_EXE = join(TARGET, 'debug', 'collide-bridge')
export const COLLIDE_EXE = join(TARGET, 'debug', 'collide')

/** The root of the repository, for a test that reads a fixture of the host's tests. */
export const SOURCE_ROOT = REPO_ROOT

const PYTHON = '/usr/bin/python3'
const GIT_ENV = {
  PATH: '/usr/bin:/bin',
  HOME: '/nonexistent',
  GIT_CONFIG_GLOBAL: '/dev/null',
  GIT_CONFIG_SYSTEM: '/dev/null',
  GIT_CONFIG_NOSYSTEM: '1',
}

/** The prefix of every root, so that removing one is never a guess. */
const ROOT_PREFIX = 'cdb-'

/** What fake Claude plays: the recording for the first launch, and one for a launch with `--resume`. */
export interface Plan {
  readonly first: string
  readonly resumed?: string
  /** Folders put on the `PATH` of the daemon's login shell, after fake Claude's: where `node` is, for a pack. */
  readonly path?: readonly string[]
  /** `none` leaves fake Claude out, so the daemon finds no `claude` on its `PATH` (ts-0194-qp). */
  readonly claude?: 'fake' | 'none'
}

/**
 * `$1` is the folder for the files, the rest is the command. The outer shell ends at once; the subshell it leaves waits
 * for it to be gone before it starts the command, so the command is an orphan, and records its pid and its exit code.
 */
const LAUNCHER = `dir=$1; shift
(
  while kill -0 $$ 2>/dev/null; do sleep 0.01; done
  "$@" </dev/null >"$dir/out" 2>"$dir/err" &
  pid=$!
  echo $pid >"$dir/pid.tmp"; mv "$dir/pid.tmp" "$dir/pid"
  wait $pid
  echo $? >"$dir/code.tmp"; mv "$dir/code.tmp" "$dir/code"
) </dev/null >/dev/null 2>&1 &
`

export function git(dir: string, ...args: string[]): string {
  return execFileSync('git', args, { cwd: dir, env: GIT_ENV, encoding: 'utf8' }).trim()
}

function sleep(ms: number): Promise<void> {
  return new Promise((resolve) => setTimeout(resolve, ms))
}

export async function waitFor<T>(what: string, within: number, read: () => T | undefined | false): Promise<T> {
  const until = Date.now() + within
  for (;;) {
    const found = read()
    if (found) return found
    if (Date.now() > until) throw new Error(`timed out after ${within} ms waiting for: ${what}`)
    await sleep(25)
  }
}

function alive(pid: number): boolean {
  try {
    process.kill(pid, 0)
    return true
  } catch {
    return false
  }
}

/** The pid and command of every process of this user, from `ps`. */
function processes(): { pid: number; command: string }[] {
  const text = execFileSync('/bin/ps', ['-axo', 'pid=,command='], { encoding: 'utf8', maxBuffer: 64 * 1024 * 1024 })
  return text
    .split('\n')
    .map((line) => /^\s*(\d+)\s+(.*)$/.exec(line))
    .filter((match) => match !== null)
    .map((match) => ({ pid: Number(match[1]), command: match[2]! }))
}

/** The processes whose command line names `text`, other than this one. */
export function processesNaming(text: string): { pid: number; command: string }[] {
  return processes().filter((p) => p.pid !== process.pid && p.command.includes(text))
}

export interface Bridged {
  readonly port: number
  readonly token: string
}

export class World {
  readonly root: string
  readonly repo: string
  readonly claudeLog: string
  readonly url: string
  readonly token: string
  private readonly detachedDir: string
  private readonly bridgePid: number
  private stopped = false

  private constructor(root: string, bridged: Bridged, detachedDir: string, bridgePid: number) {
    this.root = root
    this.repo = join(root, 'repo')
    this.claudeLog = join(root, 'bin', 'claude-calls.log')
    this.url = `http://127.0.0.1:${bridged.port}`
    this.token = bridged.token
    this.detachedDir = detachedDir
    this.bridgePid = bridgePid
  }

  /** Makes the root, the repository and fake Claude, and starts the bridge over them. */
  static async start(plan: Plan): Promise<World> {
    const root = realpathSync(mkdtempSync(`/tmp/${ROOT_PREFIX}`))
    try {
      return await World.startIn(root, plan)
    } catch (failure) {
      removeRoot(root)
      throw failure
    }
  }

  private static async startIn(root: string, plan: Plan): Promise<World> {
    const bin = join(root, 'bin')
    mkdirSync(bin)
    const first = join(root, 'first.jsonl')
    writeFileSync(first, plan.first)
    let resumed: string | null = null
    if (plan.resumed !== undefined) {
      resumed = join(root, 'resumed.jsonl')
      writeFileSync(resumed, plan.resumed)
    }
    if (plan.claude !== 'none') {
      writeFileSync(join(bin, 'claude'), fakeClaude(join(bin, 'claude-calls.log'), first, resumed), { mode: 0o755 })
      chmodSync(join(bin, 'claude'), 0o755)
    }
    // The login shell the daemon runs to read its base environment: it puts fake Claude first on the PATH.
    const shell = join(root, 'fake-shell')
    const path = [bin, ...(plan.path ?? []), '/usr/bin', '/bin'].join(':')
    writeFileSync(shell, `#!/bin/sh\nexport PATH=${path}\neval "$4"\n`, { mode: 0o755 })
    chmodSync(shell, 0o755)
    // The aliases of models.yaml are the fixture's own, below the HOME of the daemon.
    mkdirSync(join(root, '.config', 'collide'), { recursive: true })
    writeFileSync(
      join(root, '.config', 'collide', 'models.yaml'),
      'tools:\n  claude:\n    fast: { name: haiku }\n    strong: { name: opus }\n',
    )
    const repo = join(root, 'repo')
    mkdirSync(repo)
    git(repo, 'init', '-q', '-b', 'main')
    // The identity of the merge commits that accept makes: the daemon's git reads no global configuration.
    git(repo, 'config', 'user.name', 'test')
    git(repo, 'config', 'user.email', 'test@example.invalid')
    git(repo, 'config', 'commit.gpgsign', 'false')
    writeFileSync(join(repo, 'README'), 'hello\n')
    git(repo, 'add', '.')
    git(repo, 'commit', '-q', '-m', 'start')

    const detachedDir = join(root, 'bridge')
    mkdirSync(detachedDir)
    const args = [
      '--root', root,
      '--shell', shell,
      '--static', process.env.COLLIDE_BRIDGE_PAGE ?? '',
      '--parent', String(process.pid),
    ]
    const launched = spawnSync('/bin/sh', ['-c', LAUNCHER, 'sh', detachedDir, BRIDGE_EXE, ...args], {
      env: { PATH: '/usr/bin:/bin' },
      stdio: 'ignore',
    })
    if (launched.status !== 0) throw new Error(`the launcher exited with ${launched.status}`)
    const pid = await waitFor('the bridge to start', 15_000, () => {
      const text = readText(join(detachedDir, 'pid'))
      return text === '' ? undefined : Number(text)
    })
    const bridged = await waitFor('the bridge to listen', 60_000, () => {
      const line = readText(join(detachedDir, 'out')).split('\n').find((l) => l.startsWith('listening '))
      if (line === undefined) {
        const ended = readText(join(detachedDir, 'code'))
        if (ended !== '') throw new Error(`the bridge ended with ${ended}: ${readText(join(detachedDir, 'err'))}`)
        return undefined
      }
      const [, port, token] = line.split(' ')
      return { port: Number(port), token: token! }
    })
    return new World(root, bridged, detachedDir, pid)
  }

  /** One call of the owner's protocol through the bridge, as the page makes it. */
  async rpc<T = any>(method: string, params: unknown = {}): Promise<T> {
    const answer = await fetch(`${this.url}/rpc`, {
      method: 'POST',
      headers: { 'content-type': 'application/json', authorization: `Bearer ${this.token}` },
      body: JSON.stringify({ method, params }),
    })
    const body = (await answer.json()) as { result?: T; error?: { message: string } }
    if (body.error !== undefined) throw new Error(`${method}: ${body.error.message}`)
    return body.result as T
  }

  /** The links `shell.open_external` was asked to open, in order. The bridge opens none (ts-0183-ee). */
  async opened(): Promise<string[]> {
    const answer = await fetch(`${this.url}/opened`, { headers: { authorization: `Bearer ${this.token}` } })
    return (await answer.json()) as string[]
  }

  /**
   * The whole text of every log of this world: the bridge's, the daemon's, fake Claude's and any other `.log` or
   * `.err` file below the root, but not the repository. For a test that looks for what must never be written.
   */
  logs(): string {
    return filesBelow(this.root, [this.repo])
      .filter((file) => /\.(log|err)$/.test(file) || file.startsWith(this.detachedDir))
      .map((file) => `--- ${file}\n${readText(file)}`)
      .join('\n')
  }

  /**
   * The files below the root, repository included, that hold `text`. Sockets and the like are not read. A file that
   * is gone by the time it is read is skipped: SQLite removes its `-wal` and `-shm` files as connections close, and
   * what they held is in the store's own file by then. Any other failure to read throws.
   */
  filesHolding(text: string): string[] {
    return filesBelow(this.root, []).filter((file) => {
      try {
        return readFileSync(file, 'utf8').includes(text)
      } catch (failure) {
        if ((failure as NodeJS.ErrnoException).code === 'ENOENT') return false
        throw failure
      }
    })
  }

  /** What the bridge and the daemon wrote, for a failure's report. */
  diagnostics(): string {
    const parts = [
      ['the bridge', join(this.detachedDir, 'err')],
      ['the daemon', join(this.root, 'daemon.err')],
      ['the daemon log', join(this.root, 'data', 'daemon.log')],
    ] as const
    return parts.map(([name, file]) => `--- ${name}\n${readText(file).slice(-4000)}`).join('\n')
  }

  /** Whether the daemon of this world answers on its socket. */
  daemonAnswers(): Promise<boolean> {
    return new Promise((resolve) => {
      const socket = connect(join(this.root, 'run', 'd.sock'))
      socket.once('connect', () => {
        socket.destroy()
        resolve(true)
      })
      socket.once('error', () => resolve(false))
    })
  }

  /**
   * Ends the bridge, which stops the runs that are live and the daemon, then looks for what is left. A process that
   * names this root and is still there is killed and reported. Removes the root. Safe to call twice.
   */
  async stop(): Promise<string[]> {
    if (this.stopped) return []
    this.stopped = true
    await this.shutdown().catch(() => undefined)
    if (alive(this.bridgePid)) {
      process.kill(this.bridgePid, 'SIGTERM')
      await waitFor('the bridge to end', 5_000, () => !alive(this.bridgePid)).catch(() => {
        process.kill(this.bridgePid, 'SIGKILL')
      })
    }
    const left = processesNaming(this.root)
    const report = left.map((p) => `${p.pid} ${p.command}`)
    for (const p of left) {
      try {
        process.kill(p.pid, 'SIGKILL')
      } catch {
        // It ended by itself.
      }
    }
    if (await this.daemonAnswers()) report.push('the daemon still answers on its socket')
    removeRoot(this.root)
    return report
  }

  private shutdown(): Promise<void> {
    return new Promise<void>((resolve, reject) => {
      const call = request(
        `${this.url}/shutdown`,
        { method: 'POST', headers: { authorization: `Bearer ${this.token}`, 'content-type': 'application/json' } },
        (answer) => {
          answer.resume()
          answer.on('end', () => resolve())
        },
      )
      call.on('error', reject)
      call.end('{}')
    }).then(async () => {
      await waitFor('the bridge to end', 30_000, () => !alive(this.bridgePid))
    })
  }
}

/** The regular files below `dir`, but not below any of `skip`. */
function filesBelow(dir: string, skip: readonly string[]): string[] {
  if (skip.includes(dir)) return []
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name)
    const kind = statSync(path, { throwIfNoEntry: false })
    if (kind === undefined) return []
    if (kind.isDirectory()) return filesBelow(path, skip)
    return kind.isFile() ? [path] : []
  })
}

function readText(file: string): string {
  try {
    return readFileSync(file, 'utf8').trim()
  } catch {
    return ''
  }
}

/** Removes a root made by [`World.start`], by its own path. */
function removeRoot(root: string): void {
  if (!root.startsWith(`/private/tmp/${ROOT_PREFIX}`) && !root.startsWith(`/tmp/${ROOT_PREFIX}`)) {
    throw new Error(`refusing to remove ${root}`)
  }
  if (!existsSync(root)) return
  // The daemon's cache holds a read-only copy of the SDK (a board's first load makes it), which `rm` cannot empty.
  spawnSync('/bin/chmod', ['-R', 'u+w', root])
  rmSync(root, { recursive: true, force: true })
}

/** The `claude` script: the version, and the recording played for a first launch and for one with `--resume`. */
function fakeClaude(log: string, first: string, resumed: string | null): string {
  const play = (recording: string) => `exec ${PYTHON} ${FAKE_SCRIPT} --emulate-hooks ${recording} "$@"`
  const resume = resumed === null ? '' : `case " $* " in *" --resume "*) ${play(resumed)} ;; esac\n`
  return (
    `#!/bin/sh\necho "$*" >> ${log}\n` +
    `if [ "$*" = "--version" ]; then echo "2.1.325 (Claude Code)"; exit 0; fi\n` +
    `${resume}${play(first)}\n`
  )
}
