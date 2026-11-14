import { reduce, queueOrder, type Action } from '../src/reducer'
import { initialState } from '../src/world'
import type { State } from '../src/types'

let t = 10_000

export function begin(id: string): State {
  t = 10_000
  let s = initialState('orchard', true)
  s = reduce(s, { type: 'loadScenario', id, t })
  s = reduce(s, { type: 'startTimer', t: (t += 2000) })
  return s
}

export function go(s: State, ...actions: Action[]): State {
  for (const a of actions) s = reduce(s, { ...a, t: (t += 700) })
  return s
}

export const last = (s: State) => s.session.attempts[s.session.attempts.length - 1]

export function itemFor(s: State, runId: string) {
  return s.queue.find((q) => q.runId === runId && q.status === 'pending')
}

export function answerItem(s: State, runId: string, key: string): State {
  const it = itemFor(s, runId)
  if (!it) throw new Error(`no pending item for ${runId}`)
  return go(s, { type: 'queueSelect', id: it.id }, { type: 'answer', key })
}

export { queueOrder }
