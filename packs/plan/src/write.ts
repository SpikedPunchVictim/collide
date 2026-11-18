import { mkdir, readFile, stat } from 'node:fs/promises'
import { join } from 'node:path'
import { isDeepStrictEqual } from 'node:util'
import type { ApplyParamsOutput, ApplyResultInput } from '@collide/pack-sdk'
import { replaceFile } from './atomic.ts'
import { CheckoutError, isMainCheckout, resolveCheckout } from './checkout.ts'
import { addLinkLine, clearOwnersLine, setOwnerLine, setStatusLine } from './frontmatter-edit.ts'
import type { Edit } from './frontmatter-edit.ts'
import { checkCreate, CREATE, createLocked, rootOfCreate } from './create.ts'
import type { IdHooks } from './ids.ts'
import { FEATURE_TARGET_EXAMPLE, parseFeatureTarget } from './feature-target.ts'
import { parseFeatures } from './features.ts'
import {
  BLOCKED_BY,
  CATEGORIES,
  hasOwnStatuses,
  ID_PATTERN,
  IMPLEMENTS,
  INFORMS,
  PROPOSAL,
  PROPOSAL_STATUSES,
  readItem,
  SPIKE_STATUSES,
  stateOf,
  statusesOf,
} from './item.ts'
import { describeFeatures, featuresOf } from './links.ts'
import { LOCK_FILE, withLock } from './lock.ts'
import type { LockOptions } from './lock.ts'
import { CONTROL, onlyKeys, refused, stringArg } from './op-util.ts'
import { checkReid, REID, reidLocked } from './reid.ts'
import { findFence, parseFrontmatter, readItemFile, versionOf } from './read-file.ts'
import { DEFAULT_LAYOUT, rootsOf, spikesDirsOf } from './layout.ts'
import type { Layout } from './layout.ts'
import { scanEpics, toTask } from './scan.ts'
import type { FoundItem, RootName, Scan } from './scan.ts'

export { TEMP_SUFFIX } from './atomic.ts'

const SET_STATUS = 'setStatus'
const LINK = 'link'
const CLAIM = 'claim'
const UNCLAIM = 'unclaim'

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

const idRegex = new RegExp(ID_PATTERN)

/** The link types `link` writes. */
const STORED_LINKS: readonly string[] = [BLOCKED_BY, IMPLEMENTS, INFORMS]

/** The status values `setStatus` takes for any kind: the five categories, a proposal's own six words and a spike's. */
const SET_STATUS_VALUES: readonly string[] = [
  ...new Set([
    ...CATEGORIES,
    ...PROPOSAL_STATUSES.map((status) => status.name),
    ...SPIKE_STATUSES.map((status) => status.name),
  ]),
]

/** What a `status` may be, kind by kind: the refusal for a word that no kind has lists them all. */
const STATUS_WORDS = [
  `${CATEGORIES.join(', ')} for a task or an epic`,
  `${PROPOSAL_STATUSES.map((status) => status.name).join(', ')} for a proposal`,
  `${SPIKE_STATUSES.map((status) => status.name).join(', ')} for a spike`,
].join('; ')

/** The refusal for a move to `proposed` while the Features part has problems: every problem, with its line. */
function proposedRefusal(found: FoundItem): string | null {
  const { problems } = parseFeatures(found.body, found.bodyLine)
  if (problems.length === 0) return null
  const lines = problems.map((problem) => `  line ${problem.line}: ${problem.reason}`)
  return `${found.item.id} cannot move to proposed: its ## Features part in ${found.path} has ${problems.length} problem${problems.length === 1 ? '' : 's'}; fix ${problems.length === 1 ? 'it' : 'them'} first:\n${lines.join('\n')}`
}

const NO_PACK_DATA =
  "create: root private needs the pack's data folder, and there is none: COLLIDE_PACK_DATA is not set to an absolute " +
  'folder (a command that runs without the daemon has none)'
const NOT_MAIN = 'create: root private is for the main checkout, and checkout names another: leave checkout out'

