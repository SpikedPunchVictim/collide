// The colour of each category (0.10), as the classes the cards, columns and drawer use for its dot and its words.
import type { Category } from './types.ts'

export const DOT: Record<Category, string> = {
  backlog: 'bg-c-backlog',
  ready: 'bg-c-ready',
  active: 'bg-c-active',
  review: 'bg-c-review',
  done: 'bg-c-done',
  dropped: 'bg-c-dropped',
}

export const TEXT: Record<Category, string> = {
  backlog: 'text-c-backlog',
  ready: 'text-c-ready',
  active: 'text-c-active',
  review: 'text-c-review',
  done: 'text-c-done',
  dropped: 'text-c-dropped',
}
