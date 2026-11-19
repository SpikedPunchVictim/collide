import { readFile, stat } from 'node:fs/promises'
import type { ApplyResultInput } from '@collide/pack-sdk'
import { failpoint, replaceFile } from './atomic.ts'
import { sectionProblem } from './feature-ops.ts'
import { hasSection, isUnsorted, renameHeadings } from './feature-text.ts'
import type { Where } from './feature-text.ts'
import { PROPOSAL, stateOf } from './item.ts'
import { DEFAULT_LAYOUT } from './layout.ts'
import type { Layout } from './layout.ts'
import { onlyKeys, refused } from './op-util.ts'
import { BODY_CAP, findFence, versionOf } from './read-file.ts'
import { readEpicFiles, scanEpics, toTask } from './scan.ts'
import type { FoundItem } from './scan.ts'

export const RENAME_SECTION = 'renameSection'

/** The text of a name argument, or null when it is not text or is blank. */
const nameOf = (value: unknown): string | null =>
  typeof value === 'string' && value.trim() !== '' ? value.trim() : null

/** `versions` is an optional map of an item id to the version the caller last read: the stale one is a conflict. */
const versionsOf = (args: Record<string, unknown>): Record<string, string> =>
  (args.versions ?? {}) as Record<string, string>

/** The problem with `renameSection`'s arguments, or null. Runs before the lock is taken. */
export function checkRenameSection(args: Record<string, unknown>): string | null {
  const from = nameOf(args.from)
  const to = nameOf(args.to)
  if (from === null) return 'from must be the name of a section, as text'
  if (to === null) return 'to must be the name of a section, as text'
  const problem = sectionProblem(from) ?? sectionProblem(to)
  if (problem !== null) return problem
  if (isUnsorted(from)) {
    return 'from cannot be Unsorted: Unsorted is the part for features with no section, which has no name to change'
  }
  if (from === to) return 'from and to are the same name'
  const { versions } = args
  if (versions !== undefined) {
    const map = typeof versions === 'object' && versions !== null && !Array.isArray(versions)
    if (!map || !Object.values(versions).every((version) => typeof version === 'string')) {
      return 'versions must be an object that maps an item id to the version you last read, as text'
    }
  }
  return onlyKeys(args, ['from', 'to', 'versions'])
}

/** One file with its new content, worked out and checked before anything is written. */
interface Planned {
  found: FoundItem
  content: Buffer
  mode: number
}

/**
 * Renames the section `from` to `to` in every proposal that has it, whatever its status, in the shared root and in the
 * private root when there is one (0.10, "A proposal's features"). The order: the proposals that have a heading for
 * `from` are found; each file is read again and its version checked (the one the scan saw, and the one in `versions`
 * when the caller sent it, which answers `conflict` with that file's current task); the new text of every file is made
 * and checked (a refusal writes nothing); then each file is replaced by temp and rename, in scan order. A kill between
 * two files leaves each whole and the rename part done, and running it again finishes it, since a file that has no
 * heading for `from` is left alone. The caller holds the lock of every root that has items. A file of the private root
 * is rewritten and is in `items` but not in `files`: nothing in it is committed.
 */
export async function renameSectionLocked(
  root: string,
  args: Record<string, unknown>,
  layout: Layout = DEFAULT_LAYOUT,
  privateDir: string | null = null,
): Promise<ApplyResultInput> {
  const from = args.from as string
  const to = (args.to as string).trim()
  const versions = versionsOf(args)
  const { items } = await scanEpics(root, null, layout, privateDir)
  const holders = items.filter((found) => found.item.kind === PROPOSAL && hasSection(found.body, found.bodyLine, from))
  if (holders.length === 0) {
    return refused(`${RENAME_SECTION}: no proposal has a section named ${JSON.stringify(from.trim())}`)
  }

  const planned: Planned[] = []
  const bytesOf = new Map<FoundItem, Buffer>()
  for (const found of holders) {
    const bytes = await readFile(found.absolute)
    if (versionOf(bytes) !== found.version) {
      return refused(`${found.path} changed while it was being read; send the op again`)
    }
    if (versions[found.item.id] !== undefined && versions[found.item.id] !== found.version) {
      return { outcome: 'conflict', current: toTask(found, true) }
    }
    bytesOf.set(found, bytes)
  }
  for (const found of holders) {
    const bytes = bytesOf.get(found) as Buffer
    const fence = findFence(bytes, bytes.length)
    if (!fence.ok) return refused(`${found.path}: ${fence.reason}`)
    // The file's own line ending is the one its opening line has.
    const eol = fence.start === 5 ? '\r\n' : '\n'
    const where: Where = { id: found.item.id, path: found.path, firstLine: found.bodyLine }
    const result = renameHeadings(where, bytes.subarray(fence.bodyStart).toString('utf8'), eol, from, to)
    if (!result.ok) return refused(`${RENAME_SECTION}: ${result.message}`)
    if (!result.changed) continue
    if (Buffer.byteLength(result.body) > BODY_CAP) {
      return refused(`${found.path}: the body would be over the ${BODY_CAP}-byte cap`)
    }
    const content = Buffer.concat([bytes.subarray(0, fence.bodyStart), Buffer.from(result.body, 'utf8')])
    planned.push({ found, content, mode: (await stat(found.absolute)).mode & 0o777 })
  }
  if (planned.length === 0) {
    const named = `${JSON.stringify(from.trim())} is already named ${JSON.stringify(to)}`
    return refused(`${RENAME_SECTION}: every section named ${named}`)
  }

  for (const [index, { found, content, mode }] of planned.entries()) {
    await replaceFile(found.absolute, content, mode)
    if (index < planned.length - 1) failpoint('between-files')
  }

  const after = (await readEpicFiles(root, null, layout, privateDir)).items
  const changed = planned.map(({ found }) => {
    const read = after.find((candidate) => candidate.absolute === found.absolute)
    if (read === undefined) throw new Error(`${found.path} does not read back after the write`)
    return read
  })
  const first = changed[0] as FoundItem
  return {
    outcome: 'ok',
    task: toTask(first, true),
    version: first.version,
    files: changed.filter((found) => found.root === 'shared').map((found) => found.path),
    items: changed.map((found) => ({
      key: `plan:${found.item.id}`,
      version: found.version,
      from: stateOf(found.item.kind, found.item.status),
      to: stateOf(found.item.kind, found.item.status),
    })),
  }
}
