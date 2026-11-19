import { parseFeatureTarget } from './feature-target.ts'
import { ID_PATTERN } from './item.ts'
import { readPart } from './parts.ts'
import type { PartEntry, PartProblem, PartSpec } from './parts.ts'

/** What the scan knows of an item: its status, and the features it has when it is a proposal (none otherwise). */
export type ItemLookup = (id: string) => { status: string; features: string[] } | undefined

/** One piece of an `Evidence:` label. `status` is the cited item's, and `proposal` is the proposal of a feature. */
export interface Evidence {
  target: string
  kind: 'item' | 'feature' | 'path'
  status?: string
  proposal?: string
}

/** One entry `D<n>` of the Decision log. The labels' values are text as written. */
export interface Decision {
  id: string
  title: string
  text: string
  passedOn: string[]
  evidence: Evidence[]
  revisitIf?: string
  confidence?: string
  line: number
}

export const PASSED_ON = 'Passed on'
export const EVIDENCE = 'Evidence'
export const REVISIT_IF = 'Revisit if'
export const CONFIDENCE = 'Confidence'

/** The Decision log is the `## Decision` section, entries `D<n>` (0.10, "A proposal's quality parts"). */
export const DECISION_PART: PartSpec = {
  part: 'Decision',
  letter: 'D',
  labels: [PASSED_ON, EVIDENCE, REVISIT_IF, CONFIDENCE],
}

const CONFIDENCES = ['low', 'medium', 'high']
const idRegex = new RegExp(ID_PATTERN)

type Cited = { evidence: Evidence; problem?: string }

/**
 * Reads one piece of an `Evidence:` label. An item or a feature is looked up in the scan; a path is checked for its
 * shape only (relative, with no `..` segment), because a path that is not in this checkout may be on another branch.
 */
export function readEvidence(piece: string, lookup: ItemLookup): Cited {
  const hash = piece.indexOf('#')
  const head = hash === -1 ? piece : piece.slice(0, hash)
  if (!idRegex.test(head)) {
    const shape = piece.startsWith('/') || piece.split('/').includes('..')
    const problem = shape ? 'a repo path is relative, with no .. in it' : undefined
    return { evidence: { target: piece, kind: 'path' }, ...(problem === undefined ? {} : { problem }) }
  }
  if (hash === -1) {
    const known = lookup(piece)
    if (known === undefined) return { evidence: { target: piece, kind: 'item' }, problem: 'it names no item' }
    return { evidence: { target: piece, kind: 'item', status: known.status } }
  }
  const parsed = parseFeatureTarget(piece)
  if (!parsed.ok) return { evidence: { target: piece, kind: 'feature' }, problem: parsed.reason }
  const evidence: Evidence = { target: piece, kind: 'feature', proposal: parsed.item }
  const known = lookup(parsed.item)
  if (known === undefined) return { evidence, problem: `${parsed.item} names no item` }
  if (!known.features.includes(parsed.feature)) {
    return { evidence, problem: `${parsed.item} has no feature ${parsed.feature}` }
  }
  return { evidence }
}

function decisionOf(entry: PartEntry, lookup: ItemLookup, problems: PartProblem[]): Decision {
  const { id } = entry
  const problem = (line: number, reason: string): void => void problems.push({ part: DECISION_PART.part, line, reason })
  const decision: Decision = { id, title: entry.title, text: entry.text, passedOn: [], evidence: [], line: entry.line }
  const seen = new Set<string>()
  for (const label of entry.labels) {
    if (label.name === PASSED_ON) {
      if (label.value !== '') decision.passedOn.push(label.value)
    } else if (label.name === EVIDENCE) {
      for (const piece of label.value.split(',').map((one) => one.trim()).filter((one) => one !== '')) {
        const cited = readEvidence(piece, lookup)
        decision.evidence.push(cited.evidence)
        if (cited.problem !== undefined) problem(label.line, `${id} cites "${piece}": ${cited.problem}`)
      }
    } else if (seen.has(label.name)) {
      problem(label.line, `${id} has "${label.name}:" more than once; the first is kept`)
    } else if (label.name === REVISIT_IF) {
      seen.add(label.name)
      if (label.value !== '') decision.revisitIf = label.value
    } else if (label.name === CONFIDENCE) {
      seen.add(label.name)
      if (CONFIDENCES.includes(label.value)) decision.confidence = label.value
      else problem(label.line, `${id} has a Confidence that is none of low, medium or high: "${label.value}"`)
    }
  }
  if (decision.passedOn.length === 0) {
    problem(entry.line, `${id} names no alternative: add a line "Passed on: <what was not taken, and why>"`)
  }
  return decision
}

/**
 * Reads a proposal's Decision log from its body (0.10, "A proposal's quality parts"). `lookup` is the scan, which says
 * whether an evidence id names an item and what status it has. Problems are in line order, and a problem never
 * stops an entry from being listed; only a repeated number does, as the part's reader says.
 */
export function readDecisions(
  body: string,
  firstLine: number,
  lookup: ItemLookup,
): { decisions: Decision[]; problems: PartProblem[] } {
  const { entries, problems } = readPart(body, firstLine, DECISION_PART)
  const decisions = entries.map((entry) => decisionOf(entry, lookup, problems))
  return { decisions, problems: problems.sort((a, b) => a.line - b.line) }
}
