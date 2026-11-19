// The thin-loop page's own words (0.11, "Who it is for": plain words before ids and terms of art, a state in words as
// well as in colour). The words of a run's state, a Queue kind and an answer are in `../components/words.ts`, and are
// used as they are. Each function here is total over its type, so a new tag fails `pnpm typecheck` until it has words.
import { durationWords } from '../components/words.ts'
import type { ToolKind } from '../components/types.ts'
import type { AgentVersions, DiffFile, RunSummary } from './types.ts'

type PhaseState = RunSummary['phases'][number]['instances'][number]['state']

/** Where one phase of a run is, as a sentence about the agent. */
export function phaseWords(state: PhaseState): string {
  switch (state.state) {
    case 'pending':
      return 'Getting ready'
    case 'running':
      return 'The agent is working'
    case 'paused':
      return state.cause === 'ask_human' ? 'The agent is waiting for your answer' : 'The agent is paused'
    case 'ended':
      return 'The agent has finished'
    case 'failed':
      return 'The agent failed'
    case 'stopped':
      return 'The agent was stopped'
    case 'lost':
      return 'Lost contact with the agent'
  }
}

/** What a permission request asks to do, as a request: the words of `TOOL_KIND_WORDS` say what was done. */
export const REQUEST_WORDS: Record<ToolKind, string> = {
  read: 'Wants to read a file',
  edit: 'Wants to change a file',
  delete: 'Wants to delete a file',
  move: 'Wants to move a file',
  search: 'Wants to search',
  execute: 'Wants to run a command',
  think: 'Wants to think',
  fetch: 'Wants to fetch a web page',
  other: 'Wants to use a tool',
}

export const INPUT_UNKNOWN_WORDS = 'input not available until approved'

const STATUS_WORDS: Record<DiffFile['status'], string> = {
  added: 'new',
  modified: 'changed',
  deleted: 'deleted',
  renamed: 'renamed',
}

export function fileStatusWords(status: DiffFile['status']): string {
  return STATUS_WORDS[status]
}

/** A file's counts as words: a binary file has none. */
export function countWords(file: DiffFile): string {
  if (file.additions == null || file.deletions == null) return 'binary file'
  return `${file.additions} added, ${file.deletions} removed`
}

export interface Chip {
  /** The chip's accessible name. */
  readonly name: string
  readonly text: string
  readonly tooltip: string
  /** Set for the in-use chip when its version is outside the tested range: the words that mark it. */
  readonly flag: string | null
}

export interface VersionChips {
  readonly inUse: Chip
  /** Null when the daemon sends no published versions (`claude.check_published: false`): the chip is hidden. */
  readonly published: Chip | null
}

const day = (ms: number) => new Date(ms).toISOString().slice(0, 10)

function smokeWords(agent: AgentVersions): string {
  const { smoke } = agent
  if (smoke == null) return agent.in_tested_range === false ? 'The smoke check has not run on this version yet.' : ''
  if (smoke.passed) return `The smoke check passed on ${day(smoke.at_ms)}.`
  return `The smoke check failed on ${day(smoke.at_ms)}${smoke.reason ? `: ${smoke.reason}` : ''}.`
}

function inUseChip(agent: AgentVersions): Chip {
  const outside = agent.in_tested_range === false
  const range = `Tested from ${agent.tested.from} through ${agent.tested.through}.`
  const why =
    agent.in_use == null ? (agent.in_use_error ?? 'Collide could not read the version of Claude.') : null
  const lead = outside ? 'This version is outside the range Collide has tested.' : null
  return {
    name: `Claude version in use: ${agent.in_use ?? 'unknown'}`,
    text: agent.in_use ?? 'unknown',
    tooltip: [lead, why, range, smokeWords(agent)].filter((part) => part).join(' '),
    flag: outside ? 'Outside the tested range' : null,
  }
}

function publishedChip(agent: AgentVersions, nowMs: number): Chip | null {
  const { published } = agent
  if (published == null) return null
  const latest = published.latest ?? 'unknown'
  const parts = [
    published.stable ? `Stable is ${published.stable}.` : null,
    published.fetched_ms == null ? null : `Checked ${durationWords(Math.max(0, nowMs - published.fetched_ms))} ago.`,
    published.error ? `The last check failed: ${published.error}.` : null,
  ]
  return {
    name: `Latest published Claude version: ${latest}`,
    text: `latest ${latest}`,
    tooltip: parts.filter((part) => part).join(' '),
    flag: null,
  }
}

/** The two chips (0.3, Two version chips), or none when the daemon gave no entry for the agent. */
export function versionChips(agent: AgentVersions | null, nowMs: number): VersionChips | null {
  if (agent === null) return null
  return { inUse: inUseChip(agent), published: publishedChip(agent, nowMs) }
}

/** The notice after an answer that worked, in words. */
export const ACCEPTED_WORDS = 'Accepted. The work is merged into your checkout.'
export const QUEUED_WORDS = 'Collide could not finish this and put another item in the Queue.'
export const INTEGRATING_WORDS = 'Accepted. The pull request is open and not merged yet.'
export const SENT_BACK_WORDS = 'Sent back. The agent is working on it again.'
export const DISMISSED_WORDS = 'Dismissed. Nothing was written.'
export const DISCARDED_WORDS = 'Discarded. The work and its copy of the project are deleted.'
export const NO_BOARD_WORDS = 'Choose a repository first.'