const OPS: Record<string, OpSpec> = {
  [SET_STATUS]: {
    check: (args) => {
      const { status, category } = args
      if (status !== undefined && category !== undefined) return 'give either status or category, not both'
      const given = status ?? category
      if (given === undefined) return `give status or category, one of ${CATEGORIES.join(', ')}`
      // A task's and an epic's status is named after its category (0.10), so `category` takes those five. A proposal's
      // or a spike's status is one of its own words, and `prepare` checks the value against the kind of the item.
      if (typeof given !== 'string' || !(status === undefined ? CATEGORIES : SET_STATUS_VALUES).includes(given)) {
        if (status === undefined) return `category must be one of ${CATEGORIES.join(', ')}`
        return `status must be one of ${STATUS_WORDS}`
      }
      return onlyKeys(args, ['status', 'category'])
    },
    prepare: (found, _scan, args, frontmatter) => {
      const { kind, id } = found.item
      const status = (args.status ?? args.category) as string
      const allowed = statusesOf(kind).map((candidate) => candidate.name)
      if (hasOwnStatuses(kind) && args.category !== undefined) {
        const message = `${id} is a ${kind}, whose statuses are its own: give status, one of ${allowed.join(', ')}`
        return { kind: 'refuse', message }
      }
      if (!allowed.includes(status)) {
        const what = hasOwnStatuses(kind) ? `a ${kind}` : `an item of kind ${kind}`
        const message = `${id} is ${what}; its status is one of ${allowed.join(', ')}, not ${status}`
        return { kind: 'refuse', message }
      }
      if (found.item.status === status) return { kind: 'nothing' }
      if (kind === PROPOSAL && status === 'proposed') {
        const message = proposedRefusal(found)
        if (message !== null) return { kind: 'refuse', message }
      }
      return { kind: 'edit', edit: (text) => setStatusLine(text, status), expected: { ...frontmatter, status } }
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
      const form = `to must be the id of an item, matching ${ID_PATTERN}, or of a feature of a proposal, such as ${FEATURE_TARGET_EXAMPLE}`
      if (to === null || !idRegex.test(to.split('#')[0] as string)) return form
      if (to.includes('#')) {
        const target = parseFeatureTarget(to)
        if (!target.ok) return `to ${to}: ${target.reason}`
      }
      if (args.type !== undefined && !STORED_LINKS.includes(args.type as string)) {
        return `type must be "${BLOCKED_BY}", "${IMPLEMENTS}" or "${INFORMS}", the link types that are stored`
      }
      return onlyKeys(args, ['to', 'type'])
    },
    prepare: (found, scan, args, frontmatter) => {
      const to = args.to as string
      const type = (args.type ?? BLOCKED_BY) as string
      const { id, links } = found.item
      const stored = Array.isArray(frontmatter.links) ? frontmatter.links : []
      const add = (): Prepared => ({ kind: 'edit', edit: (text) => addLinkLine(text, type, to), expected: { ...frontmatter, links: [...stored, { type, to }] } })

      if (type === BLOCKED_BY) {
        if (to.includes('#')) return { kind: 'refuse', message: `${BLOCKED_BY} names an item, not a feature, so ${to} cannot be its target; to link a task to the feature it builds, use type=${IMPLEMENTS}` }
        if (to === id) return { kind: 'refuse', message: `${id} cannot be blocked by itself` }
        if (!scan.items.some((other) => other.item.id === to)) return { kind: 'refuse', message: `there is no item ${to} to be blocked by` }
        return links.some((link) => link.type === BLOCKED_BY && link.to === to) ? { kind: 'nothing' } : add()
      }

      // informs: the target is an item, and the link blocks nothing.
      if (type === INFORMS) {
        if (to.includes('#')) {
          return { kind: 'refuse', message: `${INFORMS} names an item, not a feature, so ${to} cannot be its target` }
        }
        if (to === id) return { kind: 'refuse', message: `${id} cannot inform itself` }
        if (!scan.items.some((other) => other.item.id === to)) {
          return { kind: 'refuse', message: `there is no item ${to} to inform` }
        }
        return links.some((link) => link.type === INFORMS && link.to === to) ? { kind: 'nothing' } : add()
      }

      // implements: the target is a feature of a proposal that has it.
      const [itemId] = to.split('#') as [string]
      const proposal = scan.items.find((other) => other.item.id === itemId)
      if (proposal === undefined) return { kind: 'refuse', message: `there is no item ${itemId} to implement a feature of` }
      const { kind } = proposal.item
      if (kind !== PROPOSAL) return { kind: 'refuse', message: `${itemId} is ${/^[aeiou]/.test(kind) ? 'an' : 'a'} ${kind}, not a proposal; ${IMPLEMENTS} links to a feature of a proposal, such as ${FEATURE_TARGET_EXAMPLE}` }
      if (itemId === id) return { kind: 'refuse', message: `${id} cannot implement its own feature` }
      const features = featuresOf(proposal)
      const target = parseFeatureTarget(to)
      if (!target.ok) return { kind: 'refuse', message: `${IMPLEMENTS} links to a feature: give ${itemId}#F<n>; ${describeFeatures(features)}` }
      if (!features.some((feature) => feature.id === target.feature)) return { kind: 'refuse', message: `${itemId} has no feature ${target.feature}; ${describeFeatures(features)}` }
      return links.some((link) => link.type === IMPLEMENTS && link.to === to) ? { kind: 'nothing' } : add()
    },
  },
}

