/** One feature of a proposal: `id` is `F<n>`, and `section` is empty when the entry is unsorted. */
export interface Feature {
  id: string
  title: string
  section: string
  summary: string
}

/** A line of the Features part that did not parse. `line` counts from 1 in the item's file. */
export interface FeatureProblem {
  line: number
  reason: string
}

export interface ParsedFeatures {
  features: Feature[]
  problems: FeatureProblem[]
}

/**
 * Where the parts of a Features part are, as indexes into the lines of the body (`body.split('\n')`), so that an op can
 * edit the text. Only what parsed is listed: a heading or an entry that was refused is not here.
 */
export interface FeaturePart {
  /** The index of the `## Features` line. */
  start: number
  /** The index of the line after the part: the next `## ` line, or the number of lines. */
  end: number
  /** Each `### <section>` heading: its line, and its name as written, trimmed. */
  headings: Array<{ line: number; name: string }>
  /** Each entry: its id, its first line and its last, which is its last continuation line. */
  entries: Array<{ id: string; first: number; last: number }>
}

/** A section name is 1 to 40 characters once trimmed (0.10, "A proposal's features"). */
export const SECTION_MAX = 40

const PART_HEADING = /^## Features[ \t]*$/
const SECTION_HEADING = /^###(?:[ \t]+(.*))?$/
const ENTRY = /^- \*\*F(0|[1-9][0-9]*) (.+?)\*\*: (.*)$/
const ENTRY_FORM = '- **F<n> <title>**: <summary>'

/** A fenced code block that is open: its fence character and how many of them opened it. */
interface Fence {
  char: string
  length: number
}

const openingFence = (line: string): Fence | null => {
  const match = /^(`{3,}|~{3,})/.exec(line.trimStart())
  return match === null ? null : { char: match[1]![0]!, length: match[1]!.length }
}

const closesFence = (line: string, fence: Fence): boolean => {
  const trimmed = line.trim()
  return trimmed.length >= fence.length && [...trimmed].every((char) => char === fence.char)
}

/**
 * Reads a proposal's `## Features` part from its body (0.10, "A proposal's features"). The part runs from the line
 * `## Features` to the next line that starts with `## `; a `## Features` line inside a fenced block elsewhere in the
 * body is not the part. In the part are blank lines, `### <section>` headings and entries,
 * `- **F<n> <title>**: <summary>`, whose continuation lines are indented by at least two spaces. Anything else is a
 * problem, and every problem is reported, in line order. `firstLine` is the line of the file that the body's first
 * line is, so a problem's `line` is a line of the file.
 *
 * A refused entry is not a feature, and the continuation lines of an entry that was refused are not reported again.
 * A second `## Features` part is a problem on its heading, and its content is not read. A fenced block inside the part
 * is one problem on its opening line, and the lines inside it are skipped.
 */
export function parseFeatures(body: string, firstLine = 1): ParsedFeatures {
  const { features, problems } = readFeaturePart(body, firstLine)
  return { features, problems }
}

/** What `parseFeatures` reads, and where the first part is: null when the body has no `## Features` part. */
export function readFeaturePart(body: string, firstLine = 1): ParsedFeatures & { part: FeaturePart | null } {
  const features: Feature[] = []
  const problems: FeatureProblem[] = []
  let part: FeaturePart | null = null
  let range: { id: string; first: number; last: number } | null = null
  const lines = body.split('\n').map((line) => (line.endsWith('\r') ? line.slice(0, -1) : line))
  const firstOf = new Map<string, number>()

  // `outside` is any text before or after the part, `in` the part, and `skip` the content of a second part.
  let state: 'outside' | 'in' | 'skip' = 'outside'
  let seenPart = false
  let fence: Fence | null = null
  let section = ''
  let current: Feature | null = null
  let absorbing = false

  lines.forEach((line, index) => {
    const number = firstLine + index
    const problem = (reason: string): void => void problems.push({ line: number, reason })
    const closeEntry = (absorb: boolean): void => {
      current = null
      range = null
      absorbing = absorb
    }

    if (state !== 'outside' && line.startsWith('## ')) {
      if (state === 'in' && part !== null) part.end = index
      state = 'outside'
      fence = null
    }
    if (state === 'outside') {
      if (fence !== null) {
        if (closesFence(line, fence)) fence = null
        return
      }
      fence = openingFence(line)
      if (fence !== null || !PART_HEADING.test(line)) return
      if (seenPart) {
        problem('a second ## Features part: a proposal has one, so move its entries into the first and delete this heading')
        state = 'skip'
        return
      }
      seenPart = true
      state = 'in'
      section = ''
      part = { start: index, end: lines.length, headings: [], entries: [] }
      closeEntry(false)
      return
    }
    if (state === 'skip') return

    if (fence !== null) {
      if (closesFence(line, fence)) fence = null
      return
    }
    if (line.trim() === '') return
    const opened = openingFence(line)
    if (opened !== null) {
      fence = opened
      closeEntry(false)
      return problem('a fenced code block is not allowed in the Features part; its lines are not read')
    }
    if (line.startsWith('####')) {
      closeEntry(false)
      return problem('only ### section headings are allowed here, not ####')
    }
    const heading = SECTION_HEADING.exec(line)
    if (heading !== null) {
      closeEntry(false)
      const name = (heading[1] ?? '').trim()
      const length = [...name].length
      if (name === '') return problem('a section heading is empty; name the section, such as ### Packs')
      if (length > SECTION_MAX) return problem(`a section name is at most ${SECTION_MAX} characters; this one has ${length}`)
      if (name.includes('#')) return problem('a section name has no #')
      section = /^unsorted$/i.test(name) ? '' : name
      part?.headings.push({ line: index, name })
      return
    }
    const entry = ENTRY.exec(line)
    if (entry !== null) {
      const [, digits, rawTitle, rawSummary] = entry as unknown as [string, string, string, string]
      const id = `F${digits}`
      const title = rawTitle.trim()
      const summary = rawSummary.trim()
      closeEntry(true)
      if (title === '' || summary === '') return problem(`an entry needs a title and a summary: ${ENTRY_FORM}`)
      if (digits === '0') return problem('F0 is not a feature id: numbers count from 1')
      const first = firstOf.get(id)
      if (first !== undefined) return problem(`${id} is already used on line ${first}; each F<n> appears once in a proposal`)
      firstOf.set(id, number)
      current = { id, title, section, summary }
      features.push(current)
      range = { id, first: index, last: index }
      part?.entries.push(range)
      absorbing = false
      return
    }
    if (line.startsWith('- ')) {
      closeEntry(true)
      return problem(`an entry is written ${ENTRY_FORM}`)
    }
    if (line.startsWith('  ')) {
      if (current !== null) {
        (current as Feature).summary += ` ${line.trim()}`
        if (range !== null) range.last = index
      } else if (!absorbing) problem('a continuation line has no entry before it')
      return
    }
    if (line.startsWith(' ')) return problem('a continuation line is indented by at least two spaces, not one')
    problem(`this line is neither an entry nor a continuation: an entry is written ${ENTRY_FORM}`)
  })

  return { features, problems, part }
}
