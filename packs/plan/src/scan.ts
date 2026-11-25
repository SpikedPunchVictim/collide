import { lstat, readdir } from 'node:fs/promises'
import { join, relative, sep } from 'node:path'
import type { ListResultInput, TaskInput, TaskInvalidOutput } from '@collide/pack-sdk'
import { readAssumptions } from './assumptions.ts'
import type { Assumption } from './assumptions.ts'
import { readDecisions } from './decisions.ts'
import type { Decision, ItemLookup } from './decisions.ts'
import { readClaims } from './claims.ts'
import type { Claim } from './claims.ts'
import { coverageOf } from './coverage.ts'
import { parseFeatures } from './features.ts'
import { readFindings } from './findings.ts'
import type { Finding } from './findings.ts'
import { KNOWN_KINDS, PROPOSAL, readItem, SPIKE, stateOf } from './item.ts'
import type { Item } from './item.ts'
import { DEFAULT_LAYOUT, rootsOf, spikesDirsOf } from './layout.ts'
import type { Layout } from './layout.ts'
import { featuresOf, isBrokenLink } from './links.ts'
import type { PartProblem } from './parts.ts'
import { readPreregistration } from './preregistration.ts'
import type { Preregistration } from './preregistration.ts'
import { readItemFile } from './read-file.ts'

const EPIC_FOLDER = /^(ep-[0-9]{3,}-[a-z]{2})(-.*)?$/
const SPIKE_FOLDER = /^sp-[0-9]{3,}-[a-z]{2}(-.*)?$/

/** The one file of a spike folder that is read (0.10, Planning sources). */
export const SPIKE_RECORD = 'record.md'

/** One item that was read and checked, with where it was found. */
export interface FoundItem {
  item: Item
  epic: string | null
  /** Which root the file is in. A private item is not a file of the checkout (see `PRIVATE_PREFIX`). */
  root: RootName
  /** The path the provider reports: from the checkout for a shared item, `private:` and then from the private root. */
  path: string
  /** Where the file is on disk. */
  absolute: string
  body: string
  /** The line of the file that `body` starts on. */
  bodyLine: number
  version: string
}

export interface Scan {
  items: FoundItem[]
  invalid: TaskInvalidOutput[]
  /** Items whose kind is not one of `KNOWN_KINDS`. They are listed, not dropped. */
  unknownKind: number
  /** Items with a link whose target is no item that was read, or no feature that its proposal has. They are listed, not dropped. */
  brokenLink: number
}

export type RootName = 'shared' | 'private'

/** The start of the `path` of a private item, so that it is never taken for a file of the checkout (0.10). */
export const PRIVATE_PREFIX = 'private:'

const posix = (cwd: string, absolute: string): string => relative(cwd, absolute).split(sep).join('/')

/** The path to report for the file at `absolute`: from `cwd` for a shared file, or `private:` and from `privateDir`. */
export function pathOf(cwd: string, privateDir: string | null, absolute: string): string {
  if (privateDir !== null && absolute.startsWith(`${privateDir}${sep}`)) {
    return `${PRIVATE_PREFIX}${posix(privateDir, absolute)}`
  }
  return posix(cwd, absolute)
}

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
 * Reads the items under `<cwd>/<root>/epics/` for the layout's root and then for each of its `previous` roots (the
 * default root, `.collide/planning`, when no layout is given), and then under `<privateDir>/epics/` when there is a
 * private root: each epic folder's `epic.md` and the `.md` files directly in its `tasks/`, `proposals/` and
 * `questions/` folders. Then the spikes: each folder of the layout's spikes folders (`spikesDirsOf`), and of
 * `<privateDir>/spikes/`, that is named for a spike id has its `record.md` read. Nothing else is read. A spike folder
 * is never listed, so what is inside it (`out/`, `work/`, `node_modules/`, another `record.md`) costs nothing and is
 * never an item. Symbolic links are not followed. A `questions/` folder anywhere but directly in an epic folder is
 * never opened. The epic is the folder, so it is reported from the path and never from frontmatter, and a spike has
 * none.
 * Every file that reads is returned, in scan order, including a second file with an id another file has (`scanEpics`
 * sets that one aside); `invalid` holds the files that did not read.
 */
