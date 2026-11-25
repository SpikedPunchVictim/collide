import type {
  ListResultInput,
  ProviderNotifications,
  TaskChangedOutput,
  TaskInput,
  TaskInvalidOutput,
} from '@collide/pack-sdk'
import type { GitRename } from './git.ts'

/** What one scan of a checkout found: its tasks by id, its invalid files by path, and where git had HEAD. */
export interface Snapshot {
  /** The commit HEAD named when the files were read, or null when git could not say. */
  head: string | null
  /** The items by id, as `list` gives them. */
  tasks: Map<string, TaskInput>
  /** The reason for each file that is not an item, by its path from the checkout. */
  invalid: Map<string, string>
  /** The collections as `list` gives them: the epics. */
  collections: Collection[]
}

type Collection = NonNullable<ListResultInput['collections']>[number]

export const emptySnapshot = (): Snapshot => ({ head: null, tasks: new Map(), invalid: new Map(), collections: [] })

/** The snapshot of what a `list` answered (or a read found), taken with HEAD as it was around that read. */
export const snapshotOf = (
  head: string | null,
  found: { tasks: TaskInput[]; collections?: Collection[]; invalid: TaskInvalidOutput[] },
): Snapshot => ({
  head,
  tasks: new Map(found.tasks.map((task) => [task.id, task])),
  invalid: new Map(found.invalid.map((item) => [item.path, item.reason])),
  collections: found.collections ?? [],
})

/** One notification of `watch`, with its method. */
export type Notification = {
  [M in keyof ProviderNotifications]: { method: M; params: ProviderNotifications[M] }
}[keyof ProviderNotifications]

type Diff = NonNullable<TaskChangedOutput['diff']>

const same = (a: unknown, b: unknown): boolean => JSON.stringify(a) === JSON.stringify(b)

/** Fields of an item that are not a field of the diff: the key and id name it, the version is in the notice. */
const NOT_DIFFED = new Set(['key', 'id', 'version', 'state'])

/**
 * The fields that differ between two reads of an item, or between nothing and its first read. Core reads the item
 * again when a notice names a field other than the status, so every field of the item that differs is named: the
 * status as `status`, each of the item's own fields as `fields.<name>`, and every other field (the title, the links,
 * the path, the features, the coverage and the rest) by its own name, as `{from, to}`.
 */
export function diffOf(before: TaskInput | null, after: TaskInput): Diff {
  const diff: Diff = {}
  const field = (name: string, from: unknown, to: unknown): void => {
    if (!same(from, to)) diff[name] = { from, to }
  }
  field('status', before?.state.status ?? null, after.state.status)
  const own = (task: TaskInput | null): Record<string, unknown> => task?.fields ?? {}
  for (const name of [...new Set([...Object.keys(own(before)), ...Object.keys(own(after))])].sort()) {
    field(`fields.${name}`, own(before)[name] ?? null, own(after)[name] ?? null)
  }
  const record = (task: TaskInput | null): Record<string, unknown> => (task ?? {}) as Record<string, unknown>
  const names = new Set([...Object.keys(record(before)), ...Object.keys(record(after))])
  for (const name of [...names].sort()) {
    if (NOT_DIFFED.has(name) || name === 'fields') continue
    field(name, record(before)[name] ?? null, record(after)[name] ?? null)
  }
  return diff
}

/** The ids that a checkout had and no longer has, and the ones it did not have. */
export function idsChanged(before: Snapshot, after: Snapshot): { gone: string[]; fresh: string[] } {
  return {
    gone: [...before.tasks.keys()].filter((id) => !after.tasks.has(id)),
    fresh: [...after.tasks.keys()].filter((id) => !before.tasks.has(id)),
  }
}

const keyOf = (id: string): string => `plan:${id}`

/**
 * What changed in one checkout between two snapshots, as notifications.
 * - An item whose id went while another came is a rename only when git says its file was renamed between the two
 *   HEADs. The plan keeps no id map, so otherwise it is a removal and a new item, never a guess.
 * - A file that became unreadable is reported as invalid, and not as removed: core takes it out of its list itself.
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

  // A file that is gone with HEAD where it was is a deletion; once HEAD moved, the plan does not say why.
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
    if (was !== null && same(was, task)) continue
    changed(was, task, diffOf(was, task))
  }

  for (const [path, reason] of after.invalid) {
    if (before.invalid.get(path) !== reason) out.push({ method: 'task.invalid', params: { path, checkout, reason } })
  }
  return out
}

/**
 * What the checkout's collections, and the places of its items in them, changed to between two snapshots, as one
 * notification; none when they did not change. An item is named when it is in both snapshots and its collections
 * differ. An item that is new or renamed is left to its own notification, which makes core read it.
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
