import { closesFence, openingFence } from './features.ts'
import type { Fence } from './features.ts'

/**
 * What one part of a proposal is, for `readPart`: the name in its `## ` heading, the letter of its entries (`D` for
 * `D1`, `D2`), the labels its entries may have and, for a part with `### ` headings, their names.
 */
export interface PartSpec {
  part: string
  letter: string
  labels: readonly string[]
  /** The `### ` headings the part has, when its entries are sorted under them (the Findings part). */
  sections?: readonly string[]
}

/** A labelled line of an entry, as `Passed on: A daemon.` is. `name` is the label as `PartSpec` spells it. */
export interface PartLabel {
  name: string
  value: string
  line: number
}

/** One entry. `text` is what follows the colon and the lines below it up to the first label. */
export interface PartEntry {
  id: string
  title: string
  text: string
  /** The line of the entry in the item's file. */
  line: number
  labels: PartLabel[]
  /** The part's `### ` heading the entry is under, as `PartSpec` spells it. Left out when it is under none. */
  section?: string
}

/** A line of a part that is not right. `line` counts from 1 in the item's file. */
export interface PartProblem {
  part: string
  line: number
  reason: string
}

/** A word, up to three, and a colon: the start of a labelled line. */
const LABEL = /^([A-Za-z]+(?: [A-Za-z]+){0,2}):(?: +(.*))?$/
const NAME = /^## +(.*?) *$/
const SECTION = /^### +(.*?) *$/

const joined = (text: string, more: string): string => (text === '' ? more : `${text} ${more}`)

/**
 * Reads the entries of one part of a proposal's body (0.10, "A proposal's quality parts"). A part runs from a line
 * `## <name>`, matched in any case after trimming, to the next line that starts with `## `; there may be more than one,
 * and the numbers are shared by them. An entry is `- **<letter><n> <title>**: <text>`, and its continuation lines are
 * indented by at least two spaces. A continuation line that starts with a label (`Passed on:`) is a labelled line, and
 * the lines under it, up to the next label, continue its value. A fenced block is skipped wherever it is.
 *
 * Anything that is not an entry is plain text, read as nothing and no problem, so a section written before the part
 * existed reads unchanged. A line that starts like an entry and is not one is a problem, and so are `<letter>0`, a
 * number that was used before and a label the part does not have. A refused entry is not listed, and what is indented
 * under it is not reported again. `firstLine` is the line of the file that the body's first line is.
 *
 * A part that names its `sections` also reads the `### ` headings in it: an entry is under the last one, matched in any
 * case, and a heading that is none of them is a problem and leaves the entries below it under none. The reader does not
 * say whether an entry may be under none; the part does.
 */
export function readPart(
  body: string,
  firstLine: number,
  { part, letter, labels, sections }: PartSpec,
): { entries: PartEntry[]; problems: PartProblem[] } {
  const entries: PartEntry[] = []
  const problems: PartProblem[] = []
  const form = `- **${letter}<n> <title>**: <text>`
  const entryLine = new RegExp(`^- \\*\\*${letter}(0|[1-9][0-9]*) (.+?)\\*\\*:[ \\t]*(.*)$`)
  const entryStart = new RegExp(`^- \\*\\*${letter}[0-9]`)
  const heading = part.toLowerCase()
  const firstOf = new Map<string, number>()

  let inPart = false
  let fence: Fence | null = null
  let current: PartEntry | null = null
  let section: string | undefined
  // Where the lines under the entry's first line go: its text, a label's value, or nowhere after an unknown label.
  let target: PartLabel | 'text' | 'nowhere' = 'text'

  body.split('\n').forEach((raw, index) => {
    const line = raw.endsWith('\r') ? raw.slice(0, -1) : raw
    const number = firstLine + index
    const problem = (reason: string): void => void problems.push({ part, line: number, reason })

    if (fence !== null) {
      if (closesFence(line, fence)) fence = null
      return
    }
    fence = openingFence(line)
    if (fence !== null) {
      current = null
      return
    }
    if (line.startsWith('## ')) {
      inPart = NAME.exec(line)?.[1]?.toLowerCase() === heading
      current = null
      section = undefined
      return
    }
    if (!inPart || line.trim() === '') return

    const named = sections === undefined ? null : SECTION.exec(line)
    if (named !== null && sections !== undefined) {
      const written = named[1] as string
      section = sections.find((one) => one.toLowerCase() === written.toLowerCase())
      current = null
      if (section === undefined) {
        const have = sections.map((one) => `"### ${one}"`).join(', ')
        problem(`the heading "${written}" is not one of this part's headings: ${have}`)
      }
      return
    }

    const entry = entryLine.exec(line)
    if (entry !== null) {
      const [, digits, rawTitle, text] = entry as unknown as [string, string, string, string]
      const id = `${letter}${digits}`
      const title = rawTitle.trim()
      current = null
      if (title === '') return problem(`${id} needs a title: an entry is written ${form}`)
      if (digits === '0') return problem(`${id} is not an entry number: numbers count from 1`)
      const first = firstOf.get(id)
      if (first !== undefined) return problem(`${id} is already used on line ${first}; each number appears once`)
      firstOf.set(id, number)
      const under = section === undefined ? {} : { section }
      current = { id, title, text: text.trim(), line: number, labels: [], ...under }
      entries.push(current)
      target = 'text'
      return
    }
    if (entryStart.test(line)) {
      current = null
      return problem(`an entry is written ${form}`)
    }
    if (!line.startsWith('  ') || current === null) {
      current = null
      return
    }

    const text = line.trim()
    const written = LABEL.exec(text)
    if (written !== null) {
      const name = written[1] as string
      const known = labels.find((label) => label.toLowerCase() === name.toLowerCase())
      if (known !== undefined) {
        target = { name: known, value: (written[2] ?? '').trim(), line: number }
        current.labels.push(target)
        return
      }
      if (/^[A-Z]/.test(name)) {
        target = 'nowhere'
        const have = labels.map((label) => `${label}:`).join(', ')
        return problem(`${current.id} has a label that this part does not have, "${name}:"; its labels are ${have}`)
      }
    }
    if (target === 'text') current.text = joined(current.text, text)
    else if (target !== 'nowhere') target.value = joined(target.value, text)
  })

  return { entries, problems }
}