export async function readEpicFiles(
  cwd: string,
  checkout: string | null = null,
  layout: Layout = DEFAULT_LAYOUT,
  privateDir: string | null = null,
): Promise<{ items: FoundItem[]; invalid: TaskInvalidOutput[] }> {
  const items: FoundItem[] = []
  const invalid: TaskInvalidOutput[] = []
  const reject = (absolute: string, reason: string): void =>
    void invalid.push({ path: pathOf(cwd, privateDir, absolute), checkout, reason })

  const readOne = async (absolute: string, epic: string | null, root: RootName): Promise<void> => {
    const read = await readItemFile(absolute)
    if (!read.ok) return reject(absolute, read.reason)
    const checked = readItem(read.frontmatter)
    if (!checked.ok) return reject(absolute, checked.reason)
    const path = pathOf(cwd, privateDir, absolute)
    const { body, bodyLine, version } = read
    items.push({ item: checked.item, epic, root, path, absolute, body, bodyLine, version })
  }

  const roots: Array<{ dir: string; name: RootName }> = rootsOf(layout).map((root) => ({
    dir: join(cwd, root),
    name: 'shared',
  }))
  if (privateDir !== null) roots.push({ dir: privateDir, name: 'private' })
  for (const { dir, name } of roots) {
    const epicsDir = join(dir, 'epics')
    for (const folder of (await entries(epicsDir)).filter((entry) => entry.isDirectory())) {
      const epicDir = join(epicsDir, folder.name)
      const epic = EPIC_FOLDER.exec(folder.name)?.[1]
      if (epic === undefined) {
        reject(epicDir, `epic folder "${folder.name}" must start with an epic id such as ep-002-va`)
        continue
      }
      for (const file of (await entries(epicDir)).filter((entry) => entry.isFile() && entry.name === 'epic.md')) {
        await readOne(join(epicDir, file.name), null, name)
      }
      for (const subfolder of ['tasks', 'proposals', 'questions']) {
        const inside = await entries(join(epicDir, subfolder))
        const files = inside.filter((entry) => entry.isFile() && entry.name.endsWith('.md'))
        for (const file of files) await readOne(join(epicDir, subfolder, file.name), epic, name)
      }
    }
  }

  const spikeRoots: Array<{ dir: string; name: RootName }> = spikesDirsOf(layout).map((dir) => ({
    dir: join(cwd, dir),
    name: 'shared',
  }))
  if (privateDir !== null) spikeRoots.push({ dir: join(privateDir, 'spikes'), name: 'private' })
  for (const { dir, name } of spikeRoots) {
    for (const folder of (await entries(dir)).filter((entry) => entry.isDirectory())) {
      const spikeDir = join(dir, folder.name)
      if (!SPIKE_FOLDER.test(folder.name)) {
        reject(spikeDir, `spike folder "${folder.name}" must start with a spike id such as sp-0005-rt`)
        continue
      }
      const record = join(spikeDir, SPIKE_RECORD)
      try {
        // A folder with no record, or a record that is a link, is no item, as an epic folder with no epic.md is none.
        if (!(await lstat(record)).isFile()) continue
      } catch (error) {
        const code = (error as NodeJS.ErrnoException).code
        if (code === 'ENOENT' || code === 'ENOTDIR') continue
        reject(record, `the record cannot be read: ${code ?? String(error)}`)
        continue
      }
      await readOne(record, null, name)
    }
  }

  return { items, invalid }
}

/** The items under `epics/` and `spikes/` of every root of the layout, and of the private root (`readEpicFiles`). */
export async function scanEpics(
  cwd: string,
  checkout: string | null = null,
  layout: Layout = DEFAULT_LAYOUT,
  privateDir: string | null = null,
): Promise<Scan> {
  const { items, invalid } = await readEpicFiles(cwd, checkout, layout, privateDir)
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
  const brokenLink = kept.filter((found) => found.item.links.some((link) => isBrokenLink(link, seen))).length
  return { items: kept, invalid, unknownKind, brokenLink }
}

const EPIC = 'epic'

/** A collection as the provider lists it: an epic, with no groups and its `epic.md` as its path. */
export type EpicCollection = NonNullable<ListResultInput['collections']>[number]

/** The key an epic is a collection under: its id, in this provider's source. */
const collectionOf = (epicId: string): string => `plan:${epicId}`

/**
 * Each epic that has an `epic.md` as a collection (0.10; ts-0187-hb), in scan order: its key, its title and its file
 * (`ts-0216-ve`: the app says where the epic is). The folder is still the membership, and `part-of` still says so: an
 * item is also named in the collections of its folder's epic.
 */
export function collectionsOf(items: readonly FoundItem[]): EpicCollection[] {
  return items
    .filter((found) => found.item.kind === EPIC)
    .map((found) => ({ id: collectionOf(found.item.id), title: found.item.title, groups: [], path: found.path }))
}

/** What `list` answers for a scan: every item with no body, the epics as collections, and the invalid files. */
export function listedOf({ items, invalid }: Pick<Scan, 'items' | 'invalid'>) {
  return { tasks: items.map((found) => toTask(found, false, items)), collections: collectionsOf(items), invalid }
}

