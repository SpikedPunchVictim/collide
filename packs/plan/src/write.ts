import { randomBytes } from 'node:crypto'
import { open, readdir, readFile, rename, stat, unlink } from 'node:fs/promises'
import { basename, dirname, join } from 'node:path'
import { isDeepStrictEqual } from 'node:util'
import type { ApplyParamsOutput, ApplyResultInput } from '@collide/pack-sdk'
import { addBlockedByLine, clearOwnersLine, setOwnerLine, setStatusLine } from './frontmatter-edit.ts'
import type { Edit } from './frontmatter-edit.ts'
import { CATEGORIES, ID_PATTERN, readItem } from './item.ts'
import { LOCK_FILE, withLock } from './lock.ts'
import type { LockOptions } from './lock.ts'
import { findFence, parseFrontmatter, readItemFile, versionOf } from './read-file.ts'
import { scanEpics, SHARED_ROOT, toTask } from './scan.ts'
import type { FoundItem, Scan } from './scan.ts'

/** The ending of a temp file's name. The scan reads only `.md` files, so it never opens one, and `.gitignore` lists it. */
export const TEMP_SUFFIX = '.collide-tmp'

/**
 * Test hook: when the environment variable `COLLIDE_FAILPOINT` equals the name of a failpoint, the process kills
 * itself with SIGKILL there. `before-rename` is the one failpoint: it fires after the temp file is written and
 * synced and before it is renamed over the item, so a test can check that a killed writer leaves the old file whole.
 * With the variable unset (every real run) nothing happens.
 */
function failpoint(name: 'before-rename'): void {
  if (process.env.COLLIDE_FAILPOINT !== name) return
  process.kill(process.pid, 'SIGKILL')
  // The signal can arrive a moment after `kill` returns; do not go on in the meantime.
  Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 10_000)
}

const SET_STATUS = 'setStatus'
const LINK = 'link'
const CLAIM = 'claim'
const UNCLAIM = 'unclaim'

const refused = (message: string): ApplyResultInput => ({ outcome: 'refused', message })

type Frontmatter = Record<string, unknown>

/** What an op does to one item: nothing, a refusal, or a line edit plus the frontmatter it must produce. */
type Prepared =
  | { kind: 'refuse'; message: string }
  | { kind: 'nothing' }
  | { kind: 'edit'; edit: (text: string) => Edit; expected: Frontmatter }

interface OpSpec {
  /** The problem with `args`, or null. Runs before the lock is taken. */
  check: (args: Record<string, unknown>) => string | null
  prepare: (found: FoundItem, scan: Scan, args: Record<string, unknown>, frontmatter: Frontmatter) => Prepared
}

const stringArg = (args: Record<string, unknown>, name: string): string | null => (typeof args[name] === 'string' ? args[name] : null)

/** Refuses an argument that the op does not declare, so a misspelled one is not silently dropped. */
const onlyKeys = (args: Record<string, unknown>, allowed: string[]): string | null => {
  const extra = Object.keys(args).find((key) => !allowed.includes(key))
  return extra === undefined ? null : `unknown argument "${extra}"; this op takes ${allowed.length === 0 ? 'no arguments' : allowed.join(', ')}`
}

const idRegex = new RegExp(ID_PATTERN)
const CONTROL = /[\u0000-\u001f\u007f]/

