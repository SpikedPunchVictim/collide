import { readFeaturePart } from './features.ts'
import type { Feature, FeaturePart, FeatureProblem } from './features.ts'
import { describeFeatures } from './links.ts'

/**
 * Edits of a proposal's Features part, as text (0.10, "A proposal's features"). Each works on a body and returns the
 * new body, or says why it will not: a part that has problems is refused with them, and the new part is read again
 * and refused if it has one, or if its features are not the ones the edit means. A line the edit does not touch is
 * returned as it was, with its own line ending; a line it writes ends with `eol`.
 */

/** The name of the section that holds the features with no section. */
export const UNSORTED = 'Unsorted'

/** The key two names share when they are one section, as in `core/src/section.rs`: trimmed and lowercased. */
export const sectionKey = (name: string): string => name.trim().toLowerCase()

export const isUnsorted = (name: string): boolean => sectionKey(name) === 'unsorted'

/** Where the body is, for a message: the item's id and path, and the line of the file that the body starts on. */
export interface Where {
  id: string
  path: string
  firstLine: number
}

export type TextResult = { ok: true; body: string; changed: boolean } | { ok: false; message: string }

const fail = (message: string): TextResult => ({ ok: false, message })

/** The lines of a body, each with its own line ending. Index `n` is line `n` of `readFeaturePart`. */
const linesOf = (body: string): string[] => body.match(/[^\n]*\n|[^\n]+$/g) ?? []

const endingOf = (line: string): string => (line.endsWith('\r\n') ? '\r\n' : line.endsWith('\n') ? '\n' : '')

const withEnding = (line: string, eol: string): string => (line.endsWith('\n') ? line : `${line}${eol}`)

/**
 * The index of the last line from `from` up to `end` (exclusive) that is not blank, and `from` when all are. `end` may
 * be one past the last line: a body that ends with a line end is read by `split` as having an empty line after it.
 */
function lastContent(lines: string[], from: number, end: number): number {
  for (let at = end - 1; at > from; at--) if ((lines[at] ?? '').trim() !== '') return at
  return from
}

/** Lines to remove and lines to put after an index: the whole of an edit. */
interface Splice {
  remove: Set<number>
  after: Map<number, string[]>
}

function insertAfter(edit: Splice, index: number, lines: string[]): void {
  edit.after.set(index, [...(edit.after.get(index) ?? []), ...lines])
}

/** Applies the edit. A line that something is put after, and has no line end (the last of a file), gets one. */
function applySplice(lines: string[], edit: Splice, eol: string): string {
  const out: string[] = []
  lines.forEach((line, index) => {
    if (!edit.remove.has(index)) out.push(line)
    const put = edit.after.get(index)
    if (put === undefined) return
    const last = out.length - 1
    if (last >= 0) out[last] = withEnding(out[last] as string, eol)
    out.push(...put.map((text) => withEnding(text, eol)))
  })
  return out.join('')
}

const clip = (text: string): string => (text.length > 80 ? `${text.slice(0, 80)}...` : text)

/** Each problem, with its line and the text of that line in `body`. */
function listProblems(problems: FeatureProblem[], body: string, firstLine: number): string {
  const lines = body.split('\n').map((line) => line.replace(/\r$/, ''))
  return problems
    .map((problem) => {
      const text = lines[problem.line - firstLine] ?? ''
      return `  line ${problem.line}: ${problem.reason} (the line reads: ${JSON.stringify(clip(text))})`
    })
    .join('\n')
}

/** What the part of a body holds, or the refusal for a part that is missing or has problems. */
function readPart(where: Where, body: string): { ok: true; part: FeaturePart; features: Feature[] } | TextResult {
  const read = readFeaturePart(body, where.firstLine)
  if (read.problems.length > 0) {
    const count = `${read.problems.length} problem${read.problems.length === 1 ? '' : 's'}`
    const head = `${where.id} cannot change its features: its ## Features part in ${where.path} has ${count}`
    const fix = read.problems.length === 1 ? 'it' : 'them'
    return fail(`${head}; fix ${fix} first:\n${listProblems(read.problems, body, where.firstLine)}`)
  }
  if (read.part === null) {
    return fail(`${where.id} has no ## Features part in ${where.path}: add the line "## Features" first`)
  }
  return { ok: true, part: read.part, features: read.features }
}

/** Reads the new body again: no problem, and the features `expected`, in any order. */
function checkAfter(where: Where, body: string, expected: Feature[]): TextResult {
  const read = readFeaturePart(body, where.firstLine)
  if (read.problems.length > 0) {
    const head = `${where.id}: the change would leave its ## Features part with a problem, so nothing was written`
    return fail(`${head}:\n${listProblems(read.problems, body, where.firstLine)}`)
  }
  const key = (feature: Feature): string =>
    JSON.stringify([feature.id, feature.title, feature.section, feature.summary])
  const got = read.features.map(key).sort()
  const want = expected.map(key).sort()
  const same = got.length === want.length && got.every((entry, at) => entry === want[at])
  if (same) return { ok: true, body, changed: true }
  return fail(`${where.id}: the change would not leave its features as asked, so nothing was written`)
}

/** The heading of `part` that holds the section `name`, or undefined. `Unsorted` is any heading that spells it. */
const headingOf = (part: FeaturePart, name: string): { line: number; name: string } | undefined =>
  part.headings.find((heading) => sectionKey(heading.name) === sectionKey(name))

const endOfSection = (part: FeaturePart, line: number): number =>
  part.headings.find((heading) => heading.line > line)?.line ?? part.end

/** The name a feature has under the heading `name`: empty for Unsorted. */
const sectionOf = (name: string): string => (isUnsorted(name) ? '' : name)