export const OP_NAMES = [...Object.keys(OPS), CREATE, REID]

/** Reads the item back from disk, so the reply is what is on disk. */
async function readBack(found: FoundItem, absolute: string): Promise<FoundItem> {
  const read = await readItemFile(absolute)
  if (!read.ok) throw new Error(`${found.path} does not read back after the write: ${read.reason}`)
  const checked = readItem(read.frontmatter)
  if (!checked.ok) throw new Error(`${found.path} does not read back after the write: ${checked.reason}`)
  return { ...found, item: checked.item, body: read.body, bodyLine: read.bodyLine, version: read.version }
}

async function writeLocked(
  root: string,
  id: string,
  version: string,
  spec: OpSpec,
  args: Record<string, unknown>,
  layout: Layout,
  privateDir: string | null,
  lockedRoot: RootName,
): Promise<ApplyResultInput> {
  const scan = await scanEpics(root, null, layout, privateDir)
  const found = scan.items.find((candidate) => candidate.item.id === id)
  if (found === undefined) return refused(`there is no item ${id}`)
  // The lock held is the lock of the root the item was in when the op arrived: a file copied to the other root since
  // then is not written under it.
  if (found.root !== lockedRoot) {
    return refused(`${id} moved to the ${found.root} root while the write waited; send the op again`)
  }
  const { absolute } = found

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
    files: after.root === 'private' ? [] : [after.path],
    items: [{ key, version: after.version, from: stateOf(found.item.kind, found.item.status), to: stateOf(after.item.kind, after.item.status) }],
  }
}

/** Runs `work` holding each lock in turn, the first outermost. Every writer takes them in the same order. */
function withLocks<T>(paths: string[], work: () => Promise<T>, options: LockOptions): Promise<T> {
  return paths.reduceRight((inner, path) => () => withLock(path, inner, options), work)()
}

/** Whether any of the folders is there. */
async function anyFolder(paths: string[]): Promise<boolean> {
  for (const path of paths) {
    try {
      await stat(path)
      return true
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error
    }
  }
  return false
}

/**
 * Protocol v0's `apply` for the markdown provider. `mainRoot` is the main checkout, and a write goes there unless
 * `checkout` names another checkout of the repository (an entry of `git worktree list`; anything else is refused and
 * writes nothing). The shared root is `<checkout>/<layout.root>` (`.collide/planning` by default), and every write
 * to a shared item holds that checkout's own lock file in it, whichever shared root the item is in: a write to an item
 * under a `previous` root changes that file and no other. `create` and `reid` still see every ref when they number or
 * draw an id. `version` is required: a write must say which content it was made against. An op that would change
 * nothing writes nothing and answers `ok` with no `files` and no `items`. Two ops are not about one version of one
 * item: `create` makes an item, so it takes no id and no version, and `reid` rewrites links in several files, each
 * read under the lock, so it takes no version (it refuses one rather than ignore it). `hooks` are for tests.
 *
 * `packPlanning` is the private root, `<pack data>/planning`, or null when the pack has no data folder. It belongs to
 * the main checkout: a call that names another checkout neither reads nor writes it. A write to a private item holds
 * the lock file in the private root and touches nothing in the repository; `reid`, which may rewrite files in both
 * roots, holds the lock of each root that exists, the shared one first.
 */
