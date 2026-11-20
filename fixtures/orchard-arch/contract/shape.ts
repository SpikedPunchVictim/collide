import { STATUSES } from '../../packs/orchard/providers/plan/declaration.ts'
import {
  ANY_ID_PATTERN,
  ANY_SHORT_ID_PATTERN,
  EPIC_ARG,
  ID_PATTERN,
  PROPOSAL_ARG,
  SHORT_ID_PATTERN,
} from '../../packs/orchard/providers/plan/ids.ts'

// The shape of one run of a plan CLI. A shape is kept in a public repository and is made from a real repository's
// output, so it keeps nothing of the run but the words that are the CLI's own: see `leafOf` and `keyOf`.
//
// A shape is a string (a leaf), an array of zero or one shape (a list) or an object of shapes (an object).
//   - A leaf is `null`, `boolean`, `number`, `string`, the name of an id pattern (`task-id`, `task-short-id`,
//     `proposal-id`, `epic-id`, `id`, `short-id`) or a kept constant, written `=word`. Leaves that meet when shapes
//     merge are joined with `|`, sorted. `mixed` is what a leaf and a list or an object become when they meet.
//   - A list holds its elements merged into one shape, or nothing when it is empty.
//   - An object holds its keys, sorted. A key that ends in `?` is absent from some objects merged into this one.
//     An object that has a data key (an id, a path, a date, a name) is a map: every key is named by what it is
//     (`<task-id>` and the like, or `<string>`), and the shapes of its values are merged.

export type Shape = string | Shape[] | { [key: string]: Shape }

/** The id patterns, most specific first. The first that a string fits is its name. */
const ID_NAMES: Array<[string, RegExp]> = [
  ['task-id', new RegExp(ID_PATTERN)],
  ['task-short-id', new RegExp(SHORT_ID_PATTERN)],
  ['proposal-id', new RegExp(PROPOSAL_ARG)],
  ['epic-id', new RegExp(EPIC_ARG)],
  ['id', new RegExp(ANY_ID_PATTERN)],
  ['short-id', new RegExp(ANY_SHORT_ID_PATTERN)],
]

const STATUS_NAMES = new Set<string>(STATUSES.map((status) => status.name))

/** A key of the CLI's own schema: camelCase or snake_case words. Anything else is data (a slug, a date, a path). */
const SCHEMA_KEY = /^[a-z][a-zA-Z0-9]*(?:_[a-zA-Z0-9]+)*$/
const SCHEMA_KEY_MAX = 40

/** A `command` the CLI names itself: lowercase words with hyphens. */
const COMMAND_WORD = /^[a-z]+(?:-[a-z]+)*$/
const COMMAND_MAX = 24

const idName = (text: string): string | null => ID_NAMES.find(([, pattern]) => pattern.test(text))?.[0] ?? null

/** What a string leaf is kept as: a pack status as itself, an id as its pattern's name, any other text as `string`. */
function leafOf(text: string): string {
  if (STATUS_NAMES.has(text)) return `=${text}`
  return idName(text) ?? 'string'
}

const isSchemaKey = (key: string): boolean => key.length <= SCHEMA_KEY_MAX && SCHEMA_KEY.test(key)

/** The name a key is kept under. `mapped` is true when the object it is in has a data key. */
function keyOf(key: string, mapped: boolean): string {
  if (!mapped) return key
  if (STATUS_NAMES.has(key)) return key
  const name = idName(key)
  return name === null ? '<string>' : `<${name}>`
}

const alternatives = (leaf: string): string[] => leaf.split('|')

const kindOf = (shape: Shape): 'leaf' | 'list' | 'object' =>
  typeof shape === 'string' ? 'leaf' : Array.isArray(shape) ? 'list' : 'object'

/** A key without its `?`. */
const bare = (key: string): string => (key.endsWith('?') ? key.slice(0, -1) : key)

/** Merges two shapes into one that holds both. */
export function mergeShapes(a: Shape, b: Shape): Shape {
  const kinds = [kindOf(a), kindOf(b)]
  if (kinds[0] !== kinds[1]) return 'mixed'
  if (typeof a === 'string' && typeof b === 'string') {
    if (a === 'mixed' || b === 'mixed') return 'mixed'
    return [...new Set([...alternatives(a), ...alternatives(b)])].sort().join('|')
  }
  if (Array.isArray(a) && Array.isArray(b)) {
    const [first] = a
    const [second] = b
    if (first === undefined) return b
    if (second === undefined) return a
    return [mergeShapes(first, second)]
  }
  const left = a as { [key: string]: Shape }
  const right = b as { [key: string]: Shape }
  const names = new Set([...Object.keys(left), ...Object.keys(right)].map(bare))
  const merged: { [key: string]: Shape } = {}
  for (const name of [...names].sort()) {
    const inLeft = name in left ? left[name] : `${name}?` in left ? left[`${name}?`] : undefined
    const inRight = name in right ? right[name] : `${name}?` in right ? right[`${name}?`] : undefined
    const optional = inLeft === undefined || inRight === undefined || `${name}?` in left || `${name}?` in right
    const shape = inLeft === undefined ? inRight : inRight === undefined ? inLeft : mergeShapes(inLeft, inRight)
    merged[optional ? `${name}?` : name] = shape as Shape
  }
  return merged
}

