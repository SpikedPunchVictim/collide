import type { ProviderNotifications, TaskChangedOutput, TaskInput } from '@collide/pack-sdk'
import type { Collection } from './collections.ts'
import { BLOCKED_BY } from './declaration.ts'
import type { GitRename } from './git.ts'
import { renamedTo as renamedByMap } from './id-map.ts'
import type { IdMapRow } from './id-map.ts'
import type { Queue } from './queue.ts'

/** What one scan of a checkout found: its tasks by short id, its invalid files by path, and where git had HEAD. */
export interface Snapshot {
  /** The commit HEAD named when the files were read, or null when git could not say. */
  head: string | null
  tasks: Map<string, TaskInput>
  /** The reason for each file that is not a task, by its path from the checkout. */
  invalid: Map<string, string>
  idMap: IdMapRow[]
  /** The collections as `list` gives them: the epics, and the human queue when it could be read. */
  collections: Collection[]
  /** What the human queue was read for (see `queueSignature`), so that the same questions are not asked about twice. */
  asked: string
  /** What the plan CLI said the queue was, or null when no question was open or it could not say. */
  queue: Queue | null
}

export const emptySnapshot = (): Snapshot => ({
  head: null,
  tasks: new Map(),
  invalid: new Map(),
  idMap: [],
  collections: [],
  asked: '',
  queue: null,
})

/** One notification of `watch`, with its method. */
export type Notification = {
  [M in keyof ProviderNotifications]: { method: M; params: ProviderNotifications[M] }
}[keyof ProviderNotifications]

type Diff = NonNullable<TaskChangedOutput['diff']>

const keyOf = (shortId: string): string => `plan:${shortId}`
const same = (a: unknown, b: unknown): boolean => JSON.stringify(a) === JSON.stringify(b)

const dependsOn = (task: TaskInput | null): string[] =>
  (task?.links ?? []).filter((link) => link.type === BLOCKED_BY).map((link) => link.to)

/**
 * The fields that differ between two reads of a task, or between nothing and its first read. The status, the title,
 * each of the task's own fields and its path are `{from, to}`; its dependencies are `{added, removed}`.
 */
export function diffOf(before: TaskInput | null, after: TaskInput): Diff {
  const diff: Diff = {}
  const field = (name: string, from: unknown, to: unknown): void => {
    if (!same(from, to)) diff[name] = { from, to }
  }
  field('status', before?.state.status ?? null, after.state.status)
  field('title', before?.title ?? null, after.title)
  const names = new Set([...Object.keys(before?.fields ?? {}), ...Object.keys(after.fields ?? {})])
  for (const name of [...names].sort()) field(name, before?.fields?.[name] ?? null, after.fields?.[name] ?? null)
  const was = dependsOn(before)
  const now = dependsOn(after)
  const added = now.filter((id) => !was.includes(id))
  const removed = was.filter((id) => !now.includes(id))
  if (added.length > 0 || removed.length > 0) diff.depends_on = { added, removed }
  field('path', before?.path ?? null, after.path ?? null)
  return diff
}

/** The short ids that a checkout had and no longer has, and the ones it did not have. */
export function idsChanged(before: Snapshot, after: Snapshot): { gone: string[]; fresh: string[] } {
  return {
    gone: [...before.tasks.keys()].filter((id) => !after.tasks.has(id)),
    fresh: [...after.tasks.keys()].filter((id) => !before.tasks.has(id)),
  }
}

/**
 * What changed in one checkout between two snapshots, as notifications.
 * - A task whose short id went and another came is a rename only when the id map says so (its rows, followed to their
 *   end) or git says its file was renamed between the two HEADs. Otherwise it is a removal and a new task, never a
 *   guess.
 * - A task that became unparseable is reported as invalid, and not as removed: core takes it out of its list itself.
 * - `cause` is `head` when HEAD moved between the two reads, else `worktree`.
 */
