import { join } from 'node:path'
import type { TaskInvalidOutput, TaskInput } from '@collide/pack-sdk'
import { readCollections } from './collections.ts'
import type { Collection, Listed } from './collections.ts'
import { filesIn, foldersIn, inBatches } from './files.ts'
import { adrFile, proposalDir, questionFile, spikeDir, taskFile } from './ids.ts'
import type { NameIds, TaskFile } from './ids.ts'
import { ADR, QUESTION, SPIKE, toKindTask } from './kinds.ts'
import type { KindName } from './kinds.ts'
import { QUEUE_ID, readQueue } from './queue.ts'
import type { Queue } from './queue.ts'
import { readCorpusFile } from './read-file.ts'
import type { ReadResult } from './read-file.ts'
import { toTask } from './task.ts'
import type { TaskResult } from './task.ts'

const ARCH = 'arch'
const PROPOSALS = 'arch/proposals'
const SPIKES = 'arch/spikes'
/** The file of a spike's folder that has its frontmatter. */
const SPIKE_FILE = 'PLAN.md'

export interface FoundTask {
  task: TaskInput
  body: string
}

export interface Scan {
  /** Every readable task, ADR, spike and question, in short id order. */
  tasks: FoundTask[]
  collections: Collection[]
  invalid: TaskInvalidOutput[]
}

interface Candidate {
  kind: 'task' | KindName
  /** The two digits of the proposal whose folder holds the file, for a task and for a question. */
  proposal: string | null
  /** The file's path from the checkout, with `/` separators: a spike's is its PLAN.md. */
  path: string
  /** What the file's (or the spike folder's) name says. A task's has the proposal that its id names, too. */
  named: NameIds
}

/**
 * Every file named like a task or a question, in the `tasks` or `questions` folder of a proposal's folder, every ADR
 * at the top of `arch/`, and every spike folder that has a PLAN.md. Anything else is not an item. A link or a folder
 * where a file belongs is not read.
 */
async function candidates(root: string): Promise<Candidate[]> {
  const found: Candidate[] = []
  for (const folder of await foldersIn(join(root, PROPOSALS))) {
    const proposal = proposalDir(folder)
    if (proposal === null) continue
    for (const name of await filesIn(join(root, PROPOSALS, folder, 'tasks'))) {
      const named = taskFile(name)
      if (named !== null) found.push({ kind: 'task', proposal, named, path: `${PROPOSALS}/${folder}/tasks/${name}` })
    }
    for (const name of await filesIn(join(root, PROPOSALS, folder, 'questions'))) {
      const named = questionFile(name)
      if (named !== null) {
        found.push({ kind: QUESTION, proposal, named, path: `${PROPOSALS}/${folder}/questions/${name}` })
      }
    }
  }
  for (const name of await filesIn(join(root, ARCH))) {
    const named = adrFile(name)
    if (named !== null) found.push({ kind: ADR, proposal: null, named, path: `${ARCH}/${name}` })
  }
  for (const folder of await foldersIn(join(root, SPIKES))) {
    const named = spikeDir(folder)
    if (named === null || !(await filesIn(join(root, SPIKES, folder))).includes(SPIKE_FILE)) continue
    found.push({ kind: SPIKE, proposal: null, named, path: `${SPIKES}/${folder}/${SPIKE_FILE}` })
  }
  return found
}

interface Parsed extends Listed {
  shortId: string
  path: string
  body: string
}

/** What a scan may be asked to add: the human queue, which is read by running the checkout's `./plan status`. */
export interface QueueRun {
  env: NodeJS.ProcessEnv
  timeoutMs: number
}

/**
 * Reads one checkout's `arch/` tree: every task and question file under `arch/proposals/<proposal>`, every ADR at the
 * top of `arch/`, every spike's PLAN.md under `arch/spikes`, and every epic under `arch/work/epics`. A file that cannot
 * be read goes to `invalid`, with its path, and does not stop the others; so do two files that have one short id. An
 * epic that cannot be read is logged and left out: it is not an item.
 *
 * With `queue`, and when a question is open, the plan CLI is asked for the human queue, which becomes the collection
 * `plan:queue` and a place in the `collections` of each question in it. Without `queue` no CLI is run: a write, and the
 * watcher, read the files and nothing else.
 */
