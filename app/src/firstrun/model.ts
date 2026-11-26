// What the first-run page says of each check (ts-0194-qp; note 13; the mock's `welcomeHtml`). The daemon's checks carry
// Collide's own words and the command a fix names (`FirstRunCheck`), so this only chooses the title and the mark of
// each row. Nothing here reads the owner, so a test draws it for every state.
import type { BoardChecksResultOutput } from '../owner/generated.ts'

export type Check = BoardChecksResultOutput['checks'][number]

/** How a row is marked: passed, failed, or a check that did not run to an answer, shown as neither. */
export type Mark = 'ok' | 'no' | 'unknown'

export interface CheckRow {
  readonly id: Check['id']
  readonly mark: Mark
  readonly title: string
  /** Collide's words for the row: the fix when it failed, what was found when it passed. */
  readonly detail: string
  /** The command a Copy button puts on the clipboard, when the fix is one. */
  readonly copy: string | null
}

const TITLES: Record<string, readonly [passed: string, failed: string]> = {
  git_repo: ['A git repository', 'Not a git repository'],
  claude_installed: ['Claude Code is installed', 'Claude Code is not installed'],
  claude_logged_in: ['Claude Code is logged in', 'Claude Code is not logged in'],
}

const UNKNOWN_TITLES: Record<string, string> = {
  claude_installed: 'Could not check that Claude Code is installed',
  claude_logged_in: 'Could not check that Claude Code is logged in',
}

const GIT_FIX =
  "Collide needs git to keep each agent's work apart. Making it one adds a `.git` folder and changes nothing else."

/** The rows of the checks the page shows, in the daemon's order. A check the page has no row for is left out. */
export function checkRows(checks: readonly Check[]): readonly CheckRow[] {
  return checks.flatMap((check) => {
    const titles = TITLES[check.id]
    if (titles === undefined) return []
    const mark: Mark = check.could_not_check === true ? 'unknown' : check.ok ? 'ok' : 'no'
    const title = mark === 'unknown' ? (UNKNOWN_TITLES[check.id] ?? titles[0]) : titles[check.ok ? 0 : 1]
    const detail = check.id === 'git_repo' && !check.ok ? GIT_FIX : check.message
    return [{ id: check.id, mark, title, detail, copy: check.copy ?? null }]
  })
}

/** Whether the board can open: no check the daemon marks required has failed, and the git check is there and passed. */
export function canOpen(checks: readonly Check[]): boolean {
  const git = checks.find((check) => check.id === 'git_repo')
  return git?.ok === true && checks.every((check) => check.ok || !check.required)
}

/** The folder is not a git repository, so Make it one is offered. */
export function needsGit(checks: readonly Check[]): boolean {
  return checks.some((check) => check.id === 'git_repo' && !check.ok)
}

/** A message with backticks as the daemon writes it, split into plain and code parts. */
export function parts(message: string): readonly { readonly text: string; readonly code: boolean }[] {
  return message.split('`').map((text, index) => ({ text, code: index % 2 === 1 }))
}
