import { randomInt } from 'node:crypto'
import { mkdir, readdir, stat } from 'node:fs/promises'
import { dirname, join, relative, sep } from 'node:path'
import type { ApplyResultInput } from '@collide/pack-sdk'
import { replaceFile, syncDirectory } from './atomic.ts'
import { drawFreeId, IdSourceError, KIND_IDS, nextNumber, seededGenerator, takenIds } from './ids.ts'
import type { CreatableKind, IdHooks } from './ids.ts'
import { DEFAULT_LAYOUT } from './layout.ts'
import type { Layout } from './layout.ts'
import { readItem, stateOf } from './item.ts'
import { CONTROL, onlyKeys, refused } from './op-util.ts'
import { readItemFile } from './read-file.ts'
import { readEpicFiles, toTask } from './scan.ts'

export const CREATE = 'create'

export const TITLE_MAX = 200

const SLUG_WORDS = 6
const SLUG_WORD_MAX = 40
const EPIC_ID = /^ep-[0-9]{3,}-[a-z]{2}$/
const EPIC_FOLDER = /^(ep-[0-9]{3,}-[a-z]{2})(-.*)?$/

/**
 * The slug of a title: its first six words, lower case, joined by hyphens, as the item files here are named. A word is
 * a run of letters and digits (a hyphen, a dot or an en dash ends one, an apostrophe does not), so "Collide's" is
 * `collides` and "S5–S8" is two words. A title with no such word has the slug `item`.
 */