const OPS: Record<string, OpSpec> = {
  [SET_STATUS]: {
    check: (args) => {
      const { status, category } = args
      if (status !== undefined && category !== undefined) return 'give either status or category, not both'
      const given = status ?? category
      if (given === undefined) return `give status or category, one of ${CATEGORIES.join(', ')}`
      // Every status is named after its category (0.10), so both take the same five values.
      if (typeof given !== 'string' || !(CATEGORIES as readonly string[]).includes(given)) return `${status === undefined ? 'category' : 'status'} must be one of ${CATEGORIES.join(', ')}`
      return onlyKeys(args, ['status', 'category'])
    },
    prepare: (found, _scan, args, frontmatter) => {
      const category = (args.status ?? args.category) as string
      if (found.item.status === category) return { kind: 'nothing' }
      return { kind: 'edit', edit: (text) => setStatusLine(text, category), expected: { ...frontmatter, status: category } }
    },
  },
  [CLAIM]: {
    check: (args) => {
      const claimant = stringArg(args, 'claimant')
      if (claimant === null || claimant.trim() !== claimant || claimant === '' || claimant.length > 200 || CONTROL.test(claimant)) {
        return 'claimant must be text of 1 to 200 characters, with no line break or other control character and no space at either end'
      }
      return onlyKeys(args, ['claimant'])
    },
    prepare: (found, _scan, args, frontmatter) => {
      const claimant = args.claimant as string
      const { owners } = found.item
      if (owners.includes(claimant)) return { kind: 'nothing' }
      if (owners.length > 0) return { kind: 'refuse', message: `${found.item.id} already shows an owner (${owners.join(', ')}); unclaim it first` }
      return { kind: 'edit', edit: (text) => setOwnerLine(text, claimant), expected: { ...frontmatter, owners: [claimant] } }
    },
  },
  [UNCLAIM]: {
    check: (args) => onlyKeys(args, []),
    prepare: (found, _scan, _args, frontmatter) =>
      found.item.owners.length === 0 ? { kind: 'nothing' } : { kind: 'edit', edit: clearOwnersLine, expected: { ...frontmatter, owners: [] } },
  },
  [LINK]: {
    check: (args) => {
      const to = stringArg(args, 'to')
      if (to === null || !idRegex.test(to)) return `to must be the id of an item, matching ${ID_PATTERN}`
      if (args.type !== undefined && args.type !== 'blocked-by') return 'type must be "blocked-by", the one link type that is stored'
      return onlyKeys(args, ['to', 'type'])
    },
    prepare: (found, scan, args, frontmatter) => {
      const to = args.to as string
      const { id, links } = found.item
      if (to === id) return { kind: 'refuse', message: `${id} cannot be blocked by itself` }
      if (!scan.items.some((other) => other.item.id === to)) return { kind: 'refuse', message: `there is no item ${to} to be blocked by` }
      if (links.some((link) => link.type === 'blocked-by' && link.to === to)) return { kind: 'nothing' }
      const stored = Array.isArray(frontmatter.links) ? frontmatter.links : []
      return {
        kind: 'edit',
        edit: (text) => addBlockedByLine(text, to),
        expected: { ...frontmatter, links: [...stored, { type: 'blocked-by', to }] },
      }
    },
  },
}

export const OP_NAMES = Object.keys(OPS)

/** Syncs a directory so that a rename in it survives a crash. Windows cannot open a directory, so it is skipped there. */
async function syncDirectory(path: string): Promise<void> {
  if (process.platform === 'win32') return
  const handle = await open(path, 'r')
  try {
    await handle.sync()
  } finally {
    await handle.close()
  }
}

/**
 * Replaces `path` with `content` so that a reader sees the old file or the new one and never half of either: the
 * content goes to a temp file beside it (same directory, so the rename stays on one file system), is synced, renamed
 * over the item, and the directory is synced. The file keeps its permission bits.
 */
async function replaceFile(path: string, content: Buffer, mode: number): Promise<void> {
  // Every temp file is made under the lock, which this writer holds, so one still here is what a killed writer left.
  const mine = `.${basename(path)}.`
  for (const name of await readdir(dirname(path))) {
    if (name.startsWith(mine) && name.endsWith(TEMP_SUFFIX)) await unlink(join(dirname(path), name))
  }
  const temp = join(dirname(path), `.${basename(path)}.${process.pid}-${randomBytes(4).toString('hex')}${TEMP_SUFFIX}`)
  const handle = await open(temp, 'wx', mode)
  try {
    try {
      await handle.chmod(mode)
      await handle.writeFile(content)
      await handle.sync()
    } finally {
      await handle.close()
    }
    failpoint('before-rename')
    await rename(temp, path)
  } catch (error) {
    // Best effort: the write failed and that error is the one to report, so a failed cleanup must not replace it.
    await unlink(temp).catch(() => {})
    throw error
  }
  await syncDirectory(dirname(path))
}