export function compare(
  checkout: string,
  before: Snapshot,
  after: Snapshot,
  gitRenames: GitRename[] = [],
): Notification[] {
  const cause: TaskChangedOutput['cause'] = before.head !== after.head ? 'head' : 'worktree'
  const { gone, fresh } = idsChanged(before, after)
  const unplaced = new Set(fresh)

  const renamed = new Map<string, string>()
  for (const id of gone) {
    const target = renamedByMap(after.idMap, id)
    if (target !== null && unplaced.has(target)) {
      renamed.set(id, target)
      unplaced.delete(target)
    }
  }
  for (const id of gone) {
    if (renamed.has(id)) continue
    const moved = gitRenames.find((rename) => rename.from === before.tasks.get(id)?.path)
    if (moved === undefined) continue
    const target = [...unplaced].find((candidate) => after.tasks.get(candidate)?.path === moved.to)
    if (target !== undefined) {
      renamed.set(id, target)
      unplaced.delete(target)
    }
  }

  const out: Notification[] = []
  const changed = (from: TaskInput | null, to: TaskInput, diff: Diff): void => {
    out.push({
      method: 'task.changed',
      params: { key: to.key, version: to.version, from: from?.state ?? null, to: to.state, diff, cause, checkout },
    })
  }

  for (const [from, to] of renamed) {
    out.push({ method: 'task.renamed', params: { from: keyOf(from), to: keyOf(to), checkout } })
    const { path: _moved, ...diff } = diffOf(before.tasks.get(from) ?? null, after.tasks.get(to) as TaskInput)
    if (Object.keys(diff).length > 0) changed(before.tasks.get(from) ?? null, after.tasks.get(to) as TaskInput, diff)
  }

  // A file that is gone with HEAD where it was is a deletion; once HEAD moved, the adapter does not say why.
  const removedCause = before.head !== null && before.head === after.head ? 'deleted' : 'unknown'
  const invalidPaths = new Set(after.invalid.keys())
  for (const id of gone) {
    const task = before.tasks.get(id) as TaskInput
    if (renamed.has(id) || invalidPaths.has(task.path ?? '')) continue
    out.push({ method: 'task.removed', params: { key: keyOf(id), cause: removedCause, checkout } })
  }

  const renamedTo = new Set(renamed.values())
  for (const [id, task] of after.tasks) {
    const was = before.tasks.get(id) ?? null
    if (renamedTo.has(id)) continue
    if (was !== null && was.version === task.version && was.path === task.path) continue
    changed(was, task, diffOf(was, task))
  }

  for (const [path, reason] of after.invalid) {
    if (before.invalid.get(path) !== reason) out.push({ method: 'task.invalid', params: { path, checkout, reason } })
  }
  return out
}

/**
 * What the checkout's collections, and the places of its tasks in them, changed to between two snapshots, as one
 * notification; none when they did not change. A task is named when it is in both snapshots and its places differ. A
 * task that is new or renamed is left to its own notification, which makes core read it.
 */
export function collectionsChanged(checkout: string, before: Snapshot, after: Snapshot): Notification[] {
  const members: Array<{ key: string; collections: string[] }> = []
  for (const [id, task] of after.tasks) {
    const was = before.tasks.get(id)
    if (was !== undefined && !same(was.collections ?? [], task.collections ?? [])) {
      members.push({ key: task.key, collections: task.collections ?? [] })
    }
  }
  if (members.length === 0 && same(before.collections, after.collections)) return []
  return [{ method: 'collections.changed', params: { collections: after.collections, members, checkout } }]
}

/** One short id found at more than one path across checkouts. */
export interface CollisionFound {
  shortId: string
  locations: Array<{ checkout: string; path: string }>
}

/**
 * The short ids that sit at two or more different paths across the checkouts' snapshots. The same path in two checkouts
 * is one task, and one checkout never collides with itself: its own duplicates are invalid files.
 */
export function collisionsOf(snapshots: Map<string, Snapshot>): CollisionFound[] {
  const byId = new Map<string, Array<{ checkout: string; path: string }>>()
  for (const [checkout, snapshot] of snapshots) {
    for (const [shortId, task] of snapshot.tasks) {
      byId.set(shortId, [...(byId.get(shortId) ?? []), { checkout, path: task.path ?? '' }])
    }
  }
  const found: CollisionFound[] = []
  for (const [shortId, locations] of [...byId].sort(([a], [b]) => (a < b ? -1 : 1))) {
    if (new Set(locations.map((location) => location.path)).size < 2) continue
    locations.sort((a, b) => (a.checkout + a.path < b.checkout + b.path ? -1 : 1))
    found.push({ shortId, locations })
  }
  return found
}
