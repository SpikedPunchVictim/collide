import { readEvidenceList } from './decisions.ts'
import type { Evidence, ItemLookup } from './decisions.ts'
import { readPart } from './parts.ts'
import type { PartEntry, PartProblem, PartSpec } from './parts.ts'

/**
 * One entry `A<n>` of the Assumption ledger. `state` is one of `STATES`, and `unverified` when the entry has no
 * `State:` or one that is not valid. `spikeStatus` is the status of the spike `spike` names, when it names one.
 */
export interface Assumption {
  id: string
  title: string
  text: string
  state: string
  evidence: Evidence[]
  spike?: string
  spikeStatus?: string
  line: number
}

export const STATE = 'State'
export const EVIDENCE = 'Evidence'
export const SPIKE_LABEL = 'Spike'

/** The Assumption ledger is the `## Assumptions` section, entries `A<n>` (0.10, "A proposal's quality parts"). */
export const ASSUMPTION_PART: PartSpec = { part: 'Assumptions', letter: 'A', labels: [STATE, EVIDENCE, SPIKE_LABEL] }

const STATES = ['unverified', 'verified', 'contradicted', 'accepted risk']
const UNVERIFIED = 'unverified'
const SPIKE_KIND = 'spike'

function assumptionOf(entry: PartEntry, lookup: ItemLookup, problems: PartProblem[]): Assumption {
  const { id } = entry
  const problem = (line: number, reason: string): void =>
    void problems.push({ part: ASSUMPTION_PART.part, line, reason })
  const assumption: Assumption = {
    id,
    title: entry.title,
    text: entry.text,
    state: UNVERIFIED,
    evidence: [],
    line: entry.line,
  }
  const seen = new Set<string>()
  for (const label of entry.labels) {
    if (label.name === EVIDENCE) {
      for (const cited of readEvidenceList(label.value, lookup)) {
        assumption.evidence.push(cited.evidence)
        if (cited.problem !== undefined) problem(label.line, `${id} cites "${cited.piece}": ${cited.problem}`)
      }
    } else if (seen.has(label.name)) {
      problem(label.line, `${id} has "${label.name}:" more than once; the first is kept`)
    } else if (label.name === STATE) {
      seen.add(label.name)
      if (STATES.includes(label.value)) assumption.state = label.value
      else {
        const have = `${STATES.slice(0, -1).join(', ')} or ${STATES.at(-1)}`
        problem(label.line, `${id} has a State that is none of ${have}: "${label.value}"; it reads as ${UNVERIFIED}`)
      }
    } else if (label.name === SPIKE_LABEL) {
      seen.add(label.name)
      if (label.value === '') continue
      assumption.spike = label.value
      const known = lookup(label.value)
      if (known === undefined) {
        problem(label.line, `${id} has a Spike, "${label.value}", that names no item`)
      } else if (known.kind !== SPIKE_KIND) {
        problem(label.line, `${id} has a Spike, "${label.value}", that is not a spike`)
      } else {
        assumption.spikeStatus = known.status
      }
    }
  }
  return assumption
}

/**
 * Reads a proposal's Assumption ledger from its body (0.10, "A proposal's quality parts"). `lookup` is the scan, which
 * says whether an evidence id or a spike names an item, of what kind, and what status it has. A missing `State:` reads
 * as `unverified` and is not a problem. Problems are in line order, and a problem never stops an entry from being
 * listed; only a repeated number does, as the part's reader says.
 */
export function readAssumptions(
  body: string,
  firstLine: number,
  lookup: ItemLookup,
): { assumptions: Assumption[]; problems: PartProblem[] } {
  const { entries, problems } = readPart(body, firstLine, ASSUMPTION_PART)
  const assumptions = entries.map((entry) => assumptionOf(entry, lookup, problems))
  return { assumptions, problems: problems.sort((a, b) => a.line - b.line) }
}
