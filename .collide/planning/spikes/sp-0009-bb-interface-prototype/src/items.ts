// Queue kinds: what a and d mean per kind, in each variant, and the item specs.
import type { Effect, ItemKind, ItemSpec, KeysVariant, Run, State } from './types'

export interface Act {
  effect: Effect
  label: string
  /** what the action does, shown under the button */
  hint: string
  /** the confirm variant arms these with a first press */
  consequential?: boolean
}

type Keys = { a: Act; d: Act; r?: Act }

const allow: Keys = {
  a: { effect: 'allow', label: 'Allow once', hint: 'The run continues and the call goes ahead.' },
  d: { effect: 'deny', label: 'Deny', hint: 'The call is refused and the agent is told.' },
}
const apply: Keys = {
  a: { effect: 'apply', label: 'Apply edit', hint: 'The edit is written to the worktree.' },
  d: { effect: 'reject', label: 'Reject edit', hint: 'The edit is dropped and the agent is told.' },
}

const LITERAL: Record<ItemKind, Keys> = {
  permission: allow,
  command: allow,
  edit: apply,
  gate: {
    a: { effect: 'proceed', label: 'Proceed unchecked', hint: 'The phase moves on without a gate verdict.', consequential: true },
    d: { effect: 'stop', label: 'Stop the run', hint: 'The run ends here. Its work is kept.', consequential: true },
    r: { effect: 'retry', label: 'Retry the gate', hint: 'The gate command runs again.' },
  },
  review: {
    a: { effect: 'accept', label: 'Accept', hint: 'The diff is kept and the task is done.', consequential: true },
    d: { effect: 'discard', label: 'Discard', hint: 'The worktree and diff are thrown away.', consequential: true },
    r: { effect: 'reprompt', label: 'Re-prompt', hint: 'Send the run back with a note.' },
  },
  budget: {
    a: { effect: 'raise', label: 'Raise cap by $2', hint: 'The run continues with a higher cap.', consequential: true },
    d: { effect: 'stop', label: 'Stop the run', hint: 'The run ends here. Its work is kept.', consequential: true },
  },
}

const SAFE: Record<ItemKind, Keys> = {
  ...LITERAL,
  gate: {
    a: { effect: 'retry', label: 'Retry the gate', hint: 'The gate command runs again.' },
    d: { effect: 'hold', label: 'Hold the run', hint: 'The run pauses at the gate. Nothing is lost.' },
    r: { effect: 'stop', label: 'Stop the run', hint: 'The run ends here. Its work is kept.' },
  },
  review: {
    a: { effect: 'accept', label: 'Accept', hint: 'The diff is kept and the task is done.' },
    d: { effect: 'reprompt', label: 'Send back', hint: 'Send the run back with a note.' },
    r: { effect: 'discard', label: 'Discard', hint: 'The worktree and diff are thrown away.' },
  },
  budget: {
    a: { effect: 'continue', label: 'Continue, +$1', hint: 'The run continues to the end of this phase.' },
    d: { effect: 'pause', label: 'Pause the run', hint: 'The run pauses. Nothing is lost.' },
    r: { effect: 'stop', label: 'Stop the run', hint: 'The run ends here. Its work is kept.' },
  },
}

export const ACTIONS: Record<KeysVariant, Record<ItemKind, Keys>> = {
  literal: LITERAL,
  safe: SAFE,
  confirm: LITERAL,
}

export function actFor(v: KeysVariant, kind: ItemKind, key: string): Act | undefined {
  return ACTIONS[v][kind][key as 'a' | 'd' | 'r']
}

/** In the confirm variant a consequential action needs a second press. */
export function needsConfirm(v: KeysVariant, act: Act): boolean {
  return v === 'confirm' && !!act.consequential
}

export const KIND_LABEL: Record<ItemKind, string> = {
  permission: 'Permission request',
  command: 'Command',
  edit: 'Edit',
  gate: 'Failed gate',
  review: 'Review',
  budget: 'Budget exceeded',
}

export const KIND_ORDER: ItemKind[] = ['permission', 'command', 'edit', 'gate', 'budget', 'review']

export function reviewSpec(run: Run, s: State): ItemSpec {
  const task = s.tasks.find((t) => t.id === run.taskId)
  const add = run.diff.reduce((a, f) => a + f.add, 0)
  const del = run.diff.reduce((a, f) => a + f.del, 0)
  return {
    kind: 'review',
    title: `Review ${run.id}${task ? `: ${task.title}` : ''}`,
    body: run.report,
    cwd: `work/${run.id}`,
    why: 'The run finished. Its diff waits for your decision.',
    ctx: {
      why: ['finished run to review', 'gates: typecheck pass, tests pass'],
      does: [`${run.diff.length} files changed, +${add} -${del}`, 'accepting keeps the diff and marks the task done'],
      task: task ? [`${task.id} ${task.title}`, task.body] : ['scratch task'],
      led: ['two reviewers read the diff', `${run.findings.length} findings, adjudicated`],
      history: ['this repo: 14 runs accepted, 3 discarded'],
      words: [run.report],
    },
    expect: ['accept', 'reprompt'],
  }
}
