import { join } from 'node:path'
import type { ListResultInput, TaskInput } from '@collide/pack-sdk'
import { readEpicTables } from './epic.ts'
import { epicFile } from './ids.ts'
import { readCorpusFile } from './read-file.ts'
import { filesIn } from './files.ts'

export const EPICS = 'arch/work/epics'

export type Collection = NonNullable<ListResultInput['collections']>[number]

/** A task, and the epic its own frontmatter names. */
export interface Listed {
  task: TaskInput
  epic: string | null
}

const keyOf = (shortId: string): string => `plan:${shortId}`

/**
 * Each epic file as a collection, in epic id order, and for each task the epics that hold it. A task is in an epic when
 * its own `epic:` says so or when the epic's tables name it, and only an epic that has a file counts. An epic file that
 * cannot be read, or has the id of one already read, is logged and skipped.
 */
export async function readCollections(
  root: string,
  listed: Listed[],
  log: (message: string) => void,
): Promise<{ collections: Collection[]; membership: Map<string, string[]> }> {
  const known = new Set(listed.map(({ task }) => task.key))
  const collections: Collection[] = []
  const seen = new Set<string>()
  const membership = new Map<string, string[]>()
  const hold = (taskKey: string, collectionId: string): void => {
    membership.set(taskKey, [...(membership.get(taskKey) ?? []), collectionId])
  }

  for (const name of await filesIn(join(root, EPICS))) {
    const shortId = epicFile(name)
    if (shortId === null) continue
    const path = `${EPICS}/${name}`
    const file = await readCorpusFile(join(root, path))
    if (!file.ok) {
      log(`plan: epic ${path} skipped: ${file.reason}`)
      continue
    }
    const { title } = file.frontmatter
    if (typeof title !== 'string' || title.trim() === '') {
      log(`plan: epic ${path} skipped: title must be a non-empty string`)
      continue
    }
    if (seen.has(shortId)) {
      log(`plan: epic ${path} skipped: ${shortId} is the id of another epic file`)
      continue
    }
    seen.add(shortId)

    const tables = readEpicTables(file.body)
    const id = keyOf(shortId)
    const members = new Set(tables.members.map(keyOf))
    for (const { task, epic } of listed) if (epic === shortId) members.add(task.key)
    for (const { task } of listed) if (members.has(task.key)) hold(task.key, id)
    collections.push({
      id,
      title,
      groups: tables.groups.map((group) => ({
        name: group.name,
        tasks: group.shortIds.map(keyOf).filter((key) => known.has(key)),
      })),
    })
  }
  return { collections, membership }
}
