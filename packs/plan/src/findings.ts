import { readPart } from './parts.ts'
import type { PartEntry, PartProblem, PartSpec } from './parts.ts'

/**
 * One entry `R<n>` of the Findings part. `severity` is as written. `section` is left out for an entry under neither
 * heading. A finding with neither `accepted` nor `dismissed` is `open`.
 */
export interface Finding {
  id: string
  section?: string
  severity: string
  title: string
  text: string
  target?: string
  accepted?: string
  dismissed?: string
  open: boolean
  line: number
}

export const TARGET = 'Target'
export const ACCEPTED = 'Accepted'
export const DISMISSED = 'Dismissed'
export const SECTIONS = ['Red-team', 'Pre-mortem']

/** The Findings part is the `## Findings` section, with the headings of `SECTIONS`, entries `R<n>`. */
export const FINDING_PART: PartSpec = {
  part: 'Findings',
  letter: 'R',
  labels: [TARGET, ACCEPTED, DISMISSED],
  sections: SECTIONS,
}

const SEVERITIES = ['high', 'medium', 'low']
const TARGET_SHAPE = /^[FDA][1-9][0-9]*$/
const SEVERITY_AND_TITLE = /^(\S+)\s*(.*)$/

function findingOf(entry: PartEntry, own: ReadonlySet<string>, problems: PartProblem[]): Finding {
  const { id } = entry
  const problem = (line: number, reason: string): void => void problems.push({ part: FINDING_PART.part, line, reason })
  const [, severity = '', title = ''] = SEVERITY_AND_TITLE.exec(entry.title) ?? []
  const finding: Finding = { id, severity, title, text: entry.text, open: true, line: entry.line }
  if (entry.section !== undefined) finding.section = entry.section
  else {
    const have = SECTIONS.map((name) => `"### ${name}"`).join(' and ')
    problem(entry.line, `${id} is outside the headings ${have}`)
  }
  if (!SEVERITIES.includes(severity)) {
    const have = `${SEVERITIES.slice(0, -1).join(', ')} or ${SEVERITIES.at(-1)}`
    problem(entry.line, `${id} has a severity that is none of ${have}: "${severity}"`)
  } else if (title === '') {
    problem(entry.line, `${id} has no title after its severity: write - **${id} <severity> <title>**: <text>`)
  }

  const seen = new Set<string>()
  for (const label of entry.labels) {
    if (seen.has(label.name)) {
      problem(label.line, `${id} has "${label.name}:" more than once; the first is kept`)
    } else if (label.name === TARGET) {
      seen.add(label.name)
      if (!TARGET_SHAPE.test(label.value)) {
        problem(label.line, `${id} has a Target that is none of F<n>, D<n> or A<n>: "${label.value}"`)
        continue
      }
      finding.target = label.value
      if (!own.has(label.value)) problem(label.line, `${id} targets ${label.value}, which this proposal does not have`)
    } else if (label.value === '') {
      problem(label.line, `${id} has an empty ${label.name}:; the finding stays open until it says something`)
    } else {
      seen.add(label.name)
      if (label.name === ACCEPTED) finding.accepted = label.value
      else finding.dismissed = label.value
      if (finding.accepted !== undefined && finding.dismissed !== undefined) {
        problem(label.line, `${id} has both ${ACCEPTED}: and ${DISMISSED}:; a finding is accepted or dismissed`)
      }
    }
  }
  finding.open = finding.accepted === undefined && finding.dismissed === undefined
  return finding
}

/**
 * Reads a proposal's Findings part from its body (0.10, "A proposal's quality parts"). `own` is the ids of the
 * features, decisions and assumptions that the proposal has, which a `Target:` is checked against. A finding that is
 * open, whatever its severity, is listed and is no problem. A problem never stops an entry from being listed, and an
 * entry outside the two headings is kept as written; only a repeated number stops one, as the part's reader says.
 * Problems are in line order.
 */
export function readFindings(
  body: string,
  firstLine: number,
  own: ReadonlySet<string>,
): { findings: Finding[]; problems: PartProblem[] } {
  const { entries, problems } = readPart(body, firstLine, FINDING_PART)
  const findings = entries.map((entry) => findingOf(entry, own, problems))
  return { findings, problems: problems.sort((a, b) => a.line - b.line) }
}
