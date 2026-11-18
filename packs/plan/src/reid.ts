import { readdir, readFile, rename, stat } from 'node:fs/promises'
import { basename, dirname, join, relative, sep } from 'node:path'
import { isDeepStrictEqual } from 'node:util'
import type { ApplyResultInput } from '@collide/pack-sdk'
import { replaceFile, syncDirectory, TEMP_SUFFIX } from './atomic.ts'
import { replaceLinkTarget, setIdLine } from './frontmatter-edit.ts'
import type { Edit } from './frontmatter-edit.ts'
import { checkSeed, generatorFor } from './create.ts'
import { namesItem } from './feature-target.ts'
import { drawFreeId, IdSourceError, takenIds } from './ids.ts'
import type { IdHooks } from './ids.ts'
import { DEFAULT_LAYOUT } from './layout.ts'
import type { Layout } from './layout.ts'
import { SPIKE, stateOf } from './item.ts'
import { onlyKeys, refused } from './op-util.ts'
import { findFence, parseFrontmatter, versionOf } from './read-file.ts'
import { pathOf, PRIVATE_PREFIX, readEpicFiles, toTask } from './scan.ts'
import type { FoundItem } from './scan.ts'

export const REID = 'reid'

/** The problem with `reid`'s arguments, or null. Runs before the lock is taken. */
export function checkReid(args: Record<string, unknown>): string | null {
  return checkSeed(args) ?? onlyKeys(args, ['seed'])
}

/** The files under `dir`, at any depth, as absolute paths. A temp file of a killed writer is not one. */
async function filesUnder(dir: string): Promise<string[]> {
  const found: string[] = []
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) found.push(...(await filesUnder(path)))
    else if (entry.isFile() && !entry.name.endsWith(TEMP_SUFFIX)) found.push(path)
  }
  return found
}

/** One file with its new content, worked out and checked before anything is written. */
interface Planned {
  found: FoundItem
  absolute: string
  content: Buffer
  mode: number
}

/**
 * Works out the new content of `found`: its own `id:` line when it is the item being renamed, and the target of every
 * link that names the old id. Returns the reason when a line is in a form the provider does not edit, or when the
 * edit changes more than those fields.
 */
async function plan(found: FoundItem, oldId: string, newId: string, renamed: boolean): Promise<Planned | string> {
  const { absolute } = found
  const bytes = await readFile(absolute)
  if (versionOf(bytes) !== found.version) return `${found.path} changed while it was being read; send the op again`
  const fence = findFence(bytes, bytes.length)
  if (!fence.ok) return `${found.path}: ${fence.reason}`
  const text = bytes.subarray(fence.start, fence.end).toString('utf8')
  const parsed = parseFrontmatter(text)
  if (!parsed.ok) return `${found.path}: ${parsed.reason}`

  const expected: Record<string, unknown> = { ...parsed.value }
  let edited: Edit = { ok: true, text }
  if (renamed) {
    edited = setIdLine(text, newId)
    expected.id = newId
  }
  if (edited.ok && found.item.links.some((link) => namesItem(link.to, oldId))) {
    edited = replaceLinkTarget(edited.text, oldId, newId)
    // A link to a feature, `pr-0003-km#F2`, keeps its feature: only the id before the `#` is the old one.
    expected.links = (parsed.value.links as Array<Record<string, unknown>>).map((link) => (typeof link.to === 'string' && namesItem(link.to, oldId) ? { ...link, to: `${newId}${link.to.slice(oldId.length)}` } : link))
  }
  if (!edited.ok) return `${found.path}: ${edited.reason}`
  const reparsed = parseFrontmatter(edited.text)
  if (!reparsed.ok || !isDeepStrictEqual(reparsed.value, expected)) {
    return `${found.path}: the line edit would change more than the id and the link targets, so nothing was written; edit the file by hand`
  }
  const content = Buffer.concat([bytes.subarray(0, fence.start), Buffer.from(edited.text, 'utf8'), bytes.subarray(fence.end)])
  return { found, absolute, content, mode: (await stat(absolute)).mode & 0o777 }
}

