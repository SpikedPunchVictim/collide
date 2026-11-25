// The recordings fake Claude plays, made from the host's own (`host/tests/adapters/claude/fixtures/recordings/`) the
// way `host/tests/adapters/claude/quick_run.rs` makes its own: the turn it waits for and the steps after it are
// replaced. A recording is the text of a `.jsonl` file, one step a line.
import { readFileSync } from 'node:fs'
import { join } from 'node:path'

const REPO_ROOT = join(import.meta.dirname, '..', '..', '..')

/** The folder of the adapter's fixtures: fake Claude's script and its recordings. */
export const FIXTURES = join(REPO_ROOT, 'host', 'tests', 'adapters', 'claude', 'fixtures')

export const FAKE_SCRIPT = join(FIXTURES, 'fake_claude.py')

/** A step of a recording. */
export type Step = Record<string, unknown>

function recording(name: string): string {
  return readFileSync(join(FIXTURES, 'recordings', `${name}.jsonl`), 'utf8')
}

/** The first turn, which `report.jsonl` waits for: any message of the user. */
const PROMPT_TURN = '{"expect":{"type":"user","message":{"role":"user"}}}'

/** `report.jsonl` with `turn` in place of the turn it waits for, and `steps` after it. */
function reportWith(turn: string, steps: readonly Step[]): string {
  const lines: string[] = []
  let inserted = false
  for (const line of recording('report').split('\n')) {
    if (!inserted && line.startsWith('{"expect"')) {
      lines.push(turn, ...steps.map((step) => JSON.stringify(step)))
      inserted = true
    } else {
      lines.push(line)
    }
  }
  return lines.join('\n')
}

/** What the agent does: writes `file` and commits it. */
export function adds(file: string): Step[] {
  return [{ write_file: { path: file, text: 'from the agent\n' } }, { git_commit: `Add ${file}` }]
}

/** A phase that does `steps` for the first prompt and reports. */
export function firstPhase(steps: readonly Step[]): string {
  return reportWith(PROMPT_TURN, steps)
}

/** A phase for a resumed session: it waits for `words` as the turn, does `steps` and reports. */
export function resumedPhase(words: string, steps: readonly Step[]): string {
  const turn = JSON.stringify({
    expect: { type: 'user', message: { role: 'user', content: [{ type: 'text', text: words }] } },
  })
  return reportWith(turn, steps)
}

/** A Bash call that waits for the person's answer, which the recording expects to be allow. */
export function permissionPhase(): string {
  return recording('permission-request')
}

/**
 * A Bash call that waits for the person's answer, with `before` done first and `after` done once it is allowed: a run
 * that has written a file and is held where the person can see it, and that goes on when they answer (ts-0189-fg).
 */
export function heldPhase(before: readonly Step[], after: readonly Step[]): string {
  const lines: string[] = []
  let expects = 0
  for (const line of recording('permission-request').split('\n')) {
    lines.push(line)
    if (!line.startsWith('{"expect"')) continue
    expects += 1
    const steps = expects === 1 ? before : expects === 2 ? after : []
    lines.push(...steps.map((step) => JSON.stringify(step)))
  }
  return lines.join('\n')
}
