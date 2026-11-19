import { parseDocument } from 'yaml'
import { closesFence, openingFence } from './features.ts'
import type { Fence } from './features.ts'
import type { PartProblem } from './parts.ts'
import { MAX_ALIAS_COUNT, YAML_OPTIONS } from './read-file.ts'

/** One claim of a spike's record, as the planning view's tables and the verify gate read it. No prose is carried. */
export interface Claim {
  id: string
  label: string
  loadBearing: boolean
  verifiable?: boolean
}

/** The part a claim block's problems are listed under. */
export const CLAIM_PART = 'claims'

/** A claim block's lines may be this many bytes. Checked before parsing, as the frontmatter's cap is. */
export const CLAIM_CAP = 8 * 1024

const LABELS = ['measured', 'inferred', 'unmeasured']
const ID = /^Q[1-9][0-9]*\.c[1-9][0-9]*$/
const ID_FORM = 'Q<n>.c<n>'
const TEXT_KEYS = ['unit', 'tolerance', 'repro', 'output']
const KEYS = new Set(['id', 'label', 'says', 'value', ...TEXT_KEYS, 'cost_usd', 'verifiable', 'load_bearing'])
const QUOTE_HINT = 'quote a value that looks like a number'

const isMapping = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const describe = (value: unknown): string => {
  if (value === null) return 'empty'
  if (Array.isArray(value)) return 'a list'
  if (typeof value === 'object') return 'a mapping'
  return `a ${typeof value}`
}

/** The block's YAML as a mapping, or why it is not one. */
function parseBlock(text: string): { ok: true; value: Record<string, unknown> } | { ok: false; reason: string } {
  try {
    const doc = parseDocument(text, YAML_OPTIONS)
    const problem = doc.errors[0] ?? doc.warnings[0]
    if (problem) return { ok: false, reason: `a claim block is not valid YAML: ${problem.message.split('\n')[0]}` }
    const value: unknown = doc.toJS({ maxAliasCount: MAX_ALIAS_COUNT })
    if (!isMapping(value)) {
      return { ok: false, reason: `a claim block must be a mapping of keys to values, not ${describe(value)}` }
    }
    return { ok: true, value }
  } catch (error) {
    const reason = error instanceof Error ? error.message : String(error)
    return { ok: false, reason: `a claim block is not valid YAML: ${reason}` }
  }
}

/** What is wrong with the keys and values of a block that parsed, in the order of the checks. */
function faults(block: Record<string, unknown>): string[] {
  const found: string[] = []
  const { id, label, says } = block
  for (const key of Object.keys(block).filter((name) => !KEYS.has(name))) {
    found.push(`a claim has no key "${key}"; its keys are ${[...KEYS].join(', ')}`)
  }
  if (id === undefined) found.push(`a claim needs an id, written ${ID_FORM}`)
  else if (typeof id !== 'string' || !ID.test(id)) {
    const got = typeof id === 'string' ? JSON.stringify(id) : `${describe(id)}; ${QUOTE_HINT}`
    found.push(`the id must be written ${ID_FORM}, such as Q1.c1, not ${got}`)
  }
  if (label === undefined) found.push(`a claim needs a label: ${LABELS.join(', ')}`)
  else if (typeof label !== 'string' || !LABELS.includes(label)) {
    const got = typeof label === 'string' ? JSON.stringify(label) : describe(label)
    found.push(`the label ${got} is none of ${LABELS.join(', ')}`)
  }
  if (says === undefined) found.push('a claim needs says: what it claims, as text')
  else if (typeof says !== 'string') found.push(`says must be text, not ${describe(says)}; ${QUOTE_HINT}`)
  else if (says.trim() === '') found.push('says is empty: write what the claim says')

  const { value, cost_usd: cost, verifiable, load_bearing: loadBearing } = block
  if (value !== undefined && typeof value !== 'string' && !(typeof value === 'number' && Number.isFinite(value))) {
    found.push(`value must be text or a number, not ${describe(value)}`)
  }
  for (const key of TEXT_KEYS) {
    if (block[key] !== undefined && typeof block[key] !== 'string') {
      found.push(`${key} must be text, not ${describe(block[key])}; ${QUOTE_HINT}`)
    }
  }
  if (cost !== undefined && !(typeof cost === 'number' && Number.isFinite(cost) && cost >= 0)) {
    found.push('cost_usd must be a number of dollars, 0 or more')
  }
  if (verifiable !== undefined && verifiable !== 'yes' && verifiable !== 'no') {
    const got = typeof verifiable === 'string' ? JSON.stringify(verifiable) : describe(verifiable)
    found.push(`verifiable must be yes or no, not ${got}`)
  }
  if (loadBearing !== undefined && typeof loadBearing !== 'boolean') {
    found.push(`load_bearing must be true or false, not ${describe(loadBearing)}`)
  }
  return found
}