/** What the scan knows of an item, for an evidence id: the first item of that id in `items`. */
const lookupIn =
  (items: readonly FoundItem[]): ItemLookup =>
  (id) => {
    const found = items.find((candidate) => candidate.item.id === id)
    if (found === undefined) return undefined
    const features = found.item.kind === PROPOSAL ? featuresOf(found).map((feature) => feature.id) : []
    return { status: found.item.status, features, kind: found.item.kind }
  }

interface QualityParts {
  decisions: Decision[]
  assumptions: Assumption[]
  findings: Finding[]
  claims: Claim[]
  preregistration: Preregistration | null
  problems: PartProblem[]
}

const NO_PARTS: QualityParts = {
  decisions: [],
  assumptions: [],
  findings: [],
  claims: [],
  preregistration: null,
  problems: [],
}

/** A spike's claim blocks and Pre-registration part, with the problems of both in line order. */
function spikePartsOf(found: FoundItem): QualityParts {
  const { body, bodyLine, item } = found
  const claimed = readClaims(body, bodyLine)
  const registered = readPreregistration(body, bodyLine, item.fields.preregistered)
  const problems = [...claimed.problems, ...registered.problems].sort((a, b) => a.line - b.line)
  return { ...NO_PARTS, claims: claimed.claims, preregistration: registered.preregistration, problems }
}

/**
 * A proposal's Decision log, Assumption ledger and Findings, or a spike's claims and Pre-registration, with the
 * problems of its parts in line order. Any other kind has none.
 */
function qualityPartsOf(found: FoundItem, featureIds: string[], items: readonly FoundItem[]): QualityParts {
  if (found.item.kind === SPIKE) return spikePartsOf(found)
  if (found.item.kind !== PROPOSAL) return NO_PARTS
  const lookup = lookupIn(items)
  const { body, bodyLine } = found
  const log = readDecisions(body, bodyLine, lookup)
  const ledger = readAssumptions(body, bodyLine, lookup)
  const own = new Set(featureIds)
  for (const one of [...log.decisions, ...ledger.assumptions]) own.add(one.id)
  const reviewed = readFindings(body, bodyLine, own)
  const problems = [...log.problems, ...ledger.problems, ...reviewed.problems].sort((a, b) => a.line - b.line)
  const { decisions } = log
  return { ...NO_PARTS, decisions, assumptions: ledger.assumptions, findings: reviewed.findings, problems }
}

/**
 * One item as the provider reports it. `body` is left out of `list`, which carries every item. A proposal also carries
 * its features and the problems of its Features part, and its Decision log, Assumption ledger and Findings with the
 * problems of those parts, which are read from the body whether or not it is sent, and its `coverage`. `items` is the
 * scan, which says what an evidence id or a spike names and which tasks implement its features; a finding's
 * `Target:` is read against the proposal's own features, decisions and assumptions.
 */
export function toTask(found: FoundItem, withBody: boolean, items: readonly FoundItem[]): TaskInput {
  const { item } = found
  const isProposal = item.kind === PROPOSAL
  const { features, problems } = isProposal ? parseFeatures(found.body, found.bodyLine) : { features: [], problems: [] }
  const featureIds = features.map((feature) => feature.id)
  const quality = qualityPartsOf(found, featureIds, items)
  const coverage = coverageOf(found, featureIds, items)
  const partOf = found.epic === null ? [] : [{ type: 'part-of', to: found.epic }]
  // An item is in the collection of its folder's epic when that epic has a file. An epic and a spike are in none.
  const epicId = found.epic
  const collections =
    epicId !== null && items.some((other) => other.item.kind === EPIC && other.item.id === epicId)
      ? [collectionOf(epicId)]
      : []
  // A private item says so in `fields.root`, over any `root` field the file has.
  const own = Object.keys(item.fields).length === 0 ? null : item.fields
  const fields = found.root === 'private' ? { ...item.fields, root: 'private' } : own
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
    ...(collections.length === 0 ? {} : { collections }),
    ...(fields === null ? {} : { fields }),
    ...(features.length === 0 ? {} : { features }),
    ...(problems.length === 0 ? {} : { featureProblems: problems }),
    ...(quality.decisions.length === 0 ? {} : { decisions: quality.decisions }),
    ...(quality.assumptions.length === 0 ? {} : { assumptions: quality.assumptions }),
    ...(quality.findings.length === 0 ? {} : { findings: quality.findings }),
    ...(quality.problems.length === 0 ? {} : { partProblems: quality.problems }),
    ...(coverage === null ? {} : { coverage }),
    ...(quality.claims.length === 0 ? {} : { claims: quality.claims }),
    ...(quality.preregistration === null ? {} : { preregistration: quality.preregistration }),
  }
}
