import { join } from 'node:path'
import type { ApplyParamsOutput, ApplyResultInput, PreviewResultInput, TaskInput } from '@collide/pack-sdk'
import { CATEGORY_STATUS, STATUSES, TITLE_MAX } from './declaration.ts'
import { EPIC_ARG, PROPOSAL_ARG, SHORT_ID_PATTERN } from './ids.ts'
import { runPlan } from './run-plan.ts'
import type { PlanRun } from './run-plan.ts'
import { findTask, scan } from './scan.ts'
import type { FoundTask } from './scan.ts'
import { changedPaths, hashArch } from './tree-hash.ts'

/** What a write needs: the checkout's root, the environment and time limit of the plan CLI, and where to log. */
export interface WriteContext {
  root: string
  env: NodeJS.ProcessEnv
  timeoutMs: number
  log: (message: string) => void
}

type Ok = Extract<ApplyResultInput, { outcome: 'ok' }>
type Effect = Ok['items'][number]
type State = TaskInput['state']
type Json = Record<string, unknown>

/** The outcome of a write or of a preview of it. `task` is the task as written, and is null for a preview. */
type Done =
  | { kind: 'ok'; files: string[]; items: Effect[]; task: TaskInput | null }
  | { kind: 'conflict'; current: TaskInput }
  | { kind: 'refused'; message: string }

const refused = (message: string): Done => ({ kind: 'refused', message })
const withBody = ({ task, body }: FoundTask): TaskInput => ({ ...task, body })
const shortIdOf = (task: TaskInput): string => task.key.slice('plan:'.length)

// ---------------------------------------------------------------- the call

interface SetStatusCall {
  op: 'setStatus'
  id: string
  version: string
  /** What the call names: a status, or a category that stands for one. */
  status?: string
  category?: string
}

interface CreateCall {
  op: 'create'
  proposal: string
  title: string
  epic: string
  dependsOn: string[]
}

/** The caller built a call that the op's argument schema does not allow. It is an error reply, not a refusal. */
const badCall = (message: string): never => {
  throw new Error(message)
}

const text = (value: unknown, what: string): string =>
  typeof value === 'string' && value !== '' ? value : badCall(`${what} must be a non-empty string`)

function onlyKeys(args: Json, allowed: string[]): void {
  const extra = Object.keys(args).find((key) => !allowed.includes(key))
  if (extra !== undefined) badCall(`unknown argument "${extra}"; this op takes ${allowed.join(', ')}`)
}

function parseSetStatus({ id, version, args }: ApplyParamsOutput): SetStatusCall {
  onlyKeys(args, ['status', 'category'])
  const named = ['status', 'category'].filter((key) => args[key] !== undefined)
  if (named.length !== 1) badCall('setStatus takes one of status or category')
  const call: SetStatusCall = {
    op: 'setStatus',
    id: text(id, 'setStatus needs the id of the task, and it'),
    version: text(
      version,
      'setStatus needs the version you last read, so that a write on a changed task conflicts; it',
    ),
  }
  if (args.status !== undefined) {
    call.status = text(args.status, 'status')
    if (!STATUSES.some((status) => status.name === call.status)) badCall(`status "${call.status}" is not declared`)
  } else {
    call.category = text(args.category, 'category')
  }
  return call
}

function parseCreate({ id, version, args }: ApplyParamsOutput): CreateCall {
  if (id !== undefined && id !== null) badCall('create makes the id: leave id out')
  if (version !== undefined && version !== null) badCall('create takes no version: it changes no task')
  onlyKeys(args, ['proposal', 'title', 'epic', 'dependsOn'])
  const proposal = text(args.proposal, 'proposal')
  const title = text(args.title, 'title')
  const epic = text(args.epic, 'epic')
  if (!new RegExp(PROPOSAL_ARG).test(proposal)) badCall(`proposal "${proposal}" must be P and two digits`)
  if (!new RegExp(EPIC_ARG).test(epic)) badCall(`epic "${epic}" must be an epic id such as E-07`)
  // A title that starts with a dash would be read as a flag by the plan CLI.
  if (title.length > TITLE_MAX || title.startsWith('-') || /[\u0000-\u001f\u007f]/.test(title)) {
    badCall(`title must be one line of at most ${TITLE_MAX} characters, not starting with a dash`)
  }
  const dependsOn = args.dependsOn ?? []
  const shortId = new RegExp(SHORT_ID_PATTERN)
  if (!Array.isArray(dependsOn) || !dependsOn.every((entry) => typeof entry === 'string' && shortId.test(entry))) {
    badCall('dependsOn must be a list of task ids such as T-03-001')
  }
  return {
    op: 'create',
    proposal,
    title,
    epic,
    dependsOn: dependsOn as string[],
  }
}

function parseCall(params: ApplyParamsOutput): SetStatusCall | CreateCall {
  if (params.op === 'setStatus') return parseSetStatus(params)
  if (params.op === 'create') return parseCreate(params)
  return badCall(`this source has no op "${params.op}"`)
}

// ---------------------------------------------------------------- running the plan CLI

