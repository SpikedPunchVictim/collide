import { join } from 'node:path'
import type { TaskInvalidOutput, TaskInput } from '@collide/pack-sdk'
import { readCollections } from './collections.ts'
import type { Collection, Listed } from './collections.ts'
import { filesIn, foldersIn } from './files.ts'
import { proposalDir, taskFile } from './ids.ts'
import { readCorpusFile } from './read-file.ts'
import { toTask } from './task.ts'

const PROPOSALS = 'arch/proposals'

/** How many files are open at once. */
const BATCH = 32

export interface FoundTask {
  task: TaskInput
  body: string
}

export interface Scan {
  /** Every readable task, in short id order. */
  tasks: FoundTask[]
  collections: Collection[]
  invalid: TaskInvalidOutput[]
}

interface Candidate {
  proposal: string
  path: string
  name: string
}

/** Every file named like a task, in a `tasks` folder of a proposal's folder. Anything else is not a task. */
async function candidates(root: string): Promise<Candidate[]> {
  const found: Candidate[] = []
  for (const folder of await foldersIn(join(root, PROPOSALS))) {
    const proposal = proposalDir(folder)
    if (proposal === null) continue
    for (const name of await filesIn(join(root, PROPOSALS, folder, 'tasks'))) {
      if (taskFile(name) !== null) found.push({ proposal, name, path: `${PROPOSALS}/${folder}/tasks/${name}` })
    }
  }
  return found
}

/** Runs `work` over `items` a batch at a time, and returns the results in order. */
async function inBatches<T, R>(items: T[], work: (item: T) => Promise<R>): Promise<R[]> {
  const results: R[] = []
  for (let at = 0; at < items.length; at += BATCH) {
    results.push(...(await Promise.all(items.slice(at, at + BATCH).map(work))))
  }
  return results
}

interface Parsed extends Listed {
  shortId: string
  path: string
  body: string
}

/**
 * Reads one checkout's `arch/` tree: every task file under `arch/proposals/<proposal>/tasks`, and every epic under
 * `arch/work/epics`. A task file that cannot be read goes to `invalid`, with its path, and does not stop the others;
 * so do two files that have one short id. An epic that cannot be read is logged and left out: it is not a task.
 */
export async function scan(root: string, log: (message: string) => void): Promise<Scan> {
  const invalid: TaskInvalidOutput[] = []
  const reject = (path: string, reason: string): void => void invalid.push({ path, checkout: root, reason })

  const files = await inBatches(await candidates(root), async (candidate) => ({
    candidate,
    file: await readCorpusFile(join(root, candidate.path)),
  }))

  const byShortId = new Map<string, Parsed[]>()
  for (const { candidate, file } of files) {
    if (!file.ok) {
      reject(candidate.path, file.reason)
      continue
    }
    const named = taskFile(candidate.name)
    if (named === null) continue
    const result = toTask({ file: named, path: candidate.path, checkout: root, ...file }, candidate.proposal)
    if (!result.ok) {
      reject(candidate.path, result.reason)
      continue
    }
    const parsed: Parsed = { ...result.value, shortId: named.shortId, path: candidate.path, body: file.body }
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
  return { tasks, collections, invalid }
}
