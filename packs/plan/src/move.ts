import { copyFile, lstat, readdir, readFile, rm } from 'node:fs/promises'
import { basename, dirname, join } from 'node:path'
import type { ApplyResultInput } from '@collide/pack-sdk'
import { epicFolder, makeDirectory } from './create.ts'
import { IdSourceError, takenIds } from './ids.ts'
import type { IdHooks } from './ids.ts'
import { PROPOSAL, SPIKE, stateOf } from './item.ts'
import { spikeDirOf } from './layout.ts'
import type { Layout } from './layout.ts'
import { onlyKeys, refused } from './op-util.ts'
import { readItemFile, versionOf } from './read-file.ts'
import { pathOf, readEpicFiles, toTask } from './scan.ts'
import type { FoundItem } from './scan.ts'
import type { Asker } from './transitions.ts'

export const MOVE = 'move'

/** `move` takes no argument: what it moves, and where to, is the item's own place (0.10, Follow-ups). */
export const checkMove = (args: Record<string, unknown>): string | null => onlyKeys(args, [])

/**
 * The refusal for a request that did not come from a person at the app or the CLI, in the provider's own words, or
 * null. This is the second wall: the daemon refuses a run's agent first.
 */
export function moveWall(id: string, asker: Asker | null | undefined): string | null {
  if (asker?.kind !== 'agent') return null
  const who = typeof asker.run === 'string' && asker.run !== '' ? `the agent of run ${asker.run}` : 'an agent'
  return `only a person may keep ${id}; this request came from ${who}, and a person does it in the app or the CLI`
}

/** The regular files under `dir`, as paths from `dir`, or the first thing that is neither a file nor a folder. */
async function filesUnder(dir: string, from = ''): Promise<{ files: string[] } | { odd: string }> {
  const files: string[] = []
  const entries = await readdir(join(dir, from), { withFileTypes: true })
  for (const entry of entries.sort((a, b) => (a.name < b.name ? -1 : 1))) {
    const relative = from === '' ? entry.name : `${from}/${entry.name}`
    if (entry.isDirectory()) {
      const inside = await filesUnder(dir, relative)
      if ('odd' in inside) return inside
      files.push(...inside.files)
    } else if (entry.isFile()) {
      files.push(relative)
    } else {
      return { odd: relative }
    }
  }
  return { files }
}

/** Where `found`, a private item, goes in the shared root: the file or folder, or the refusal. */
async function destinationOf(
  root: string,
  layout: Layout,
  found: FoundItem,
): Promise<{ target: string } | { refusal: string }> {
  const { id, kind } = found.item
  if (kind === SPIKE) return { target: join(root, spikeDirOf(layout), basename(dirname(found.absolute))) }
  const folder = found.epic === null ? null : await epicFolder(join(root, layout.root, 'epics'), found.epic)
  if (folder === null) {
    const why = `which is private: keep the epic first, or make the item in a shared epic`
    return { refusal: `${id} is in ${found.epic}, ${why}` }
  }
  const subfolder = basename(dirname(found.absolute))
  return { target: join(root, layout.root, 'epics', folder, subfolder, basename(found.absolute)) }
}

/**
 * Keeps one private item (0.10, Follow-ups; Share of one item): the file moves to the shared root, in the folder of the
 * same epic, under the same id, and a spike's folder moves whole, to the shared spikes folder. The caller holds the
 * lock of both roots, the shared one first. The reply names the files now in the repository, which is what `record`
 * commits, and carries the item as read back from its new place. The content is copied and read back before the
 * private copy is removed, so a failure leaves the item where it was. A refusal moves nothing.
 *
 * What it refuses: an item that is not private; a proposal, which is shared whole; an epic, which has a folder of
 * items; an item whose links name a private item (the first, by id); an id that a shared ref or the working tree
 * already has; an epic that has no shared folder; a destination that is there; a spike folder with a link or another
 * thing that is not a file or a folder.
 */