/**
 * What a run of the plan CLI left: either the envelope's data and the files it touched, or the message that refuses
 * the call.
 */
type Executed = { kind: 'done'; data: Json; files: string[] } | { kind: 'refused'; message: string }

const firstLine = (stderr: string): string => stderr.trim().split('\n')[0] ?? ''

function parseEnvelope(stdout: string, command: string): Json {
  let envelope: unknown
  try {
    envelope = JSON.parse(stdout)
  } catch {
    return badCall(`the output of ./plan is not JSON: ${stdout.slice(0, 200)}`)
  }
  const { schema_version: version, command: named, data } = (envelope ?? {}) as Json
  if (version !== 1) badCall(`the envelope of ./plan has schema_version ${JSON.stringify(version)}; this reads 1`)
  if (named !== command) badCall(`the envelope of ./plan is for the command ${JSON.stringify(named)}, not ${command}`)
  if (typeof data !== 'object' || data === null || Array.isArray(data)) badCall('the envelope of ./plan has no data')
  return data as Json
}

/**
 * The write gate's refusal: the first error in the envelope of an exit 1. None when there is no envelope with errors,
 * which is the CLI failing (it prints `Internal error: ...` on stderr and exits 1), not its gate.
 */
function gateRefusal(run: PlanRun, command: string): string | undefined {
  let errors: unknown
  try {
    errors = parseEnvelope(run.stdout, command).errors
  } catch {
    return undefined
  }
  if (!Array.isArray(errors) || errors.length === 0) return undefined
  const { file, message } = errors[0] as Json
  const more = errors.length > 1 ? ` (and ${errors.length - 1} more)` : ''
  return `${typeof file === 'string' ? `${file}: ` : ''}${String(message)}${more}`
}

/**
 * The files that a dry run reports: those it would write and the tables it would refresh, under the checkout. The CLI
 * lists an edited file even when every change in it is to the value it already has, as `set` does to the status it
 * is given, and a write of that leaves the file as it was. Such a file is left out, so that a preview lists what the
 * write will touch.
 */
function reportedFiles(data: Json): string[] {
  const changed = (result: Json): boolean =>
    !Array.isArray(result.changes) || (result.changes as Json[]).some((change) => change.oldValue !== change.newValue)
  const results = Array.isArray(data.results) ? (data.results as Json[]).filter(changed).map((r) => r.relPath) : []
  const named = [
    ...results,
    ...(typeof data.relPath === 'string' ? [data.relPath] : []),
    ...((data.regenerated as unknown[]) ?? []),
  ]
  const files = named.map((path) => {
    if (typeof path !== 'string' || path === '' || path.startsWith('/') || path.split('/').includes('..')) {
      return badCall(`./plan reported a file that is not under arch/: ${JSON.stringify(path)}`)
    }
    return `arch/${path}`
  })
  return [...new Set(files)].sort()
}

/**
 * Runs `./plan <args> --root <root>/arch --format json`, with `--dry-run` for a preview. The command comes first: the
 * plan CLI is an oclif CLI, which reads its first argument as the command. The files a write touched are
 * those under `arch/` whose hash changed, whatever the CLI says it wrote; a preview's files are the CLI's own list,
 * and a preview that changed anything is an error.
 *
 * Exit 0 is done. Exit 1 with the write gate's errors in its envelope is a refusal with the first of them, and exit 2
 * is the CLI declining a call the adapter built well: an illegal transition is exit 2 in the plan CLI, as are a task
 * that is not there and a proposal that is not; it is a refusal with the CLI's message. Exit 1 with no such envelope
 * (the CLI failed), any other exit, a CLI that cannot be run, and a failure that changed a file are errors: the
 * adapter cannot say what state the checkout is in.
 */
async function execute(ctx: WriteContext, args: string[], command: string, dryRun: boolean): Promise<Executed> {
  const before = await hashArch(ctx.root)
  const argv = [...args, '--root', join(ctx.root, 'arch'), '--format', 'json', ...(dryRun ? ['--dry-run'] : [])]
  const run = await runPlan(ctx.root, argv, ctx.env, ctx.timeoutMs)
  const touched = changedPaths(before, await hashArch(ctx.root))

  if (dryRun && touched.length > 0) badCall(`./plan --dry-run changed ${touched.join(', ')}`)
  if (run.code !== 0 && touched.length > 0)
    badCall(`./plan ended (${run.code ?? run.signal}) after changing ${touched.join(', ')}`)
  if (run.code === 0) {
    const data = parseEnvelope(run.stdout, command)
    if (Array.isArray(data.errors) && data.errors.length > 0) badCall('./plan exited 0 and reported errors')
    return {
      kind: 'done',
      data,
      files: dryRun ? reportedFiles(data) : touched,
    }
  }
  if (run.code === 2) return { kind: 'refused', message: run.stderr.trim() || './plan exited with 2 and said nothing' }
  const gate = run.code === 1 ? gateRefusal(run, command) : undefined
  if (gate !== undefined) return { kind: 'refused', message: gate }
  const how = run.code === null ? `ended by ${run.signal}` : `exited with ${run.code}`
  return badCall(`./plan ${how}: ${firstLine(run.stderr)}`)
}