/** A claim block that is being read: where it opened, how it is fenced, and its lines so far. */
interface Open {
  fence: Fence
  claim: boolean
  line: number
  indent: number
  rows: string[]
}

/** Takes up to `indent` spaces off the start of `row`, as a fenced block in an indented item has them taken. */
function unindent(row: string, indent: number): string {
  let taken = 0
  while (taken < indent && row[taken] === ' ') taken += 1
  return row.slice(taken)
}

/**
 * Reads the claims of a spike's record from its body (0.10, "A `spike`"; `docs/research/planning.md` 4.3). A claim is
 * a fenced block whose info string is exactly `claim`, and its lines are YAML, read as the frontmatter is (S5). Fences
 * are matched by length, so a block shown inside a longer fence is text; a fence with an info string of another word is
 * skipped whole. A block with no closing fence is a problem and is not read.
 *
 * Only a block with no problem is a claim. Each thing wrong with a block is one problem, with the line of the block's
 * opening fence: a YAML error, a block that is not a mapping, one over `CLAIM_CAP`, an unknown key, a missing or bad
 * `id`, `label` or `says`, a value of the wrong type, and an `id` that an earlier block used (even one that was
 * refused). `firstLine` is the line of the file that the body's first line is.
 */
export function readClaims(body: string, firstLine = 1): { claims: Claim[]; problems: PartProblem[] } {
  const claims: Claim[] = []
  const problems: PartProblem[] = []
  const firstOf = new Map<string, number>()

  const finish = (block: Open): void => {
    const problem = (reason: string): void => void problems.push({ part: CLAIM_PART, line: block.line, reason })
    const text = block.rows.join('\n')
    const size = Buffer.byteLength(text)
    if (size > CLAIM_CAP) return problem(`a claim block is ${size} bytes, over the ${CLAIM_CAP}-byte cap`)
    const parsed = parseBlock(text)
    if (!parsed.ok) return problem(parsed.reason)
    const found = faults(parsed.value)
    const { id, label, verifiable, load_bearing: loadBearing } = parsed.value
    if (typeof id === 'string' && ID.test(id)) {
      const first = firstOf.get(id)
      if (first === undefined) firstOf.set(id, block.line)
      else found.push(`${id} is already used by the block on line ${first}; each id appears once`)
    }
    found.forEach(problem)
    if (found.length > 0) return
    claims.push({
      id: id as string,
      label: label as string,
      loadBearing: loadBearing === true,
      ...(verifiable === undefined ? {} : { verifiable: verifiable === 'yes' }),
    })
  }

  let open: Open | null = null
  body.split('\n').forEach((raw, index) => {
    const line = raw.endsWith('\r') ? raw.slice(0, -1) : raw
    if (open !== null) {
      if (!closesFence(line, open.fence)) {
        if (open.claim) open.rows.push(unindent(line, open.indent))
        return
      }
      const closed: Open = open
      open = null
      if (closed.claim) finish(closed)
      return
    }
    const fence = openingFence(line)
    if (fence === null) return
    const info = line.trimStart().slice(fence.length).trim()
    const indent = line.length - line.trimStart().length
    open = { fence, claim: info === 'claim', line: firstLine + index, indent, rows: [] }
  })

  const unclosed = open as Open | null
  if (unclosed !== null && unclosed.claim) {
    const reason = 'a claim block has no closing fence, so it is not read'
    problems.push({ part: CLAIM_PART, line: unclosed.line, reason })
  }
  return { claims, problems }
}
