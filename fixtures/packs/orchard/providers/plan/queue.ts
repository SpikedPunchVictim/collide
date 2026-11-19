import { join } from 'node:path'
import type { Collection } from './collections.ts'
import { questionRef } from './ids.ts'
import { runPlan } from './run-plan.ts'

/** The collection of the open questions that need a person. A view reads it by id: `'plan:queue' in collections`. */
export const QUEUE_ID = 'plan:queue'

/** `list` waits this long for the plan CLI's answer. A CLI that takes longer has no queue in that list. */
export const QUEUE_TIMEOUT_MS = 30_000

/** How to run the plan CLI to read the queue. */
export interface QueueOptions {
  env: NodeJS.ProcessEnv
  timeoutMs: number
  log: (message: string) => void
}

/** The queue as keys, in the order the plan CLI gave, and the collection that holds them. */
export interface Queue {
  keys: string[]
  collection: Collection
}

type Json = Record<string, unknown>

/** The question ids of `data.queue`, in order, or why the envelope is not a status envelope with a queue. */
function questionsOf(stdout: string): { ok: true; questions: string[] } | { ok: false; reason: string } {
  let envelope: unknown
  try {
    envelope = JSON.parse(stdout)
  } catch {
    return { ok: false, reason: `its output is not JSON: ${stdout.slice(0, 100)}` }
  }
  const { schema_version: version, command, data } = (envelope ?? {}) as Json
  if (version !== 1) return { ok: false, reason: `its envelope has schema_version ${JSON.stringify(version)}, not 1` }
  if (command !== 'status') return { ok: false, reason: `its envelope is for the command ${JSON.stringify(command)}` }
  const queue = (data as Json | null | undefined)?.queue
  if (!Array.isArray(queue)) return { ok: false, reason: 'its envelope has no queue list' }
  const questions: string[] = []
  for (const entry of queue) {
    const question = (entry as Json | null)?.question
    if (typeof question !== 'string') {
      return { ok: false, reason: `a queue entry names no question: ${JSON.stringify(entry)}` }
    }
    questions.push(question)
  }
  return { ok: true, questions }
}

/**
 * The human queue of a checkout: the questions in the output of `./plan status --format json`, as the plan CLI gave
 * them. The provider has no rule of its own for which question needs a person. A question that the CLI names and the
 * tree lacks is left out, and so is a name that is no question's; each is logged. When the CLI cannot be run, or
 * does not answer as a status envelope, there is no queue: that is logged, and the caller lists without one.
 *
 * `known` is the keys of the questions that were read from files.
 */
export async function readQueue(
  root: string,
  known: ReadonlySet<string>,
  options: QueueOptions,
): Promise<Queue | null> {
  const { env, log } = options
  const timeoutMs = Math.min(options.timeoutMs, QUEUE_TIMEOUT_MS)
  const argv = ['status', '--root', join(root, 'arch'), '--format', 'json']
  let run
  try {
    run = await runPlan(root, argv, env, timeoutMs)
  } catch (error) {
    log(`plan: the queue is not read: ${(error as Error).message}`)
    return null
  }
  if (run.code !== 0) {
    const how = run.code === null ? `ended by ${run.signal}` : `exited with ${run.code}`
    log(`plan: the queue is not read: ./plan status ${how}: ${run.stderr.trim().split('\n')[0] ?? ''}`)
    return null
  }
  const read = questionsOf(run.stdout)
  if (!read.ok) {
    log(`plan: the queue is not read: ./plan status answered, and ${read.reason}`)
    return null
  }
  const keys: string[] = []
  for (const name of read.questions) {
    const shortId = questionRef(name)
    const key = shortId === null ? null : `plan:${shortId}`
    if (key === null || !known.has(key)) {
      log(`plan: the queue names ${name}, which is not a question of this checkout, so it is left out`)
      continue
    }
    if (!keys.includes(key)) keys.push(key)
  }
  return {
    keys,
    collection: { id: QUEUE_ID, title: 'Waiting for a person', groups: [{ name: 'Needs a decision', tasks: keys }] },
  }
}