/** Whether the part has a heading for `name`, whatever else is wrong with it. */
export const hasSection = (body: string, firstLine: number, name: string): boolean => {
  const { part } = readFeaturePart(body, firstLine)
  return part !== null && headingOf(part, name) !== undefined
}

/** The section to write under: the name trimmed, `Unsorted` when it is empty. */
const wanted = (section: string): string => (section.trim() === '' ? UNSORTED : section.trim())

/**
 * Puts `entry` at the end of the section `name`: after the last line of its heading, or, when the part has no such
 * heading, under a new `### <name>` at the end of the part. Returns the section name that the entry has.
 */
function place(lines: string[], part: FeaturePart, name: string, entry: string[], edit: Splice, eol: string): string {
  const heading = headingOf(part, name)
  if (heading !== undefined) {
    insertAfter(edit, lastContent(lines, heading.line, endOfSection(part, heading.line)), entry)
    return sectionOf(heading.name)
  }
  const last = lastContent(lines, part.start, part.end)
  insertAfter(edit, last, [eol, `### ${name}${eol}`, ...entry])
  return sectionOf(name)
}

/** The lines of one entry: the first holds the title and the first line of the summary, the rest continue it. */
export const entryLines = (n: number, title: string, summaryLines: string[]): string[] => [
  `- **F${n} ${title}**: ${summaryLines[0] as string}`,
  ...summaryLines.slice(1).map((line) => `  ${line}`),
]

/**
 * Adds a feature at the end of `section` (matched after trimming and in any case; empty is Unsorted), as one past the
 * highest number in the body.
 */
export function addEntry(
  where: Where,
  body: string,
  eol: string,
  section: string,
  title: string,
  summaryLines: string[],
): TextResult {
  const read = readPart(where, body)
  if (!('part' in read)) return read
  const { part, features } = read
  const lines = linesOf(body)
  const id = `F${Math.max(0, ...features.map((feature) => Number(feature.id.slice(1)))) + 1}`
  const entry = entryLines(Number(id.slice(1)), title, summaryLines).map((line) => `${line}${eol}`)
  const edit: Splice = { remove: new Set(), after: new Map() }
  const at = place(lines, part, wanted(section), entry, edit, eol)
  const added: Feature = { id, title, section: at, summary: summaryLines.join(' ') }
  return checkAfter(where, applySplice(lines, edit, eol), [...features, added])
}

/** Moves a feature, with its continuation lines, to the end of `section`. A feature already there is left alone. */
export function moveEntry(where: Where, body: string, eol: string, feature: string, section: string): TextResult {
  const read = readPart(where, body)
  if (!('part' in read)) return read
  const { part, features } = read
  const entry = part.entries.find((candidate) => candidate.id === feature)
  const moved = features.find((candidate) => candidate.id === feature)
  if (entry === undefined || moved === undefined) {
    return fail(`${where.id} has no feature ${feature}; ${describeFeatures(features)}`)
  }
  const name = wanted(section)
  if (sectionKey(sectionOf(moved.section)) === sectionKey(sectionOf(name))) return { ok: true, body, changed: false }
  const lines = linesOf(body)
  const edit: Splice = { remove: new Set(), after: new Map() }
  for (let at = entry.first; at <= entry.last; at++) edit.remove.add(at)
  const text = lines.slice(entry.first, entry.last + 1).map((line) => withEnding(line, eol))
  const into = place(lines, part, name, text, edit, eol)
  const expected = features.map((candidate) => (candidate === moved ? { ...candidate, section: into } : candidate))
  return checkAfter(where, applySplice(lines, edit, eol), expected)
}

/**
 * Renames the section `from` to `to`. Every heading that holds `from` (matched after trimming and in any case) is one
 * section: the first of them is renamed, or, when another heading already holds `to`, that heading keeps its name and
 * the entries of every `from` heading are put at its end, and the `from` headings, and the blank line before each,
 * are removed. A body with no heading for `from`, or that the rename leaves as it was, comes back unchanged.
 */
export function renameHeadings(where: Where, body: string, eol: string, from: string, to: string): TextResult {
  const read = readPart(where, body)
  if (!('part' in read)) return read
  const { part, features } = read
  const named = part.headings.filter((heading) => sectionKey(heading.name) === sectionKey(from))
  if (named.length === 0) return { ok: true, body, changed: false }
  const lines = linesOf(body)
  const existing = part.headings.find(
    (heading) => !named.includes(heading) && sectionKey(heading.name) === sectionKey(to),
  )
  const target = existing ?? (named[0] as { line: number; name: string })
  const edit: Splice = { remove: new Set(), after: new Map() }
  const anchor = lastContent(lines, target.line, endOfSection(part, target.line))
  const kept = lines.slice()
  if (existing === undefined && target.name !== to) {
    const old = lines[target.line] as string
    kept[target.line] = `### ${to}${endingOf(old)}`
  }
  for (const heading of named.filter((candidate) => candidate !== target)) {
    const end = endOfSection(part, heading.line)
    let first = heading.line
    while (first - 1 > part.start && (lines[first - 1] as string).trim() === '') first--
    for (let at = first; at <= lastContent(lines, heading.line, end); at++) edit.remove.add(at)
    for (const entry of part.entries.filter((candidate) => candidate.first > heading.line && candidate.first < end)) {
      insertAfter(edit, anchor, lines.slice(entry.first, entry.last + 1).map((line) => withEnding(line, eol)))
    }
  }
  const into = sectionOf(existing === undefined ? to : existing.name)
  const moved = (feature: Feature): boolean => sectionKey(feature.section) === sectionKey(from)
  const expected = features.map((feature) => (moved(feature) ? { ...feature, section: into } : feature))
  const result = applySplice(kept, edit, eol)
  if (result === body) return { ok: true, body, changed: false }
  return checkAfter(where, result, expected)
}
