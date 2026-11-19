import { PROPOSAL, QUESTION } from './item.ts'

/**
 * The moves of a proposal, by the status it is in (0.10, Planning sources): the edges are here and nowhere else, so a
 * change is one line. `superseded` is final.
 */
export const PROPOSAL_MOVES: Readonly<Record<string, readonly string[]>> = {
  draft: ['proposed', 'rejected'],
  proposed: ['accepted', 'deferred', 'rejected', 'draft'],
  deferred: ['proposed', 'rejected'],
  accepted: ['superseded'],
  rejected: ['draft'],
  superseded: [],
}

/** The reason a proposal in `from` may not move to `to`, naming the moves it has, or null when it may. */
export function moveRefusal(id: string, from: string, to: string): string | null {
  const moves = PROPOSAL_MOVES[from] ?? []
  if (moves.includes(to)) return null
  const has =
    moves.length === 0 ? `${from} is final: it has no moves` : `from ${from} it can move to ${moves.join(', ')}`
  return `${id} is ${from} and cannot move to ${to}; ${has}`
}

/** The writes that only a person makes. `reopen` is a deferred proposal's return to `proposed`. */
export type PersonOnly = 'accept' | 'defer' | 'reject' | 'resolve' | 'reopen'

const VERB: Record<PersonOnly, string> = {
  accept: 'accept',
  defer: 'defer',
  reject: 'reject',
  resolve: 'resolve',
  reopen: 'return to proposed',
}

/**
 * Which person-only write `op` is on an item of `kind` that is in `from`, or null when any actor may make it. This is
 * the second wall, and it reads the same writes as the daemon's `person_only_request` (host/src/auth.rs): the four
 * ops, whatever the kind; a proposal's status write to `accepted`, `deferred` or `rejected`; and a deferred
 * proposal's write to `proposed`. Only a proposal and a question have any.
 */
export function personOnlyWrite(
  op: string,
  kind: string,
  from: string,
  args: Record<string, unknown>,
): PersonOnly | null {
  if (kind !== PROPOSAL && kind !== QUESTION) return null
  if (op === 'accept' || op === 'defer' || op === 'reject' || op === 'resolve') return op
  if (op !== 'setStatus' || kind !== PROPOSAL) return null
  const to = args.status
  if (to === 'accepted') return 'accept'
  if (to === 'deferred') return 'defer'
  if (to === 'rejected') return 'reject'
  return from === 'deferred' && to === 'proposed' ? 'reopen' : null
}

/** What the provider is told of who asked (`ApplyParams.actor`). None, or one with no kind, is a person. */
export interface Asker {
  kind?: string
  at?: string | null
  run?: string | null
}

/** The refusal for an agent that asks for a person-only write, in the provider's own words. */
export function agentRefusal(id: string, write: PersonOnly, asker: Asker | null | undefined): string | null {
  if (asker?.kind !== 'agent') return null
  const who = typeof asker.run === 'string' && asker.run !== '' ? `the agent of run ${asker.run}` : 'an agent'
  return `only a person may ${VERB[write]} ${id}; this request came from ${who}, and a person does it in the app`
}
