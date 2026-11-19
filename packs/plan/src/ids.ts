import { execFile } from 'node:child_process'
import { readdir } from 'node:fs/promises'
import { join } from 'node:path'
import { promisify } from 'node:util'
import { DEFAULT_LAYOUT, epicsDirsOf, spikesDirsOf } from './layout.ts'
import type { Layout } from './layout.ts'

const run = promisify(execFile)

/** The epics folders of the default layout, relative to the repository root, as git writes a path. */
const DEFAULT_EPICS = epicsDirsOf(DEFAULT_LAYOUT)

/** The folders of the default layout that hold spike folders, relative to the repository root. */
const DEFAULT_SPIKES = spikesDirsOf(DEFAULT_LAYOUT)

/** Each kind's id prefix and the digits its numbers are padded to (0.10, ids). */
export const KIND_IDS = {
  task: { prefix: 'ts', width: 4 },
  epic: { prefix: 'ep', width: 3 },
  proposal: { prefix: 'pr', width: 4 },
  spike: { prefix: 'sp', width: 4 },
  question: { prefix: 'qu', width: 4 },
} as const
export type CreatableKind = keyof typeof KIND_IDS

/** The letters of a suffix: a to z without `i`, `l` and `o`, which read as part of the number. 23 letters, 529 suffixes. */
export const SUFFIX_LETTERS = 'abcdefghjkmnpqrstuvwxyz'

/** A draw gives up after this many tries: 529 suffixes per number, so a number that has used them all stops here. */
const MAX_DRAWS = 5_000

const FILE_ID = /^([a-z]{2}-[0-9]{3,}-[a-z]{2})(?:-.*)?\.md$/
const FOLDER_ID = /^(ep-[0-9]{3,}-[a-z]{2})(?:-.*)?$/
const SPIKE_FOLDER_ID = /^(sp-[0-9]{3,}-[a-z]{2})(?:-.*)?$/

/**
 * Test hooks, never set by a real run: `generator` makes the number generator from a seed (a test rigs it to return a
 * taken id first), and `look` reads the ids that are taken (a test makes a ref appear between two looks).
 */
export interface IdHooks {
  generator?: (seed: number) => () => number
  look?: (root: string, layout: Layout, privateDir: string | null) => Promise<Set<string>>
}

/** A problem reading the ids that other refs hold. The op that asked refuses with the message. */
export class IdSourceError extends Error {}

/** The folders of an epic folder that hold item files, one level deep. */
const ITEM_FOLDERS = ['tasks', 'proposals', 'questions']

/**
 * The ids that the item paths name. A path is relative to the repository root, with `/`, and only a path under one of
 * `epicsDirs` (the epics folders of the layout's roots) or `spikesDirs` (the folders that hold spike folders)
 * counts. An epic folder names its epic (whatever is inside it), and a file directly in a `tasks/`, `proposals/` or
 * `questions/` folder names its item. A spike folder names its spike, whatever is inside it. Nothing is opened: the id
 * is the start of the name (0.10, ids: names on disk are `<id>-<slug>`).
 */
export function idsInPaths(
  paths: string[],
  epicsDirs: readonly string[] = DEFAULT_EPICS,
  spikesDirs: readonly string[] = DEFAULT_SPIKES,
): Set<string> {
  const ids = new Set<string>()
  for (const path of paths) {
    const spikes = spikesDirs.find((candidate) => path.startsWith(`${candidate}/`))
    if (spikes !== undefined) {
      const spike = SPIKE_FOLDER_ID.exec(path.slice(spikes.length + 1).split('/')[0] ?? '')?.[1]
      if (spike !== undefined) ids.add(spike)
      continue
    }
    const dir = epicsDirs.find((candidate) => path.startsWith(`${candidate}/`))
    if (dir === undefined) continue
    const [folder, subfolder, file, ...rest] = path.slice(dir.length + 1).split('/')
    const epic = FOLDER_ID.exec(folder ?? '')?.[1]
    if (epic === undefined) continue
    ids.add(epic)
    if (subfolder !== undefined && ITEM_FOLDERS.includes(subfolder) && file !== undefined && rest.length === 0) {
      const id = FILE_ID.exec(file)?.[1]
      if (id !== undefined) ids.add(id)
    }
  }
  return ids
}

async function git(root: string, args: string[]): Promise<string> {
  try {
    const { stdout } = await run('git', ['-C', root, ...args], { encoding: 'utf8', maxBuffer: 256 * 1024 * 1024, env: { ...process.env, GIT_OPTIONAL_LOCKS: '0' } })
    return stdout
  } catch (error) {
    const detail = (error as { stderr?: string }).stderr?.trim() || (error as Error).message
    throw new IdSourceError(`could not read the refs of ${root} with git (${args[0]}): ${detail}`)
  }
}

/**
 * The ids on every local branch and fetched remote ref, under the layout's roots. Refs on one commit are read once. A
 * spikes folder is listed one level down, so the files inside a spike folder are never listed.
 */
