import { lstat, realpath } from 'node:fs/promises'
import { isAbsolute, join, relative, resolve, sep } from 'node:path'
import { CONTROL } from './op-util.ts'

/** The shared root when no layout is set (0.10, Planning sources). */
export const DEFAULT_ROOT = '.collide/planning'

/**
 * Where the shared planning folders are, as the `layout` setting gives them, checked and with every path written as
 * the repository sees it: relative, with `/`, and with a symbolic link inside the repository resolved.
 */
export interface Layout {
  /** New items go to `<root>/epics/`. */
  root: string
  /** Roots that held items before `root` changed. Listed and written in place, never written to anew. */
  previous: string[]
  /** `paths.spike`: where spike folders go. Nothing reads it before the spike workflow (ts-0126-nf). */
  spike: string | null
  /** `code`: accepted, and used by Stage 9's spike workflow. */
  code: string | null
}

export const DEFAULT_LAYOUT: Layout = { root: DEFAULT_ROOT, previous: [], spike: null, code: null }

/** A layout that cannot be used. The message names the key and the rule; `initialize` fails with it. */
export class LayoutError extends Error {}

/** The roots to read, the current one first. */
export const rootsOf = (layout: Layout): string[] => [layout.root, ...layout.previous]

/** The epics folder of each root, as git writes a path. */
export const epicsDirsOf = (layout: Layout): string[] => rootsOf(layout).map((root) => `${root}/epics`)

const KEYS = ['root', 'previous', 'paths', 'code']

/** The Collide directory's own files, which a planning folder may not sit over (0.9). Compared in lower case. */
const PROTECTED = ['board.yaml', 'packs.lock', 'config.yaml', 'views', 'profiles']

const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)
const missing = (error: unknown): boolean => ['ENOENT', 'ENOTDIR'].includes((error as NodeJS.ErrnoException).code ?? '')

/** The refusal for `parts` (names, in order, from the repository root), or null. Case is ignored: a file system may. */
function ruleOf(parts: string[]): string | null {
  const lower = parts.map((part) => part.toLowerCase())
  if (lower[0] === '.git') return 'is under .git/, which is git\'s own folder'
  if (lower[0] === '.collide' && lower[1] !== undefined && PROTECTED.includes(lower[1])) {
    return `is a protected file of the Collide directory, or under one: it would cover .collide/${lower[1]}`
  }
  return null
}

const partsOf = (path: string): string[] => path.split(sep).filter((part) => part !== '')

/**
 * Follows `parts` down from the repository root, one name at a time. A name that is a symbolic link is resolved, and
 * the walk goes on from where it leads; one that leads out of the repository, or to nothing, is an error. Names that
 * do not exist yet are taken as they are. Returns the parts the path has with every link resolved, and the first link
 * met.
 */
async function resolveLinks(
  label: string,
  text: string,
  parts: string[],
  repo: string,
): Promise<{ resolved: string[]; via: string | null }> {
  let real = repo
  let via: string | null = null
  for (const [index, part] of parts.entries()) {
    const next = join(real, part)
    let info
    try {
      info = await lstat(next)
    } catch (error) {
      if (missing(error)) return { resolved: [...partsOf(relative(repo, real)), ...parts.slice(index)], via }
      throw error
    }
    if (!info.isSymbolicLink()) {
      real = next
      continue
    }
    const link = parts.slice(0, index + 1).join('/')
    let target: string
    try {
      target = await realpath(next)
    } catch (error) {
      if (missing(error) || (error as NodeJS.ErrnoException).code === 'ELOOP') {
        throw new LayoutError(`${label} "${text}" goes through the symbolic link "${link}", which points at nothing`)
      }
      throw error
    }
    const inside = relative(repo, target)
    if (inside === '..' || inside.startsWith(`..${sep}`) || isAbsolute(inside)) {
      throw new LayoutError(`${label} "${text}" goes through the symbolic link "${link}", which leaves the repository`)
    }
    via ??= link
    real = target
  }
  return { resolved: partsOf(relative(repo, real)), via }
}

/** Checks one path of the layout against the rules and returns it as the repository sees it. */
async function checkPath(label: string, given: unknown, repo: string): Promise<string> {
  if (typeof given !== 'string') throw new LayoutError(`${label} must be text, a path inside the repository`)
  const text = given
  const refuse = (rule: string): LayoutError => new LayoutError(`${label} "${text}" ${rule}`)
  if (CONTROL.test(text)) throw refuse('has a control character')
  if (text.includes('\\')) throw refuse('has a backslash: write the path with / between folders')
  if (isAbsolute(text) || /^[A-Za-z]:/.test(text)) {
    throw refuse('is an absolute path: it must be relative to the repository')
  }
  const parts = text.split('/').filter((part) => part !== '' && part !== '.')
  if (parts.includes('..')) throw refuse('has a ".." part: it must stay inside the repository')
  if (parts.length === 0) throw refuse('is empty, or only ".": name a folder inside the repository')
  const direct = ruleOf(parts)
  if (direct !== null) throw refuse(direct)

  const { resolved, via } = await resolveLinks(label, text, parts, repo)
  const followed = ruleOf(resolved)
  if (followed !== null) throw refuse(`goes through the symbolic link "${via}" and ${followed}`)
  return resolved.join('/')
}

/**
 * Reads the `layout` setting for the repository at `repoRoot` and checks every path in it. `given` is the setting's
 * value, or undefined when there is none, which is the default layout. Throws a `LayoutError` for the first problem.
 */
export async function resolveLayout(repoRoot: string, given: unknown): Promise<Layout> {
  if (given === undefined) given = {}
  if (!isRecord(given)) throw new LayoutError('layout must be an object, with the keys root, previous, paths and code')
  const unknown = Object.keys(given).find((key) => !KEYS.includes(key))
  if (unknown !== undefined) {
    throw new LayoutError(`layout.${unknown} is not a key of layout; its keys are ${KEYS.join(', ')}`)
  }

  const repo = await realpath(repoRoot).catch(() => resolve(repoRoot))
  const root = given.root === undefined ? DEFAULT_ROOT : await checkPath('layout.root', given.root, repo)

  const previous: string[] = []
  if (given.previous !== undefined) {
    if (!Array.isArray(given.previous)) throw new LayoutError('layout.previous must be a list of paths')
    for (const [index, entry] of given.previous.entries()) {
      const path = await checkPath(`layout.previous[${index}]`, entry, repo)
      if (path !== root && !previous.includes(path)) previous.push(path)
    }
  }

  let spike: string | null = null
  if (given.paths !== undefined) {
    if (!isRecord(given.paths)) throw new LayoutError('layout.paths must be an object, with the key spike')
    for (const [kind, path] of Object.entries(given.paths)) {
      if (kind !== 'spike') {
        throw new LayoutError(
          `layout.paths.${kind}: only spike can be placed: the other kinds live in their epic's folder`,
        )
      }
      spike = await checkPath('layout.paths.spike', path, repo)
    }
  }

  const code = given.code === undefined ? null : await checkPath('layout.code', given.code, repo)
  return { root, previous, spike, code }
}