export function slugOf(title: string): string {
  const words = title
    .toLowerCase()
    .replace(/['\u2019]/g, '')
    .split(/[^a-z0-9]+/)
    .filter((word) => word !== '')
    .slice(0, SLUG_WORDS)
    .map((word) => word.slice(0, SLUG_WORD_MAX))
  return words.length === 0 ? 'item' : words.join('-')
}

/** The problem with a `seed` argument, or null. The seed is any whole number. */
export function checkSeed(args: Record<string, unknown>): string | null {
  return args.seed === undefined || Number.isSafeInteger(args.seed) ? null : 'seed must be a whole number'
}

/** The generator for a `seed` argument: seeded when it is given, random (a fresh seed) when it is not. */
export function generatorFor(args: Record<string, unknown>, hooks: IdHooks): () => number {
  const seed = typeof args.seed === 'number' ? args.seed : randomInt(2 ** 32)
  return (hooks.generator ?? seededGenerator)(seed)
}

/** The problem with `create`'s arguments, or null. Runs before the lock is taken. */
export function checkCreate(args: Record<string, unknown>): string | null {
  const { kind, epic, title } = args
  if (kind !== undefined && kind !== 'task' && kind !== 'epic' && kind !== 'proposal') return 'kind must be "task", "epic" or "proposal"'
  if (typeof title !== 'string' || title.trim() !== title || title === '' || title.length > TITLE_MAX || CONTROL.test(title)) {
    return `title must be text of 1 to ${TITLE_MAX} characters, with no line break or other control character and no space at either end`
  }
  if (kind === 'epic') {
    if (epic !== undefined) return 'an epic is part of no epic: leave epic out'
  } else if (typeof epic !== 'string' || !EPIC_ID.test(epic)) {
    return `a ${kind === 'proposal' ? 'proposal' : 'task'} needs epic, the id of its epic, such as ep-002-va`
  }
  return checkSeed(args) ?? onlyKeys(args, ['kind', 'epic', 'title', 'seed'])
}

/** A new item: a task or an epic starts `ready`, with the parts a task has; a proposal starts as a `draft`, with the two parts 0.10 names. */
const fileOf = (id: string, kind: CreatableKind, title: string): string => {
  const head = `---\nid: ${id}\nkind: ${kind}\nstatus: ${kind === 'proposal' ? 'draft' : 'ready'}\ntitle: ${JSON.stringify(title)}\nowners: []\nlinks: []\n---\n\n`
  return head + (kind === 'proposal' ? '## Features\n\n## Decision\n' : '## Goal\n\n## Success criteria\n\n## Tests\n\n## Notes\n')
}

const exists = async (path: string): Promise<boolean> => {
  try {
    await stat(path)
    return true
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return false
    throw error
  }
}

/** The folder in the working tree that holds epic `epic`, or null. */
async function epicFolder(epicsDir: string, epic: string): Promise<string | null> {
  let names: string[]
  try {
    names = (await readdir(epicsDir, { withFileTypes: true })).filter((entry) => entry.isDirectory()).map((entry) => entry.name).sort()
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return null
    throw error
  }
  return names.find((name) => EPIC_FOLDER.exec(name)?.[1] === epic) ?? null
}

/** The refusal for an epic the root has no folder for. An epic that a previous root holds is named, with where. */
async function noEpic(root: string, layout: Layout, epic: string): Promise<string> {
  const refusal = `there is no epic ${epic} in the working tree (${layout.root}/epics/); create it first`
  if (layout.previous.length === 0) return refusal
  const { items } = await readEpicFiles(root, null, layout)
  const old = items.find((found) => found.item.kind === 'epic' && found.item.id === epic)
  if (old === undefined) return refusal
  return `${refusal}; ${epic} is at ${dirname(old.path)}, under a previous root, which a new item does not go to`
}

/** Makes `dir` and its missing parents, and syncs the directories that gained an entry, so the new file survives a crash. */
async function makeDirectory(dir: string): Promise<void> {
  const first = await mkdir(dir, { recursive: true })
  if (first === undefined) return
  for (let made = dir; ; made = dirname(made)) {
    await syncDirectory(dirname(made))
    if (made === first) return
  }
}

/**
 * Writes a new item: the number is the next free one for the kind across every ref and the working tree, the suffix is
 * drawn until the id is free, and the file goes in the epic's `tasks/` folder, or its `proposals/` folder for a proposal
 * (an epic gets its own folder with an `epic.md`). The item goes under the layout's root, never under a `previous`
 * one, and the number is free across the roots of all of them. The caller holds the lock. The reply carries the item as
 * read back from disk and the file it wrote.
 */
export async function createLocked(
  root: string,
  args: Record<string, unknown>,
  hooks: IdHooks,
  layout: Layout = DEFAULT_LAYOUT,
): Promise<ApplyResultInput> {
  const kind = (args.kind ?? 'task') as CreatableKind
  const title = args.title as string
  const { prefix, width } = KIND_IDS[kind]
  const epicsDir = join(root, layout.root, 'epics')

  let folder: string | null = null
  if (kind !== 'epic') {
    folder = await epicFolder(epicsDir, args.epic as string)
    if (folder === null) return refused(await noEpic(root, layout, args.epic as string))
  }

  const look = hooks.look ?? takenIds
  let id: string
  try {
    const number = nextNumber(await look(root, layout), prefix)
    id = await drawFreeId({ prefix, number, width, next: generatorFor(args, hooks), look: () => look(root, layout) })
  } catch (error) {
    if (error instanceof IdSourceError) return refused(`create: ${error.message}`)
    throw error
  }

  const name = `${id}-${slugOf(title)}`
  const subfolder = kind === 'proposal' ? 'proposals' : 'tasks'
  const absolute = kind === 'epic' ? join(epicsDir, name, 'epic.md') : join(epicsDir, folder as string, subfolder, `${name}.md`)
  if (await exists(absolute)) return refused(`${absolute} already exists; nothing was written`)
  await makeDirectory(dirname(absolute))
  await replaceFile(absolute, Buffer.from(fileOf(id, kind, title), 'utf8'), 0o644)

  const read = await readItemFile(absolute)
  if (!read.ok) throw new Error(`${absolute} does not read back after the write: ${read.reason}`)
  const checked = readItem(read.frontmatter)
  if (!checked.ok) throw new Error(`${absolute} does not read back after the write: ${checked.reason}`)
  if (checked.item.id !== id || checked.item.title !== title) throw new Error(`${absolute} does not read back as the item that was written`)
  const path = relative(root, absolute).split(sep).join('/')
  const found = { item: checked.item, epic: folder === null ? null : (EPIC_FOLDER.exec(folder)?.[1] as string), path, body: read.body, bodyLine: read.bodyLine, version: read.version }
  return {
    outcome: 'ok',
    task: toTask(found, true),
    version: found.version,
    files: [path],
    items: [{ key: `plan:${id}`, version: found.version, from: null, to: stateOf(kind, found.item.status) }],
  }
}