async function idsInRefs(root: string, epicsDirs: string[], spikesDirs: string[]): Promise<Set<string>> {
  const listing = await git(root, ['for-each-ref', '--format=%(objectname)', 'refs/heads', 'refs/remotes'])
  const ids = new Set<string>()
  const namesOf = (output: string): string[] => output.split('\0').filter((name) => name !== '')
  for (const commit of new Set(listing.split('\n').filter((line) => line !== ''))) {
    const inEpics = await git(root, ['ls-tree', '-r', '--name-only', '-z', commit, '--', ...epicsDirs])
    const spikePaths = spikesDirs.map((dir) => `${dir}/`)
    const inSpikes = await git(root, ['ls-tree', '--name-only', '-z', commit, '--', ...spikePaths])
    for (const id of idsInPaths([...namesOf(inEpics), ...namesOf(inSpikes)], epicsDirs, spikesDirs)) ids.add(id)
  }
  return ids
}

/**
 * The ids in the working tree: the epic folders and the files in their `tasks/`, `proposals/` and `questions/` folders,
 * and the spike folders, as `scan.ts` finds them. A spike folder is named and never opened.
 */
async function idsInWorkingTree(root: string, epicsDirs: string[], spikesDirs: string[]): Promise<Set<string>> {
  const names = async (dir: string): Promise<string[]> => {
    try {
      return await readdir(dir)
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === 'ENOENT' || (error as NodeJS.ErrnoException).code === 'ENOTDIR') return []
      throw error
    }
  }
  const paths: string[] = []
  for (const epics of epicsDirs) {
    for (const folder of await names(join(root, epics))) {
      paths.push(`${epics}/${folder}/`)
      for (const subfolder of ITEM_FOLDERS) {
        for (const file of await names(join(root, epics, folder, subfolder))) {
          paths.push(`${epics}/${folder}/${subfolder}/${file}`)
        }
      }
    }
  }
  for (const spikes of spikesDirs) {
    for (const folder of await names(join(root, spikes))) paths.push(`${spikes}/${folder}/`)
  }
  return idsInPaths(paths, epicsDirs, spikesDirs)
}

/**
 * Every id Collide can see under the layout's roots: local branches, fetched remote refs and the working tree. The
 * private root, when there is one, is read from disk only: a private id is never looked for in a ref.
 */
export async function takenIds(
  root: string,
  layout: Layout = DEFAULT_LAYOUT,
  privateDir: string | null = null,
): Promise<Set<string>> {
  const epicsDirs = epicsDirsOf(layout)
  const spikesDirs = spikesDirsOf(layout)
  const ids = await idsInRefs(root, epicsDirs, spikesDirs)
  for (const id of await idsInWorkingTree(root, epicsDirs, spikesDirs)) ids.add(id)
  if (privateDir !== null) for (const id of await idsInWorkingTree(privateDir, ['epics'], ['spikes'])) ids.add(id)
  return ids
}

/** One more than the highest number any id with `prefix` has, so a new number never repeats one that is visible. */
export function nextNumber(taken: Set<string>, prefix: string): number {
  let highest = 0
  for (const id of taken) {
    const [head, number] = id.split('-')
    if (head === prefix) highest = Math.max(highest, Number(number))
  }
  return highest + 1
}

/**
 * The seeded generator (mulberry32): a function that gives numbers in [0, 1) from a 32-bit seed. A test pins the
 * suffixes that a seed gives, so changing this changes ids that people have been told, and the test fails.
 */
export function seededGenerator(seed: number): () => number {
  let state = seed >>> 0
  return () => {
    state = (state + 0x6d2b79f5) >>> 0
    let t = state
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
}

/** Two letters: the first draw gives the first letter, the second draw the second. */
export function drawSuffix(next: () => number): string {
  const letter = (): string => SUFFIX_LETTERS[Math.floor(next() * SUFFIX_LETTERS.length)] as string
  return `${letter()}${letter()}`
}

export const formatId = (prefix: string, number: number, suffix: string, width: number): string => `${prefix}-${String(number).padStart(width, '0')}-${suffix}`

/**
 * Draws suffixes until `<prefix>-<number>-<suffix>` is free, and returns it. Each draw is checked against a fresh look
 * at every ref and the working tree, because a fetch can land between the first look and the write.
 */
export async function drawFreeId(options: { prefix: string; number: number; width: number; next: () => number; look: () => Promise<Set<string>> }): Promise<string> {
  const { prefix, number, width, next, look } = options
  for (let draws = 0; draws < MAX_DRAWS; draws++) {
    const id = formatId(prefix, number, drawSuffix(next), width)
    if (!(await look()).has(id)) return id
  }
  throw new Error(`no free suffix for ${prefix}-${String(number).padStart(width, '0')} after ${MAX_DRAWS} draws: all ${SUFFIX_LETTERS.length ** 2} may be taken`)
}