/**
 * Gives an item a new suffix and rewrites every link that names the old id, in every item file that reads. The item is
 * the file with that id; when two files share it, the second one in scan order is the one the scan sets aside as
 * invalid, and that is the one renamed (a third is reached by running `reid` again). Nothing is committed, and a
 * link meant for the other file with the old id is rewritten too: the id cannot tell them apart.
 *
 * The order: every file's new content is made and checked first (a refusal writes nothing); then each file is replaced
 * in place, the renamed item's own file last; then the file (or an epic's folder) takes the new name. A kill between
 * two steps leaves every file whole, and at worst some links ahead of the item or a file name behind its id.
 * Every root of the layout is read, and the private root when there is one, and a file is renamed where it is. A file
 * of the private root is rewritten like the others but is not in `files`: nothing in it is committed. The caller holds
 * the lock of every root that has items.
 */
export async function reidLocked(
  root: string,
  id: string,
  args: Record<string, unknown>,
  hooks: IdHooks,
  layout: Layout = DEFAULT_LAYOUT,
  privateDir: string | null = null,
): Promise<ApplyResultInput> {
  const { items: all } = await readEpicFiles(root, null, layout, privateDir)
  const sharing = all.filter((found) => found.item.id === id)
  const target = sharing[1] ?? sharing[0]
  if (target === undefined) return refused(`there is no item ${id}`)

  const [, numberText] = id.split('-') as [string, string]
  const prefix = id.slice(0, 2)
  const look = hooks.look ?? takenIds
  let newId: string
  try {
    const number = Number(numberText)
    const next = generatorFor(args, hooks)
    const width = numberText.length
    newId = await drawFreeId({ prefix, number, width, next, look: () => look(root, layout, privateDir) })
  } catch (error) {
    if (error instanceof IdSourceError) return refused(`reid: ${error.message}`)
    throw error
  }

  const planned: Planned[] = []
  for (const found of all.filter((candidate) => candidate === target || candidate.item.links.some((link) => namesItem(link.to, id)))) {
    const result = await plan(found, id, newId, found === target)
    if (typeof result === 'string') return refused(result)
    planned.push(result)
  }
  planned.sort((a, b) => Number(a.found === target) - Number(b.found === target))
  for (const { absolute, content, mode } of planned) await replaceFile(absolute, content, mode)

  // The new name: an item file keeps what follows the id in its name; an epic's folder is renamed, and its files go with it.
  const targetAbsolute = target.absolute
  const isEpicFile = target.epic === null
  const renamedFrom = isEpicFile ? dirname(targetAbsolute) : targetAbsolute
  const oldName = basename(renamedFrom)
  let renamedTo: string | null = null
  let moved: string[] = []
  if (oldName.startsWith(id)) {
    renamedTo = join(dirname(renamedFrom), `${newId}${oldName.slice(id.length)}`)
    // A spike folder is renamed whole, but only its record is a file of the item: nothing inside it is listed.
    moved = target.item.kind === SPIKE ? [targetAbsolute] : isEpicFile ? await filesUnder(renamedFrom) : [renamedFrom]
    await rename(renamedFrom, renamedTo)
    await syncDirectory(dirname(renamedTo))
  }
  const afterRename = (absolute: string): string => (renamedTo === null || (absolute !== renamedFrom && !absolute.startsWith(`${renamedFrom}${sep}`)) ? absolute : join(renamedTo, relative(renamedFrom, absolute)))

  const shown = (absolute: string): string => pathOf(root, privateDir, absolute)
  const written = planned.map(({ absolute }) => shown(afterRename(absolute)))
  const files = new Set([...written, ...moved.flatMap((absolute) => [shown(absolute), shown(afterRename(absolute))])])

  const after = (await readEpicFiles(root, null, layout, privateDir)).items
  const renamedItem = after.find((found) => found.item.id === newId)
  if (renamedItem === undefined) throw new Error(`${newId} does not read back after the write`)
  const changed = after.filter((found) => written.includes(found.path))
  return {
    outcome: 'ok',
    task: toTask(renamedItem, true),
    version: renamedItem.version,
    files: [...files].filter((path) => !path.startsWith(PRIVATE_PREFIX)),
    items: changed.map((found) => ({
      key: `plan:${found.item.id}`,
      version: found.version,
      from: stateOf(found.item.kind, found.item.status),
      to: stateOf(found.item.kind, found.item.status),
    })),
  }
}