// ---------------------------------------------------------------- the ops

function stateOf(status: unknown): State {
  const declared = STATUSES.find((candidate) => candidate.name === status)
  if (declared === undefined)
    return badCall(`./plan reported the status ${JSON.stringify(status)}, which is not declared`)
  return { status: declared.name, category: declared.category }
}

/** The status that a call writes: the one it names, or the one that its category stands for. */
function statusFor(call: SetStatusCall): { status: string } | { refusal: string } {
  if (call.status !== undefined) return { status: call.status }
  const category = call.category as string
  const status = Object.hasOwn(CATEGORY_STATUS, category) ? CATEGORY_STATUS[category] : undefined
  return status === undefined ? { refusal: `no status of this source writes the category "${category}"` } : { status }
}

async function setStatus(ctx: WriteContext, call: SetStatusCall, dryRun: boolean): Promise<Done> {
  const wanted = statusFor(call)
  if ('refusal' in wanted) return refused(wanted.refusal)
  const found = findTask((await scan(ctx.root, ctx.log)).tasks, call.id)
  if (found === undefined) return refused(`there is no task ${call.id} in this checkout`)
  if (call.version !== found.task.version) return { kind: 'conflict', current: withBody(found) }

  const shortId = shortIdOf(found.task)
  const run = await execute(ctx, ['set', shortId, `status=${wanted.status}`], 'set', dryRun)
  if (run.kind === 'refused') return refused(run.message)
  const from = found.task.state
  if (dryRun) {
    // A dry run writes no file to hash, so the version is the one the task has now.
    const item = {
      key: found.task.key,
      version: found.task.version,
      from,
      to: stateOf(wanted.status),
    }
    return { kind: 'ok', files: run.files, items: [item], task: null }
  }
  const written = findTask((await scan(ctx.root, ctx.log)).tasks, shortId)
  if (written === undefined) return badCall(`./plan set ${shortId}, and the task cannot be read afterwards`)
  const item = {
    key: written.task.key,
    version: written.task.version,
    from,
    to: written.task.state,
  }
  return {
    kind: 'ok',
    files: run.files,
    items: [item],
    task: withBody(written),
  }
}

async function create(ctx: WriteContext, call: CreateCall, dryRun: boolean): Promise<Done> {
  const depends = call.dependsOn.length === 0 ? [] : ['--depends-on', call.dependsOn.join(',')]
  const args = ['new', 'task', call.proposal, call.title, '--epic', call.epic, ...depends]
  const run = await execute(ctx, args, 'new-task', dryRun)
  if (run.kind === 'refused') return refused(run.message)

  // The CLI allocated the id: its envelope names it by proposal and number.
  const { id, frontmatter } = run.data as { id?: Json; frontmatter?: Json }
  const shortId = `T-${String(id?.scope)}-${String(id?.num)}`
  if (!new RegExp(SHORT_ID_PATTERN).test(shortId)) badCall(`./plan allocated the id ${JSON.stringify(id)}`)
  const key = `plan:${shortId}`
  if (dryRun) {
    // The task has no file yet, so it has no version.
    const item = {
      key,
      version: '',
      from: null,
      to: stateOf(frontmatter?.status),
    }
    return { kind: 'ok', files: run.files, items: [item], task: null }
  }
  const written = findTask((await scan(ctx.root, ctx.log)).tasks, shortId)
  if (written === undefined) return badCall(`./plan made ${shortId}, and the task cannot be read afterwards`)
  const item = {
    key,
    version: written.task.version,
    from: null,
    to: written.task.state,
  }
  return {
    kind: 'ok',
    files: run.files,
    items: [item],
    task: withBody(written),
  }
}

async function write(ctx: WriteContext, params: ApplyParamsOutput, dryRun: boolean): Promise<Done> {
  const call = parseCall(params)
  return call.op === 'setStatus' ? setStatus(ctx, call, dryRun) : create(ctx, call, dryRun)
}

/** `apply`: runs the op with the plan CLI and says what it did. Nothing is committed. */
export async function applyOp(ctx: WriteContext, params: ApplyParamsOutput): Promise<ApplyResultInput> {
  const done = await write(ctx, params, false)
  if (done.kind === 'conflict') return { outcome: 'conflict', current: done.current }
  if (done.kind === 'refused') return { outcome: 'refused', message: done.message }
  if (done.task === null) return badCall('a write gave no task')
  return {
    outcome: 'ok',
    task: done.task,
    version: done.task.version,
    files: done.files,
    items: done.items,
  }
}

/** `preview`: the same call with `--dry-run`. The same three outcomes, and nothing is written. */
export async function previewOp(ctx: WriteContext, params: ApplyParamsOutput): Promise<PreviewResultInput> {
  const done = await write(ctx, params, true)
  if (done.kind === 'conflict') return { outcome: 'conflict', current: done.current }
  if (done.kind === 'refused') return { outcome: 'refused', message: done.message }
  return { outcome: 'ok', files: done.files, items: done.items }
}
