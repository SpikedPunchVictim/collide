// What a repo's Collide folder brings that waits on trust (ts-0194-qp; note 10; the mock's `trustModal`), read off the
// daemon's `board.trust_review` answer. 0.9 holds a repo's packs, views and `config.yaml` until the line is trusted, so
// the prompt names what the folder brings and nothing else. `board.yaml` alone, listing no pack, brings nothing that
// waits, so it asks nothing. The words come from the answer, so a test draws them for any review.
import type { BoardTrustReviewResultOutput } from '../owner/generated.ts'

export type Review = BoardTrustReviewResultOutput

/** What the folder brings: its packs (by source), the names of its views, and whether it has settings. */
export interface Brought {
  readonly packs: readonly string[]
  readonly views: readonly string[]
  readonly settings: boolean
}

const VIEW_PREFIX = 'views/'

/** What the review says the line brings, or null when it brings nothing that waits on trust. */
export function broughtBy(review: Review): Brought | null {
  const packs = (review.packs ?? []).map((pack) => pack.source)
  const views = (review.changes ?? [])
    .filter((change) => change.kind !== 'removed' && change.name.startsWith(VIEW_PREFIX))
    .map((change) => change.name.slice(VIEW_PREFIX.length).replace(/\.yaml$/, ''))
  const settings = (review.changes ?? []).some((change) => change.kind !== 'removed' && change.name === 'config.yaml')
  return packs.length === 0 && views.length === 0 && !settings ? null : { packs, views, settings }
}

const NUMBERS = ['no', 'one', 'two', 'three', 'four', 'five', 'six', 'seven', 'eight', 'nine', 'ten']

function count(n: number, one: string, many: string): string {
  return `${NUMBERS[n] ?? n} ${n === 1 ? one : many}`
}

/** "one pack of its own, two views and its settings": what the repo brings, in the person's words. */
export function bringsWords(brought: Brought): string {
  const things = [
    brought.packs.length === 0 ? null : `${count(brought.packs.length, 'pack', 'packs')} of its own`,
    brought.views.length === 0 ? null : count(brought.views.length, 'view', 'views'),
    brought.settings ? 'its settings' : null,
  ].filter((thing): thing is string => thing !== null)
  if (things.length <= 1) return things.join('')
  return `${things.slice(0, -1).join(', ')} and ${things[things.length - 1]}`
}

/** The kinds of thing the repo brings, for the title: "packs", "views and settings". */
export function kindsWords(brought: Brought): string {
  const kinds = [
    brought.packs.length === 0 ? null : 'packs',
    brought.views.length === 0 ? null : 'views',
    brought.settings ? 'settings' : null,
  ].filter((kind): kind is string => kind !== null)
  if (kinds.length <= 1) return kinds.join('')
  return `${kinds.slice(0, -1).join(', ')} and ${kinds[kinds.length - 1]}`
}