export async function moveLocked(
  root: string,
  id: string,
  version: string,
  hooks: IdHooks,
  layout: Layout,
  privateDir: string,
): Promise<ApplyResultInput> {
  // Every file that read, so that an id in both roots is seen (a scan would set the private one aside).
  const { items } = await readEpicFiles(root, null, layout, privateDir)
  const same = items.filter((candidate) => candidate.item.id === id)
  const found = same.find((candidate) => candidate.root === 'private')
  if (found === undefined) {
    if (same.length === 0) return refused(`there is no item ${id}`)
    return refused(`${id} is already in the shared root: there is nothing to keep`)
  }
  const shared = same.find((candidate) => candidate.root === 'shared')
  if (shared !== undefined) {
    return refused(`${id} is in both roots, at ${shared.path} and ${found.path}; give the private one a new id first`)
  }
  const { kind } = found.item
  if (kind === PROPOSAL) return refused(`${id} is a proposal; a proposal is shared whole, not kept one item at a time`)
  if (kind === 'epic') return refused(`${id} is an epic, which has a folder of items; keeping an epic is not built`)

  const bytes = await readFile(found.absolute)
  if (versionOf(bytes) !== found.version) {
    return refused(`${found.path} changed while it was being read; send the op again`)
  }
  if (version !== found.version) return { outcome: 'conflict', current: toTask(found, true, items) }

  for (const link of found.item.links) {
    const target = link.to.split('#')[0]
    if (items.some((other) => other.item.id === target && other.root === 'private')) {
      return refused(`${id} links to ${target}, which is private; keep ${target} first, or take the link off ${id}`)
    }
  }
  try {
    if ((await (hooks.look ?? takenIds)(root, layout, null)).has(id)) {
      const where = 'on a branch or in the working tree'
      return refused(`${id} is already used in the shared plan, ${where}; give it a new id first`)
    }
  } catch (error) {
    if (error instanceof IdSourceError) return refused(`move: ${error.message}`)
    throw error
  }
  const where = await destinationOf(root, layout, found)
  if ('refusal' in where) return refused(where.refusal)
  const { target } = where
  try {
    await lstat(target)
    return refused(`${pathOf(root, null, target)} already exists; nothing was moved`)
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
  }

  // What is copied: the item's file, or every file of a spike's folder.
  const sourceDir = dirname(found.absolute)
  let names = [basename(found.absolute)]
  if (kind === SPIKE) {
    const inside = await filesUnder(sourceDir)
    if ('odd' in inside) {
      return refused(`${id}: ${inside.odd} is a link or another thing that is not a file; remove it first`)
    }
    names = inside.files
  }
  const copied: string[] = []
  const place = (name: string): string => (kind === SPIKE ? join(target, name) : target)
  try {
    for (const name of names) {
      const to = place(name)
      await makeDirectory(dirname(to))
      await copyFile(join(kind === SPIKE ? sourceDir : dirname(found.absolute), name), to)
      copied.push(to)
    }
    const itemFile = place(basename(found.absolute))
    const read = await readItemFile(itemFile)
    if (!read.ok || read.version !== found.version) {
      throw new Error(`${pathOf(root, null, itemFile)} does not read back as the item that was moved`)
    }
  } catch (error) {
    for (const path of copied) await rm(path, { force: true })
    if (kind === SPIKE) await rm(target, { recursive: true, force: true })
    throw error
  }
  if (kind === SPIKE) await rm(sourceDir, { recursive: true })
  else await rm(found.absolute)

  const absolute = place(basename(found.absolute))
  const after: FoundItem = { ...found, root: 'shared', path: pathOf(root, privateDir, absolute), absolute }
  const state = stateOf(kind, found.item.status)
  return {
    outcome: 'ok',
    task: toTask(
      after,
      true,
      items.map((item) => (item === found ? after : item)),
    ),
    version: found.version,
    files: copied.map((path) => pathOf(root, privateDir, path)),
    items: [{ key: `plan:${id}`, version: found.version, from: state, to: state }],
  }
}
