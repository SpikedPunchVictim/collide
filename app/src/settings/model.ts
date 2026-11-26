// What the Settings page shows, worked out from what the daemon answers (ts-0193-gk; note 12 of the board mock). Every
// function here is pure: the page reads `config.get` and `config.pack_list` and these turn the answers into rows.
// The wire has no display names, so Collide's own five keys carry the mock's words below; a pack's key is its own name.
import type { SourceDecl } from '../field/types.ts'
import type { ConfigGetResultOutput, ConfigPackListResultOutput } from '../owner/generated.ts'

export type Listing = ConfigGetResultOutput
export type Entry = Listing['entries'][number]
export type Problem = NonNullable<Listing['problems']>[number]
export type Layer = Entry['layer']
export type Pack = ConfigPackListResultOutput['packs'][number]

/** The layer as a badge says it. A run's profile applies inside a run only, so the page rarely shows it. */
export const LAYER_WORD: Readonly<Record<Layer, string>> = {
  default: 'Default',
  user: 'You',
  repo: 'This repo',
  profile: 'Profile',
}

/** Whose value a struck-out one was, and who overrode it ("yours, overridden by this repo"). */
const OWNER_WORD: Readonly<Record<Layer, string>> = {
  default: 'the default',
  user: 'yours',
  repo: "this repo's",
  profile: "the profile's",
}
const OVERRIDER_WORD: Readonly<Record<Layer, string>> = {
  default: 'the default',
  user: 'you',
  repo: 'this repo',
  profile: 'the profile',
}

interface Known {
  readonly name: string
  readonly show: (value: unknown) => string
}

const text = (value: unknown): string => (typeof value === 'string' ? value : JSON.stringify(value))

/** Collide's own settings, in the mock's words. A key not listed shows as its key. */
const KNOWN: Readonly<Record<string, Known>> = {
  'ask.agent': { name: 'Agent that answers an Ask', show: text },
  'ask.timeout_s': { name: 'Time limit for one Ask', show: (value) => `${text(value)} s` },
  'ask.budget_usd': {
    name: 'Spend cap for one Ask',
    show: (value) => (typeof value === 'number' ? `$${value.toFixed(2)}` : text(value)),
  },
  'disk.floor_gib': { name: 'Free disk space to keep', show: (value) => `${text(value)} GiB` },
  'claude.check_published': {
    name: 'Check for new Claude Code versions',
    show: (value) => (value === true ? 'On, daily and at start' : value === false ? 'Off' : text(value)),
  },
}

/** What a setting is called: the mock's name for one of Collide's own, else its key. */
export function nameOf(key: string): string {
  return KNOWN[key]?.name ?? key
}

/** A value as the table shows it: with its unit for a known key, `No value` for none. */
export function valueText(key: string, value: unknown): string {
  if (value === undefined || value === null) return 'No value'
  const known = KNOWN[key]
  if (known !== undefined) return known.show(value)
  if (typeof value === 'boolean') return value ? 'On' : 'Off'
  return text(value)
}

export interface GeneralRow {
  readonly key: string
  readonly name: string
  readonly value: string
  readonly layer: Layer
  readonly layerWord: string
  /** The value a layer passed over, with the words that say whose it was, or null. */
  readonly struck: { readonly value: string; readonly words: string } | null
  /** What is wrong with the key's values, in the daemon's words. */
  readonly problems: readonly string[]
}

export interface General {
  readonly rows: readonly GeneralRow[]
  /** Problems that name no row of the table: a section, a profile key, an agent entry. */
  readonly loose: readonly Problem[]
}

const messagesFor = (problems: readonly Problem[], key: string): string[] =>
  problems.filter((problem) => problem.key === key).map((problem) => problem.message)

