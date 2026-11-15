import { readdir } from 'node:fs/promises'
import { join, relative, sep } from 'node:path'
import type { TaskInput, TaskInvalidOutput } from '@collide/pack-sdk'
import { parseFeatures } from './features.ts'
import { KNOWN_KINDS, PROPOSAL, readItem, stateOf } from './item.ts'
import type { Item } from './item.ts'
import { readItemFile } from './read-file.ts'

/** The shared root's default path, relative to the repository root (0.10). */
export const SHARED_ROOT = '.collide/planning'

const EPIC_FOLDER = /^(ep-[0-9]{3,}-[a-z]{2})(-.*)?$/

/** One item that was read and checked, with where it was found. */
export interface FoundItem {
  item: Item
  epic: string | null
  path: string
  body: string
  /** The line of the file that `body` starts on. */
  bodyLine: number
  version: string
}

export interface Scan {
  items: FoundItem[]
  invalid: TaskInvalidOutput[]
  /** Items whose kind is not `task`, `epic` or `proposal`. They are listed, not dropped. */
  unknownKind: number
  /** Items with a link whose target is no item that was read. They are listed, not dropped. */
  brokenLink: number
}

const posix = (cwd: string, absolute: string): string => relative(cwd, absolute).split(sep).join('/')

/** The entries of a directory, sorted by name; a directory that is not there has none. */
async function entries(dir: string) {
  try {
    return (await readdir(dir, { withFileTypes: true })).sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT' || (error as NodeJS.ErrnoException).code === 'ENOTDIR') return []
    throw error
  }
}

/**
 * Reads the items under `<cwd>/.collide/planning/epics/`: each epic folder's `epic.md` and the `.md` files directly in
 * its `tasks/` and `proposals/` folders, and nothing else. Symbolic links are not followed. `spikes/` and `questions/`
 * are never opened. The epic is the folder, so it is reported from the path and never from frontmatter. Every file
 * that reads is returned, in scan order, including a second file with an id another file has (`scanEpics` sets that
 * one aside); `invalid` holds the files that did not read.
 */
export async function readEpicFiles(cwd: string, checkout: string | null = null): Promise<{ items: FoundItem[]; invalid: TaskInvalidOutput[] }> {
  const epicsDir = join(cwd, SHARED_ROOT, 'epics')
  const items: FoundItem[] = []
  const invalid: TaskInvalidOutput[] = []
  const reject = (absolute: string, reason: string): void => void invalid.push({ path: posix(cwd, absolute), checkout, reason })

  const readOne = async (absolute: string, epic: string | null): Promise<void> => {
    const read = await readItemFile(absolute)
    if (!read.ok) return reject(absolute, read.reason)
    const checked = readItem(read.frontmatter)
    if (!checked.ok) return reject(absolute, checked.reason)
    items.push({ item: checked.item, epic, path: posix(cwd, absolute), body: read.body, bodyLine: read.bodyLine, version: read.version })
  }

  for (const folder of (await entries(epicsDir)).filter((entry) => entry.isDirectory())) {
    const epicDir = join(epicsDir, folder.name)
    const epic = EPIC_FOLDER.exec(folder.name)?.[1]
    if (epic === undefined) {
      reject(epicDir, `epic folder "${folder.name}" must start with an epic id such as ep-002-va`)
      continue
    }
    for (const file of (await entries(epicDir)).filter((entry) => entry.isFile() && entry.name === 'epic.md')) {
      await readOne(join(epicDir, file.name), null)
    }
    for (const subfolder of ['tasks', 'proposals']) {
      for (const file of (await entries(join(epicDir, subfolder))).filter((entry) => entry.isFile() && entry.name.endsWith('.md'))) {
        await readOne(join(epicDir, subfolder, file.name), epic)
      }
    }
  }

  return { items, invalid }
}

/** The items under `epics/` (see `readEpicFiles`). */
export async function scanEpics(cwd: string, checkout: string | null = null): Promise<Scan> {
  const { items, invalid } = await readEpicFiles(cwd, checkout)
  // Two files with one id: the first in scan order is the item, the others are invalid.
  const seen = new Map<string, FoundItem>()
  const kept: FoundItem[] = []
  for (const found of items) {
    const first = seen.get(found.item.id)
    if (first === undefined) {
      seen.set(found.item.id, found)
      kept.push(found)
    } else {
      invalid.push({ path: found.path, checkout, reason: `id "${found.item.id}" is already used by ${first.path}` })
    }
  }

  const unknownKind = kept.filter((found) => !(KNOWN_KINDS as readonly string[]).includes(found.item.kind)).length
  const brokenLink = kept.filter((found) => found.item.links.some((link) => !seen.has(link.to))).length
  return { items: kept, invalid, unknownKind, brokenLink }
}

/**
 * One item as the provider reports it. `body` is left out of `list`, which carries every item. A proposal also carries
 * its features and the problems of its Features part, which are read from the body whether or not it is sent.
 */
export function toTask(found: FoundItem, withBody: boolean): TaskInput {
  const { item } = found
  const { features, problems } = item.kind === PROPOSAL ? parseFeatures(found.body, found.bodyLine) : { features: [], problems: [] }
  const partOf = found.epic === null ? [] : [{ type: 'part-of', to: found.epic }]
  return {
    key: `plan:${item.id}`,
    id: item.id,
    ...(item.alias === null ? {} : { aliases: [item.alias] }),
    title: item.title,
    ...(withBody ? { body: found.body } : {}),
    state: stateOf(item.kind, item.status),
    version: found.version,
    path: found.path,
    kind: item.kind,
    links: [...partOf, ...item.links],
    owners: item.owners,
    ...(Object.keys(item.fields).length === 0 ? {} : { fields: item.fields }),
    ...(features.length === 0 ? {} : { features }),
    ...(problems.length === 0 ? {} : { featureProblems: problems }),
  }
}
