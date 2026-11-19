// Plain words for the states and kinds the protocol names by tag (0.11, "Who it is for": a state is named in words as
// well as in colour, and plain words come before ids and terms of art). Each function is total over its type, so a new
// tag fails `pnpm typecheck` until it has words.
import type { AnswerKind, PauseCause, QueueKind, RunState, Task, ToolKind } from './types.ts'

const PAUSE_WORDS: Record<PauseCause, string> = {
  ask_human: 'Waiting for your answer',
  usage_limit: 'Waiting for the usage limit to reset',
  disk: 'Paused: the disk is nearly full',
}

export function runStateWords(state: RunState): string {
  switch (state.state) {
    case 'running':
      return 'Working'
    case 'paused':
      return PAUSE_WORDS[state.cause]
    case 'integrating':
      return 'Waiting for the pull request to merge'
    case 'ended':
      return `Finished: ${state.outcome}`
    case 'failed':
      return 'Failed'
    case 'stopped':
      return 'Stopped'
  }
}

export const CATEGORY_WORDS: Record<Task['state']['category'], string> = {
  backlog: 'Not ready yet',
  ready: 'Ready to start',
  active: 'In progress',
  review: 'In review',
  done: 'Done',
  dropped: 'Dropped',
}

/** What a Queue item is about, as a heading for its group and as the name of one item's kind. */
export const QUEUE_KIND_WORDS: Record<QueueKind, string> = {
  budget_exceeded: 'Over budget',
  failed_gate: 'A check failed',
  observer_disabled: 'An observer was turned off',
  permission: 'Asks permission',
  ask_human: 'Has a question',
  human_phase: 'Needs a person',
  usage_limit: 'Hit the usage limit',
  blocked_prompt: 'A prompt was blocked',
  budget_warning: 'Near the budget',
  unverified: 'Could not be checked',
  conflict: 'Has a conflict',
  acknowledge: 'Needs an acknowledgement',
  review: 'Ready to review',
  outcome_unknown: 'Outcome unknown',
  disk_full: 'The disk is full',
  question: 'Has a question',
  agent_setup: 'The agent tool was not ready',
  confirmation: 'Needs your confirmation',
}

export const ANSWER_WORDS: Record<AnswerKind, string> = {
  permission: 'Allow once',
  text: 'Reply',
  proceed: 'Proceed anyway',
  stop: 'Stop the run',
  retry: 'Try again',
  raise_cap: 'Raise the limit',
  accept: 'Accept',
  discard: 'Discard',
  reprompt: 'Send back',
  acknowledge: 'Acknowledge',
  resume: 'Resume',
  dismiss: 'Dismiss',
}

export const TOOL_KIND_WORDS: Record<ToolKind, string> = {
  read: 'Read a file',
  edit: 'Edited a file',
  delete: 'Deleted a file',
  move: 'Moved a file',
  search: 'Searched',
  execute: 'Ran a command',
  think: 'Thought',
  fetch: 'Fetched a page',
  other: 'Used a tool',
}

/** A length of time in the largest two units that matter: `45 s`, `12 min`, `2 h 5 min`. */
export function durationWords(ms: number): string {
  const secs = Math.round(ms / 1000)
  if (secs < 60) return `${secs} s`
  const mins = Math.floor(secs / 60)
  if (mins < 60) return `${mins} min`
  const rest = mins % 60
  return rest === 0 ? `${Math.floor(mins / 60)} h` : `${Math.floor(mins / 60)} h ${rest} min`
}