/** The rows of the General table: every setting that is no secret, with the problems that name it. */
export function general(listing: Listing): General {
  const problems = listing.problems ?? []
  const rows = listing.entries
    .filter((entry) => !entry.secret)
    .map((entry): GeneralRow => {
      const over = entry.overridden
      return {
        key: entry.key,
        name: nameOf(entry.key),
        value: valueText(entry.key, entry.value),
        layer: entry.layer,
        layerWord: LAYER_WORD[entry.layer],
        struck:
          over === undefined || over === null
            ? null
            : {
                value: valueText(entry.key, over.value),
                words: `${OWNER_WORD[over.layer]}, overridden by ${OVERRIDER_WORD[entry.layer]}`,
              },
        problems: messagesFor(problems, entry.key),
      }
    })
  const keys = new Set(listing.entries.map((entry) => entry.key))
  return { rows, loose: problems.filter((problem) => !keys.has(problem.key)) }
}

export interface SecretRow {
  readonly key: string
  /** The setting's own name, without its pack's. */
  readonly name: string
  /** The pack that asks for it. */
  readonly pack: string
  /** Whether the keychain holds it; null when the daemon could not say. */
  readonly set: boolean | null
  readonly problems: readonly string[]
}

/** The secrets the board's packs ask for. A key is `<pack>.<name>`: the name is what follows the last dot. */
export function secrets(listing: Listing): readonly SecretRow[] {
  const problems = listing.problems ?? []
  return listing.entries
    .filter((entry) => entry.secret)
    .map((entry) => {
      const dot = entry.key.lastIndexOf('.')
      return {
        key: entry.key,
        name: entry.key.slice(dot + 1),
        pack: dot < 0 ? '' : entry.key.slice(0, dot),
        set: entry.is_set ?? null,
        problems: messagesFor(problems, entry.key),
      }
    })
}

export interface PackRow {
  readonly name: string
  /** Where the pack is from, in words. */
  readonly from: string
  /** Its source, as the daemon names it. Null for a built-in pack, which has none to show. */
  readonly source: string | null
  /** The statuses the board's sources of this pack declare, in all; null when no source of the board is its. */
  readonly statuses: number | null
  readonly enabled: boolean
}

const ORIGIN_WORD: Readonly<Record<Pack['origin'], string>> = {
  builtin: 'Built in',
  repo: 'Brought by this repo',
  user: 'Added by you',
  offered: 'A folder of this checkout, not yet linked',
}

/** The board's packs, each with the statuses its sources declare. `sources` are the Field's. */
export function packRows(packs: readonly Pack[], sources: readonly SourceDecl[]): readonly PackRow[] {
  return packs.map((pack) => {
    const own = sources.filter((source) => source.pack === pack.name)
    return {
      name: pack.name,
      from: ORIGIN_WORD[pack.origin],
      source: pack.origin === 'builtin' ? null : pack.source,
      statuses: own.length === 0 ? null : own.reduce((sum, source) => sum + source.statuses.length, 0),
      enabled: pack.enabled,
    }
  })
}

/** The words of a count of statuses, as the mock's chip says them. */
export function statusesWord(count: number): string {
  return count === 1 ? '1 status' : `${count} statuses`
}

export interface FileRow {
  readonly label: string
  readonly path: string
  readonly exists: boolean
}

/** The files Details lists, as the daemon reports them. The defaults and a run's profile have no file of their own. */
export function files(listing: Listing): readonly FileRow[] {
  return (listing.files ?? []).map((file) => ({
    label: file.layer === 'repo' ? 'This repo' : file.for_board ? 'You, for this board' : 'You',
    path: file.path,
    exists: file.exists,
  }))
}

/**
 * The source the daemon takes for what was typed. `pack_add` takes `git:` and `dev:` sources, so a folder's path is
 * `dev:<path>` and a repository's address is `git:<address>`; a source that already has its scheme is left as it is.
 */
export function sourceOf(typed: string): string {
  const source = typed.trim()
  if (/^(git|dev):/.test(source)) return source
  if (/^(\/|~|\.\.?\/)/.test(source)) return `dev:${source}`
  if (/^([a-z][a-z0-9+.-]*:\/\/|[\w.-]+@[\w.-]+:)/i.test(source)) return `git:${source}`
  return source
}
