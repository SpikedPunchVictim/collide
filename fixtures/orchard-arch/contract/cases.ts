import { spawnSync } from 'node:child_process'
import { readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { runShape } from './shape.ts'
import type { Ended, RunShape } from './shape.ts'

// The calls that the Orchard pack makes to the plan CLI (fixtures/packs/orchard/providers/plan: `write.ts` for a write
// and a preview, `queue.ts` for the human queue), one case each. The pack runs `./plan <args> --root <root>/arch
// --format json`, with `--dry-run` for a preview. The cases run in this order, on one root, and the writes change it.

/** What a root needs for the cases: a `ready` task, and a proposal and an epic that `new task` may use. */
export interface Params {
  task: string
  proposal: string
  epic: string
}

export interface Case {
  name: string
  /** The arguments before `--root`. `<task>`, `<proposal>`, `<epic>` and `<title>` are replaced by the parameters. */
  args: string[]
  dryRun?: boolean
}

/** The title of every task that a case makes. It is made up. */
export const TITLE = 'Probe task'

/** An id that no root has: a task that is not there, and a proposal that is not. */
const NO_TASK = 'T-99-999'
const NO_PROPOSAL = 'P99'

export const CASES: Case[] = [
  { name: 'status', args: ['status'] },
  { name: 'set-dry-run', args: ['set', '<task>', 'status=doing'], dryRun: true },
  { name: 'set-legal', args: ['set', '<task>', 'status=doing'] },
  // The task is `doing` now, and no lifecycle goes from there back to `specified`.
  { name: 'set-illegal', args: ['set', '<task>', 'status=specified'] },
  { name: 'set-no-task', args: ['set', NO_TASK, 'status=doing'] },
  { name: 'new-task-dry-run', args: ['new', 'task', '<proposal>', '<title>', '--epic', '<epic>'], dryRun: true },
  { name: 'new-task', args: ['new', 'task', '<proposal>', '<title>', '--epic', '<epic>'] },
  {
    name: 'new-task-depends-on',
    args: ['new', 'task', '<proposal>', '<title>', '--epic', '<epic>', '--depends-on', '<task>'],
  },
  { name: 'new-task-no-proposal', args: ['new', 'task', NO_PROPOSAL, '<title>', '--epic', '<epic>'] },
]

function fill(arg: string, params: Params): string {
  const named: Record<string, string> = {
    '<task>': params.task,
    '<proposal>': params.proposal,
    '<epic>': params.epic,
    '<title>': TITLE,
  }
  return named[arg] ?? arg
}

/** The arguments the pack would give the CLI for a case on this root. */
export function argvOf(root: string, one: Case, params: Params): string[] {
  const args = one.args.map((arg) => fill(arg, params))
  return [...args, '--root', join(root, 'arch'), '--format', 'json', ...(one.dryRun === true ? ['--dry-run'] : [])]
}

/** A run of the CLI that lasts longer than this is cut. The real CLI starts a TypeScript program. */
const RUN_TIMEOUT_MS = 120_000

/** The CLI could not be run at all. Its message is this script's own words and may be printed. */
export class CliFailure extends Error {}

/** Runs one case with `cli`, from `root`. The child is waited for and killed at the deadline. */
export function runCase(cli: string, root: string, one: Case, params: Params, env: NodeJS.ProcessEnv): Ended {
  const run = spawnSync(cli, argvOf(root, one, params), {
    cwd: root,
    env,
    encoding: 'utf8',
    stdio: ['ignore', 'pipe', 'pipe'],
    timeout: RUN_TIMEOUT_MS,
    maxBuffer: 64 * 1024 * 1024,
  })
  if (run.error !== undefined) {
    const code = (run.error as NodeJS.ErrnoException).code ?? 'error'
    throw new CliFailure(`the CLI could not be run for the case ${one.name} (${code})`)
  }
  return { code: run.status, stdout: run.stdout, stderr: run.stderr }
}

/** The shape of one case's run. */
export const shapeOfCase = (cli: string, root: string, one: Case, params: Params, env: NodeJS.ProcessEnv): RunShape =>
  runShape(runCase(cli, root, one, params, env))

/** The status line of the task file `id-<slug>.md` in a proposal's tasks folder of `root/arch`, if there is one. */
export function taskStatus(root: string, id: string): string | null {
  const proposals = join(root, 'arch', 'proposals')
  for (const folder of readdirSync(proposals, { withFileTypes: true })) {
    if (!folder.isDirectory()) continue
    const tasks = join(proposals, folder.name, 'tasks')
    let names: string[]
    try {
      names = readdirSync(tasks)
    } catch {
      continue
    }
    const file = names.find((name) => name.startsWith(`${id}-`) && name.endsWith('.md'))
    if (file === undefined) continue
    return /^status:\s*(\S+)\s*$/m.exec(readFileSync(join(tasks, file), 'utf8'))?.[1] ?? null
  }
  return null
}