export async function scan(root: string, log: (message: string) => void, queue?: QueueRun): Promise<Scan> {
  const invalid: TaskInvalidOutput[] = []
  const reject = (path: string, reason: string): void => void invalid.push({ path, checkout: root, reason })

  const found = await candidates(root)
  const spikes = new Set(found.filter(({ kind }) => kind === SPIKE).map(({ named }) => named.shortId))
  const files = await inBatches(found, async (candidate) => ({
    candidate,
    file: await readCorpusFile(join(root, candidate.path)),
  }))

  const byShortId = new Map<string, Parsed[]>()
  for (const { candidate, file } of files) {
    if (!file.ok) {
      reject(candidate.path, file.reason)
      continue
    }
    const { kind, named, path, proposal } = candidate
    const result =
      kind === 'task'
        ? toTask({ file: named as TaskFile, path, checkout: root, ...file }, proposal as string)
        : toItem({ kind, named, path, root, proposal, file }, spikes)
    if (!result.ok) {
      reject(path, result.reason)
      continue
    }
    const parsed: Parsed = { ...result.value, shortId: named.shortId, path, body: file.body }
    byShortId.set(named.shortId, [...(byShortId.get(named.shortId) ?? []), parsed])
  }

  const listed: Parsed[] = []
  for (const [shortId, group] of byShortId) {
    if (group.length === 1) {
      listed.push(group[0] as Parsed)
      continue
    }
    for (const parsed of group) {
      const others = group.filter((other) => other !== parsed).map((other) => other.path)
      reject(parsed.path, `${shortId} is also the id of another file: ${others.join(', ')}`)
    }
  }
  listed.sort((a, b) => (a.shortId < b.shortId ? -1 : 1))

  const { collections, membership } = await readCollections(root, listed, log)
  invalid.sort((a, b) => (a.path < b.path ? -1 : 1))
  const tasks = listed.map(({ task, body }) => ({
    task: { ...task, collections: membership.get(task.key) ?? [] },
    body,
  }))
  const asked = queue === undefined ? null : await readQueueOf(root, listed, log, queue)
  return { ...withQueue({ tasks, collections }, asked), invalid }
}

type Found = Pick<Scan, 'tasks' | 'collections'>

/** What a scan found, and with `asked` the human queue as a collection and a place in each question's `collections`. */
export function withQueue(found: Found, asked: Queue | null): Found {
  if (asked === null) return found
  const queued = new Set(asked.keys)
  const placed = (one: FoundTask): FoundTask => {
    const { task, body } = one
    if (!queued.has(task.key)) return one
    return { task: { ...task, collections: [...(task.collections ?? []), QUEUE_ID] }, body }
  }
  return { tasks: found.tasks.map(placed), collections: [...found.collections, asked.collection] }
}

/** An ADR, a spike or a question read from its file, as the `Listed` that a task is. */
function toItem(
  from: {
    kind: KindName
    named: NameIds
    path: string
    root: string
    proposal: string | null
    file: Extract<ReadResult, { ok: true }>
  },
  spikes: ReadonlySet<string>,
): TaskResult {
  const { kind, named, path, root, proposal, file } = from
  const read = toKindTask(
    {
      kind,
      file: named,
      path,
      checkout: root,
      frontmatter: file.frontmatter,
      version: file.version,
      ...(kind === QUESTION && proposal !== null ? { proposal } : {}),
    },
    spikes,
  )
  return read.ok ? { ok: true, value: { task: read.task, epic: null } } : read
}

/** The questions that are open: the ones whose answer decides what the plan CLI says is queued. */
const openQuestions = (listed: Array<{ task: TaskInput }>) =>
  listed.filter(({ task }) => task.kind === QUESTION && task.state.status === 'open')

/**
 * What the human queue was read for: each open question and its version. The same signature is the same answer, so
 * the plan CLI need not be asked again. It is empty when no question is open.
 */
export const queueSignature = (listed: Array<{ task: TaskInput }>): string =>
  openQuestions(listed)
    .map(({ task }) => `${task.key}@${task.version}`)
    .join('\n')

/** The queue of the checkout, when a question is open and the CLI can say; null otherwise. */
export async function readQueueOf(
  root: string,
  listed: Array<{ task: TaskInput }>,
  log: (message: string) => void,
  queue: QueueRun,
): Promise<Queue | null> {
  if (openQuestions(listed).length === 0) return null
  const questions = listed.filter(({ task }) => task.kind === QUESTION)
  return readQueue(root, new Set(questions.map(({ task }) => task.key)), { ...queue, log })
}

/** The task that an id names: its long id, its short id, or its key. */
export function findTask(tasks: FoundTask[], id: string): FoundTask | undefined {
  const wanted = id.startsWith('plan:') ? id.slice('plan:'.length) : id
  return tasks.find(({ task }) => task.id === wanted || task.key === `plan:${wanted}`)
}