export async function applyOp(
  mainRoot: string,
  params: ApplyParamsOutput,
  lock: LockOptions = {},
  hooks: IdHooks = {},
  layout: Layout = DEFAULT_LAYOUT,
  packPlanning: string | null = null,
): Promise<ApplyResultInput> {
  const { op, id, version, checkout } = params
  const spec = Object.hasOwn(OPS, op) ? OPS[op] : undefined
  if (spec === undefined && op !== CREATE && op !== REID) return refused(`unknown op "${op}"; this provider has ${OP_NAMES.join(', ')}`)
  const args = params.args ?? {}

  // The checkout to write in: the main checkout unless one is named. The item is found, version-checked, locked (its
  // own lock file) and written there.
  let root = mainRoot
  if (checkout !== undefined && checkout !== null) {
    try {
      root = await resolveCheckout(mainRoot, checkout)
    } catch (error) {
      if (error instanceof CheckoutError) return refused(`${op}: ${error.message}`)
      throw error
    }
  }
  const planning = join(root, layout.root)
  const privateDir = packPlanning !== null && (await isMainCheckout(mainRoot, root)) ? packPlanning : null
  const lockOf = { shared: join(planning, LOCK_FILE), private: join(privateDir ?? '', LOCK_FILE) }
  const folderOf = { shared: planning, private: privateDir ?? '' }

  if (op === CREATE) {
    if (id !== undefined && id !== null) return refused('create makes the id: leave id out')
    if (version !== undefined && version !== null) return refused('create takes no version: it makes a new item')
    const problem = checkCreate(args)
    if (problem !== null) return refused(`create: ${problem}`)
    const into = rootOfCreate(args)
    if (into === 'private' && packPlanning === null) return refused(NO_PACK_DATA)
    if (into === 'private' && privateDir === null) return refused(NOT_MAIN)
    await mkdir(folderOf[into], { recursive: true })
    return withLock(lockOf[into], () => createLocked(root, args, hooks, layout, privateDir), lock)
  }

  if (typeof id !== 'string' || id === '') return refused(`${op} needs the id of the item to change`)
  if (op === REID) {
    if (version !== undefined && version !== null) return refused('reid takes no version: it rewrites several files, each read under the lock')
    const problem = checkReid(args)
    if (problem !== null) return refused(`reid: ${problem}`)
  } else if (typeof version !== 'string' || version === '') {
    return refused(`${op} needs the version you last read: send it, so that a write over a newer file can be told from one over the file you saw`)
  }
  const problem = spec === undefined ? null : spec.check(args)
  if (problem !== null) return refused(`${op}: ${problem}`)

  // The roots to lock. A write to one item locks the root that item is in; a reid may rewrite files in any root, so it
  // locks each that exists. A lock file lives in its root, which is made when it has items to write: a shared root
  // that is not there yet is made when a previous root has items.
  const sharedFolders = [...rootsOf(layout), ...spikesDirsOf(layout)].map((path) => join(root, path))
  const locked: RootName[] = []
  if (op === REID || privateDir === null) {
    if (await anyFolder(sharedFolders)) locked.push('shared')
    if (op === REID && privateDir !== null && (await anyFolder([privateDir]))) locked.push('private')
  } else {
    const holder = (await scanEpics(root, null, layout, privateDir)).items.find((candidate) => candidate.item.id === id)
    if (holder !== undefined) locked.push(holder.root)
  }
  if (locked.length === 0) return refused(`there is no item ${id}`)
  for (const name of locked) await mkdir(folderOf[name], { recursive: true })
  const work =
    spec === undefined
      ? () => reidLocked(root, id, args, hooks, layout, privateDir)
      : () => writeLocked(root, id, version as string, spec, args, layout, privateDir, locked[0] as RootName)
  return withLocks(locked.map((name) => lockOf[name]), work, lock)
}