/** The shape of any JSON value. */
export function shapeOf(value: unknown): Shape {
  if (value === null) return 'null'
  if (typeof value === 'boolean') return 'boolean'
  if (typeof value === 'number') return 'number'
  if (typeof value === 'string') return leafOf(value)
  if (Array.isArray(value)) {
    const shapes = value.map(shapeOf)
    return shapes.length === 0 ? [] : [shapes.reduce(mergeShapes)]
  }
  if (typeof value !== 'object') return 'string'
  const entries = Object.entries(value as Record<string, unknown>).sort(([a], [b]) => (a < b ? -1 : 1))
  const mapped = entries.some(([key]) => !isSchemaKey(key))
  const shape: { [key: string]: Shape } = {}
  for (const [key, inner] of entries) {
    const name = keyOf(key, mapped)
    const innerShape = shapeOf(inner)
    shape[name] = name in shape ? mergeShapes(shape[name] as Shape, innerShape) : innerShape
  }
  return shape
}

/**
 * The shape of a JSON envelope. At its top level, `command` keeps the CLI's own word and `schema_version` its number;
 * every other value is shaped as in `shapeOf`.
 */
export function envelopeShape(envelope: unknown): Shape {
  if (typeof envelope !== 'object' || envelope === null || Array.isArray(envelope)) return shapeOf(envelope)
  const shape = shapeOf(envelope) as { [key: string]: Shape }
  const { command, schema_version: version } = envelope as Record<string, unknown>
  if (typeof command === 'string' && command.length <= COMMAND_MAX && COMMAND_WORD.test(command)) {
    shape.command = `=${command}`
  }
  if (typeof version === 'number') shape.schema_version = `=${version}`
  return shape
}

/** What a run of the CLI ended with. */
export interface Ended {
  code: number | null
  stdout: string
  stderr: string
}

/** The shape of a run: how it ended, which streams it used, and the envelope on standard output when it is JSON. */
export interface RunShape {
  exit: number | null
  stdout_empty: boolean
  stdout_json: boolean
  stderr_empty: boolean
  stderr_lines: number
  envelope: Shape | null
}

const lineCount = (text: string): number => (text === '' ? 0 : text.replace(/\n+$/, '').split('\n').length)

export function runShape(ended: Ended): RunShape {
  let envelope: Shape | null = null
  if (ended.stdout !== '') {
    try {
      envelope = envelopeShape(JSON.parse(ended.stdout))
    } catch {
      envelope = null
    }
  }
  return {
    exit: ended.code,
    stdout_empty: ended.stdout === '',
    stdout_json: envelope !== null,
    stderr_empty: ended.stderr === '',
    stderr_lines: lineCount(ended.stderr),
    envelope,
  }
}

/** A shape in a message: a leaf as itself, any other as its kind. */
const named = (shape: Shape): string => (typeof shape === 'string' ? shape : kindOf(shape))

/**
 * How `actual` differs from the `recorded` shape, one message per difference, each naming the path in the envelope.
 * An empty list or an empty object on either side matches any list or object: nothing was seen to compare. A leaf
 * matches when each of its alternatives is one that was recorded.
 */
export function diffShapes(recorded: Shape, actual: Shape, path = 'envelope'): string[] {
  if (recorded === 'mixed') return []
  const kinds = [kindOf(recorded), kindOf(actual)]
  if (kinds[0] !== kinds[1]) return [`${path}: recorded ${named(recorded)}, got ${named(actual)}`]
  if (typeof recorded === 'string' && typeof actual === 'string') {
    const known = new Set(alternatives(recorded))
    const extra = alternatives(actual).filter((leaf) => !known.has(leaf))
    return extra.length === 0 ? [] : [`${path}: recorded ${recorded}, got ${actual}`]
  }
  if (Array.isArray(recorded) && Array.isArray(actual)) {
    const [first] = recorded
    const [second] = actual
    return first === undefined || second === undefined ? [] : diffShapes(first, second, `${path}[]`)
  }
  const wanted = recorded as { [key: string]: Shape }
  const got = actual as { [key: string]: Shape }
  if (Object.keys(wanted).length === 0 || Object.keys(got).length === 0) return []
  const problems: string[] = []
  for (const key of Object.keys(wanted)) {
    const name = bare(key)
    const found = name in got ? name : `${name}?` in got ? `${name}?` : undefined
    if (found === undefined) {
      if (!key.endsWith('?')) problems.push(`${path}.${name}: recorded, missing`)
      continue
    }
    if (!key.endsWith('?') && found.endsWith('?')) problems.push(`${path}.${name}: recorded in all, missing in some`)
    problems.push(...diffShapes(wanted[key] as Shape, got[found] as Shape, `${path}.${name}`))
  }
  for (const key of Object.keys(got)) {
    if (!(bare(key) in wanted) && !(`${bare(key)}?` in wanted)) problems.push(`${path}.${bare(key)}: not recorded`)
  }
  return problems
}

/** How the shape of a run differs from the recorded one, one message per difference. */
export function diffRuns(recorded: RunShape, actual: RunShape): string[] {
  const problems: string[] = []
  for (const key of ['exit', 'stdout_empty', 'stdout_json', 'stderr_empty', 'stderr_lines'] as const) {
    if (recorded[key] !== actual[key]) problems.push(`${key}: recorded ${recorded[key]}, got ${actual[key]}`)
  }
  if (recorded.envelope !== null && actual.envelope !== null) {
    problems.push(...diffShapes(recorded.envelope, actual.envelope))
  }
  return problems
}
