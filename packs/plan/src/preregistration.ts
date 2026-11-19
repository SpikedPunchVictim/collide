import { createHash } from 'node:crypto'
import { closesFence, openingFence } from './features.ts'
import type { Fence } from './features.ts'
import type { PartProblem } from './parts.ts'

/** A spike's Pre-registration part (0.10, "A `spike`"): the text of each label that is there, and what is below it. */
export interface Preregistration {
  hypothesis?: string
  cheapestTest?: string
  killCriterion?: string
  budget?: string
  /** How many `### Amendment` sections are below the original text. */
  amendments: number
  /** Whether the frontmatter's `preregistered` is the hash of the original text. Left out when there is none. */
  hashMatches?: boolean
}

/** The part's name, as a problem lists it. */
export const PREREGISTRATION_PART = 'Pre-registration'

const NAME = /^## +(.*?) *$/
const AMENDMENT = /^###[ \t]+Amendment(?:[ \t].*)?$/i
const LABEL = /^(Hypothesis|Cheapest test|Kill criterion|Budget):[ \t]*(.*)$/i

type Key = 'hypothesis' | 'cheapestTest' | 'killCriterion' | 'budget'

/** The four labels, as the part spells them, and the key of each in `Preregistration`. */
const LABELS: ReadonlyArray<{ label: string; key: Key }> = [
  { label: 'Hypothesis', key: 'hypothesis' },
  { label: 'Cheapest test', key: 'cheapestTest' },
  { label: 'Kill criterion', key: 'killCriterion' },
  { label: 'Budget', key: 'budget' },
]

const keyOf = (written: string): Key =>
  (LABELS.find((one) => one.label.toLowerCase() === written.toLowerCase()) as { key: Key }).key

/** The first 16 lowercase hex characters of the SHA-256 of `text`: the hash that `preregistered` holds. */
const hashOf = (text: string): string => createHash('sha256').update(text).digest('hex').slice(0, 16)

/** Where the part is: its heading's index in `lines`, the index its original text ends at, and its amendments. */
interface Range {
  start: number
  original: number
  amendments: number
}

/**
 * Finds the first `## Pre-registration` part, the end of its original text (the first amendment, or the part's end)
 * and the number of amendments. A second part is a problem on its heading. Fenced blocks are skipped, so a heading in
 * one is not a heading.
 */
function rangeOf(lines: string[], problem: (index: number, reason: string) => void): Range | null {
  let range: Range | null = null
  let end = lines.length
  let firstAmendment: number | null = null
  let inPart = false
  let fence: Fence | null = null

  lines.forEach((line, index) => {
    if (fence !== null) {
      if (closesFence(line, fence)) fence = null
      return
    }
    fence = openingFence(line)
    if (fence !== null) return
    if (line.startsWith('## ')) {
      if (inPart) {
        end = index
        inPart = false
      }
      if (NAME.exec(line)?.[1]?.toLowerCase() !== PREREGISTRATION_PART.toLowerCase()) return
      if (range !== null) return problem(index, 'a second Pre-registration part: a record has one; this is not read')
      range = { start: index, original: end, amendments: 0 }
      inPart = true
      return
    }
    if (inPart && range !== null && AMENDMENT.test(line)) {
      range.amendments += 1
      firstAmendment ??= index
    }
  })
  const found = range as Range | null
  if (found === null) return null
  found.original = Math.min(firstAmendment ?? end, end)
  return found
}

/** The text of each label in the original text, and the line of each, the first of a label winning. */
function labelsOf(lines: string[], range: Range): Map<Key, { text: string; index: number }> {
  const found = new Map<Key, { text: string; index: number }>()
  let current: { text: string; index: number } | null = null
  let fence: Fence | null = null

  for (let index = range.start + 1; index < range.original; index += 1) {
    const line = lines[index] as string
    if (fence !== null) {
      if (closesFence(line, fence)) fence = null
      continue
    }
    fence = openingFence(line)
    if (fence !== null || line.trim() === '' || line.startsWith('#')) {
      current = null
      continue
    }
    const written = LABEL.exec(line)
    if (written !== null) {
      const key = keyOf(written[1] as string)
      current = found.has(key) ? null : { text: (written[2] as string).trim(), index }
      if (current !== null) found.set(key, current)
    } else if (current !== null) {
      current.text = current.text === '' ? line.trim() : `${current.text} ${line.trim()}`
    }
  }
  return found
}

/**
 * Reads a spike's Pre-registration part from its body (0.10, "A `spike`"): a `## Pre-registration` heading, matched in
 * any case, to the next line that starts with `## `. The original text is the heading and what follows up to the first
 * `### Amendment` section; the sections below are amendments and are only counted. In the original, a line that starts
 * with `Hypothesis:`, `Cheapest test:`, `Kill criterion:` or `Budget:` (in any case) is a label, and its text is the
 * rest of the line and the lines that follow, up to a blank line, a heading, a fenced block or the next label. Any
 * other word and a colon is prose.
 *
 * A missing label is a problem on the heading, and an empty `Kill criterion:` is one on its line. `preregistered` is
 * the frontmatter's value, as written: `hashMatches` is whether it is the hash of the original text, which is its lines
 * (the heading to the last that is not blank, with no line end after the last) joined by LF, and so does not change
 * with the line ends of the file or when an amendment is added below. A record with no such part has none.
 * `firstLine` is the line of the file that the body's first line is.
 */
export function readPreregistration(
  body: string,
  firstLine: number,
  preregistered: unknown,
): { preregistration: Preregistration | null; problems: PartProblem[] } {
  const problems: PartProblem[] = []
  const problem = (index: number, reason: string): void =>
    void problems.push({ part: PREREGISTRATION_PART, line: firstLine + index, reason })
  const lines = body.split('\n').map((line) => (line.endsWith('\r') ? line.slice(0, -1) : line))
  const range = rangeOf(lines, problem)
  if (range === null) return { preregistration: null, problems }

  const found = labelsOf(lines, range)
  const preregistration: Preregistration = { amendments: range.amendments }
  for (const { label, key } of LABELS) {
    const one = found.get(key)
    if (one === undefined) problem(range.start, `the part has no "${label}:" line`)
    else preregistration[key as Key] = one.text
  }
  const kill = found.get('killCriterion')
  if (kill !== undefined && kill.text === '') {
    problem(kill.index, 'the Kill criterion is empty: write what would end the idea')
  }

  if (preregistered !== undefined) {
    const original = lines.slice(range.start, range.original)
    while (original.length > 0 && (original.at(-1) as string).trim() === '') original.pop()
    preregistration.hashMatches = preregistered === hashOf(original.join('\n'))
  }
  return { preregistration, problems: problems.sort((a, b) => a.line - b.line) }
}