/** Reads the item back from disk, so the reply is what is on disk. */
async function readBack(found: FoundItem, absolute: string): Promise<FoundItem> {
  const read = await readItemFile(absolute)
  if (!read.ok) throw new Error(`${found.path} does not read back after the write: ${read.reason}`)
  const checked = readItem(read.frontmatter)
  if (!checked.ok) throw new Error(`${found.path} does not read back after the write: ${checked.reason}`)
  return { ...found, item: checked.item, body: read.body, version: read.version }
}

async function writeLocked(root: string, id: string, version: string, spec: OpSpec, args: Record<string, unknown>): Promise<ApplyResultInput> {
  const scan = await scanEpics(root)
  const found = scan.items.find((candidate) => candidate.item.id === id)
  if (found === undefined) return refused(`there is no item ${id}`)
  const absolute = join(root, ...found.path.split('/'))

  const bytes = await readFile(absolute)
  if (versionOf(bytes) !== found.version) return refused(`${found.path} changed while it was being read; send the op again`)
  if (version !== found.version) return { outcome: 'conflict', current: toTask(found, true) }

  const fence = findFence(bytes, bytes.length)
  if (!fence.ok) return refused(`${found.path}: ${fence.reason}`)
  const text = bytes.subarray(fence.start, fence.end).toString('utf8')
  const parsed = parseFrontmatter(text)
  if (!parsed.ok) return refused(`${found.path}: ${parsed.reason}`)

  const prepared = spec.prepare(found, scan, args, parsed.value)
  if (prepared.kind === 'refuse') return refused(prepared.message)
  if (prepared.kind === 'nothing') return { outcome: 'ok', task: toTask(found, true), version: found.version, files: [], items: [] }

  const edited = prepared.edit(text)
  if (!edited.ok) return refused(`${found.path}: ${edited.reason}`)
  const reparsed = parseFrontmatter(edited.text)
  if (!reparsed.ok || !isDeepStrictEqual(reparsed.value, prepared.expected)) {
    return refused(`${found.path}: the line edit would change more than the one field, so nothing was written; edit the file by hand`)
  }

  const content = Buffer.concat([bytes.subarray(0, fence.start), Buffer.from(edited.text, 'utf8'), bytes.subarray(fence.end)])
  await replaceFile(absolute, content, (await stat(absolute)).mode & 0o777)

  const after = await readBack(found, absolute)
  const key = `plan:${id}`
  return {
    outcome: 'ok',
    task: toTask(after, true),
    version: after.version,
    files: [after.path],
    items: [{ key, version: after.version, from: { category: found.item.status, status: found.item.status }, to: { category: after.item.status, status: after.item.status } }],
  }
}

/**
 * Protocol v0's `apply` for the markdown provider. The root is the main checkout; the shared root is
 * `<root>/.collide/planning`, and every write there holds that root's lock file. `version` is required: a write
 * must say which content it was made against. An op that would change nothing writes nothing and answers `ok` with
 * no `files` and no `items`.
 */
export async function applyOp(root: string, params: ApplyParamsOutput, lock: LockOptions = {}): Promise<ApplyResultInput> {
  const { op, id, version, checkout } = params
  const spec = Object.hasOwn(OPS, op) ? OPS[op] : undefined
  if (spec === undefined) return refused(`unknown op "${op}"; this provider has ${OP_NAMES.join(', ')}`)
  if (checkout !== undefined && checkout !== null) return refused(`writes into another checkout are not supported yet (checkout "${checkout}"); leave checkout out to write to the main checkout`)
  if (typeof id !== 'string' || id === '') return refused(`${op} needs the id of the item to change`)
  if (typeof version !== 'string' || version === '') return refused(`${op} needs the version you last read: send it, so that a write over a newer file can be told from one over the file you saw`)
  const args = params.args ?? {}
  const problem = spec.check(args)
  if (problem !== null) return refused(`${op}: ${problem}`)

  const planning = join(root, SHARED_ROOT)
  try {
    await stat(planning)
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === 'ENOENT') return refused(`there is no item ${id}`)
    throw error
  }
  return withLock(join(planning, LOCK_FILE), () => writeLocked(root, id, version, spec, args), lock)
}
