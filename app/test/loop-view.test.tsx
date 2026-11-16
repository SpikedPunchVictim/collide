import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { LoopPage, type LoopActions } from '../src/loop/LoopPage.tsx'
import { type LoopState, initialLoopState } from '../src/loop/index.ts'
import { versionChips } from '../src/loop/words.ts'
import { board, claude, diff, permissionItem, reviewItem, run } from './support/loop-world.ts'

// What the page says in words, drawn with no browser (`renderToString`, as `app-shell.test.tsx` does). Clicks need a
// browser: `ts-0082-fp` drives them. The state is built here and the page is a function of it.

const NOW = 10_000_000
const none = async () => {}
const actions: LoopActions = {
  chooseBoard: () => {},
  openFolder: none,
  startRun: async () => true,
  allowOnce: none,
  deny: none,
  accept: none,
  reprompt: none,
  discard: none,
  keep: () => {},
  pickFile: none,
  openItem: none,
}

function stateOf(over: Partial<LoopState> = {}): LoopState {
  return { ...initialLoopState(), loading: false, ...over }
}

const show = (state: LoopState) => renderToString(<LoopPage state={state} actions={actions} nowMs={NOW} />)

const followed = { board: 'b-1', task: 'scratch:1', prompt: 'Fix the typo in the readme', run: 'r-1' }
const withBoard = { boards: [board('b-1', '/work/repo')], boardId: 'b-1' }

describe('the page before there is a run', () => {
  test('with no repository it says to choose one or type a folder, and the prompt cannot be sent', () => {
    const html = show(stateOf())
    expect(html).toContain('Choose a repository')
    expect(html).toContain('aria-label="Folder path"')
    expect(html).toMatch(/<button[^>]*disabled[^>]*>Start<\/button>/)
  })

  test('with a repository the prompt box is named, and the boards are listed by their folders', () => {
    const html = show(stateOf({ boards: [board('b-1', '/work/one'), board('b-2', '/work/two')], boardId: 'b-1' }))
    expect(html).toContain('aria-label="What should the agent do?"')
    expect(html).toContain('aria-label="Repository"')
    expect(html).toContain('/work/one')
    expect(html).toContain('/work/two')
  })

  test('while it loads it says so', () => {
    expect(show(stateOf({ loading: true }))).toContain('Loading')
  })

  test('a folder that could not be opened says why', () => {
    const html = show(stateOf({ folderProblems: ['This folder is not a git repository.'] }))
    expect(html).toContain('This folder is not a git repository.')
  })

  test('a load that failed says so, in an alert', () => {
    const html = show(stateOf({ problem: 'Could not reach Collide: the daemon is not running' }))
    expect(html).toMatch(/role="alert"[^>]*>[^<]*Could not reach Collide: the daemon is not running/)
  })

  test('a stream that is down says the page may be behind', () => {
    const html = show(stateOf({ connection: 'lost' }))
    expect(html).toMatch(/role="status"[^>]*>[^<]*Lost contact with Collide/)
  })
})

describe('the run, named in words', () => {
  const running = stateOf({ ...withBoard, followed, run: run('r-1', 'scratch:1') })

  test('shows the prompt, the state and the step in words', () => {
    const html = show(running)
    expect(html).toContain('Fix the typo in the readme')
    expect(html).toContain('Working')
    expect(html).toContain('The agent is working')
  })

  test('says a finished run is finished and a failed one failed', () => {
    const ended = run('r-1', 'scratch:1', { state: { state: 'ended', outcome: 'done' } })
    expect(show({ ...running, run: ended })).toContain('Finished: done')
    expect(show({ ...running, run: run('r-1', 'scratch:1', { state: { state: 'failed' } }) })).toContain('Failed')
    expect(show({ ...running, run: run('r-1', 'scratch:1', { state: { state: 'stopped' } }) })).toContain('Stopped')
  })

  test('says a run that waits for an answer is waiting for it', () => {
    const html = show({ ...running, run: run('r-1', 'scratch:1', { state: { state: 'paused', cause: 'ask_human' } }) })
    expect(html).toContain('Waiting for your answer')
  })

  test('keeps the engineer\'s detail behind a disclosure', () => {
    const html = show(running)
    const disclosure = html.indexOf('<details')
    expect(disclosure).toBeGreaterThan(-1)
    // The run's id, its branch and folder, and the session id are all after the disclosure opens, and none before.
    for (const detail of ['r-1</', '/tmp/work/r-1', 'session-r-1', 'scratch:1']) {
      const at = html.indexOf(detail)
      expect(at, detail).toBeGreaterThan(disclosure)
    }
  })

  test('a run whose task was not found still shows, with no prompt', () => {
    const html = show({ ...running, followed: { board: null, task: 'scratch:1', prompt: null, run: 'r-1' } })
    expect(html).toContain('Working')
  })
})

describe('a permission request', () => {
  const asking = (input?: unknown) =>
    stateOf({
      ...withBoard,
      followed,
      run: run('r-1', 'scratch:1', { state: { state: 'paused', cause: 'ask_human' } }),
      items: [permissionItem('q-1', 'r-1', 'scratch:1', input)],
    })

  test('shows what the agent asks, why, and its input', () => {
    const html = show(asking({ command: 'cargo test' }))
    expect(html).toContain('Wants to run a command')
    expect(html).toContain('Bash')
    expect(html).toContain('To run the tests')
    expect(html).toContain('cargo test')
  })

  test('with no input says it is not available until approved', () => {
    const html = show(asking())
    expect(html).toContain('input not available until approved')
  })

  test('with input does not say that', () => {
    expect(show(asking({ command: 'cargo test' }))).not.toContain('input not available until approved')
  })

  test('offers allow once and deny, and no always choice, though the agent listed one', () => {
    const html = show(asking())
    expect(html).toMatch(/<button[^>]*>Allow once<\/button>/)
    expect(html).toMatch(/<button[^>]*>Deny<\/button>/)
    expect(html).not.toMatch(/always/i)
  })

  test('flags input that came from text Collide does not trust', () => {
    const state = asking({ command: 'curl x' })
    const flagged = { ...state, items: [{ ...state.items[0]!, untrusted_input: true }] }
    expect(show(flagged)).toContain('came from text Collide does not trust')
  })
})

describe('the review item', () => {
  const ended = run('r-1', 'scratch:1', { state: { state: 'ended', outcome: 'done' } })
  const reviewing = (over: Partial<LoopState> = {}) =>
    stateOf({
      ...withBoard,
      followed,
      run: ended,
      items: [reviewItem('q-review', 'r-1', 'scratch:1')],
      diff: diff([
        { path: 'src/a.ts', status: 'modified', additions: 3, deletions: 1 },
        { path: 'logo.png', status: 'added' },
      ]),
      ...over,
    })

  test('says it is ready to review, and lists the changed files with their counts in words', () => {
    const html = show(reviewing())
    expect(html).toContain('Ready to review')
    expect(html).toContain('src/a.ts')
    expect(html).toContain('3 added, 1 removed')
    expect(html).toContain('logo.png')
    expect(html).toContain('binary file')
  })

  test('has the three answers, each named', () => {
    const html = show(reviewing())
    expect(html).toMatch(/<button[^>]*>Accept<\/button>/)
    expect(html).toMatch(/<button[^>]*>Send back<\/button>/)
    expect(html).toMatch(/<button[^>]*>Discard<\/button>/)
    expect(html).toContain('aria-label="What should change?"')
  })

  test('shows the patch of the picked file, as code', () => {
    const html = show(
      reviewing({
        picked: 'src/a.ts',
        patch: { path: 'src/a.ts', text: '@@ -1 +1 @@\n-old\n+new\n', bytes: 20, truncated: false },
      }),
    )
    expect(html).toMatch(/<pre[^>]*>[^<]*\+new/)
  })

  test('says a patch that was cut was cut', () => {
    const html = show(
      reviewing({
        picked: 'src/a.ts',
        patch: { path: 'src/a.ts', text: '@@', bytes: 99_000, truncated: true },
      }),
    )
    expect(html).toContain('cut short')
  })

  test('counts the files left out of the list', () => {
    const state = reviewing()
    const more = { ...state, diff: { ...state.diff!, more: 4 } }
    expect(show(more)).toContain('4 more files')
  })

  test('after one press of discard it says what is deleted, and offers to keep it', () => {
    const html = show(reviewing({ confirmingDiscard: true }))
    expect(html).toContain('Delete this work?')
    expect(html).toContain('copy of the project')
    expect(html).toContain('2 changed files')
    expect(html).toContain('Your own checkout is not touched')
    expect(html).toMatch(/<button[^>]*>Yes, delete this work<\/button>/)
    expect(html).toMatch(/<button[^>]*>Keep it<\/button>/)
  })

  test('with no confirmation asked the first discard button is plain', () => {
    expect(show(reviewing())).not.toContain('Yes, delete this work')
  })

  test('a refused answer shows its reason in an alert, and the item and its buttons stay', () => {
    const html = show(reviewing({ refusal: 'Your checkout is on `dev`. Switch to `main`, then accept again.' }))
    expect(html).toMatch(/role="alert"[^>]*>[^<]*Your checkout is on/)
    expect(html).toMatch(/<button[^>]*>Accept<\/button>/)
  })

  test('says what an answer did', () => {
    const html = show(stateOf({ ...withBoard, notice: 'Accepted. The work is merged into your checkout.' }))
    expect(html).toMatch(/role="status"[^>]*>[^<]*Accepted\. The work is merged/)
  })

  test('buttons are off while an answer is on its way', () => {
    const html = show(reviewing({ sending: true }))
    expect(html).toMatch(/<button[^>]*disabled[^>]*>Accept<\/button>/)
  })

  test('another item of the Queue is listed, and one with a run can be opened', () => {
    const html = show(
      reviewing({
        items: [reviewItem('q-review', 'r-1', 'scratch:1'), reviewItem('q-other', 'r-2', 'scratch:2')],
      }),
    )
    expect(html).toContain('Also waiting')
    expect(html).toMatch(/<button[^>]*>Open<\/button>/)
  })

  test('an item the page cannot answer says so and offers no answer', () => {
    const resumable = reviewItem('q-c', 'r-1', 'scratch:1', ['resume'])
    const conflict = { ...resumable, kind: 'conflict' as const, title: 'A conflict' }
    const html = show(reviewing({ items: [conflict] }))
    expect(html).toContain('A conflict')
    expect(html).toContain('This page cannot answer this one yet')
    expect(html).not.toMatch(/<button[^>]*>Accept<\/button>/)
  })
})

describe('the two version chips', () => {
  test('in range: both chips, none flagged, with their names', () => {
    const html = show(stateOf({ versions: claude() }))
    expect(html).toContain('2.1.325')
    expect(html).toContain('latest 2.1.330')
    expect(html).toContain('aria-label="Claude version in use: 2.1.325"')
    expect(html).toContain('aria-label="Latest published Claude version: 2.1.330"')
    expect(html).not.toContain('Outside the tested range')
  })

  test('out of range: the in-use chip says so in words', () => {
    const html = show(stateOf({ versions: claude({ in_use: '2.2.0', in_tested_range: false }) }))
    expect(html).toContain('Outside the tested range')
  })

  test('with no version in use the chip says unknown, and is not marked out of range', () => {
    const html = show(stateOf({ versions: claude({ in_use: null, in_tested_range: null, in_use_error: 'not found' }) }))
    expect(html).toContain('unknown')
    expect(html).not.toContain('Outside the tested range')
  })

  test('with no published versions only the in-use chip shows', () => {
    const html = show(stateOf({ versions: claude({ published: null }) }))
    expect(html).toContain('2.1.325')
    expect(html).not.toContain('latest ')
    expect(html).not.toContain('Latest published')
  })

  test('with no entry from the daemon there are no chips', () => {
    expect(show(stateOf({ versions: null }))).not.toContain('Claude version in use')
  })
})

describe('what the chips\' tooltips say', () => {
  test('in range: the tested range', () => {
    const chips = versionChips(claude(), NOW)!
    expect(chips.inUse.tooltip).toBe('Tested from 2.1.300 through 2.1.330.')
    expect(chips.inUse.flag).toBeNull()
  })

  test('out of range with a smoke check that passed: the range and when it passed', () => {
    const at = Date.UTC(2026, 10, 14, 9, 0, 0)
    const chips = versionChips(
      claude({ in_use: '2.2.0', in_tested_range: false, smoke: { passed: true, at_ms: at } }),
      NOW,
    )!
    expect(chips.inUse.flag).toBe('Outside the tested range')
    expect(chips.inUse.tooltip).toBe(
      'This version is outside the range Collide has tested. Tested from 2.1.300 through 2.1.330. ' +
        'The smoke check passed on 2026-11-14.',
    )
  })

  test('out of range with a smoke check that failed: why', () => {
    const chips = versionChips(
      claude({ in_tested_range: false, smoke: { passed: false, at_ms: 0, reason: 'no result event' } }),
      NOW,
    )!
    expect(chips.inUse.tooltip).toContain('The smoke check failed on 1970-01-01: no result event.')
  })

  test('out of range with no smoke check yet: says it has not run', () => {
    const chips = versionChips(claude({ in_tested_range: false }), NOW)!
    expect(chips.inUse.tooltip).toContain('The smoke check has not run on this version yet.')
  })

  test('no version in use: the reason the daemon gave', () => {
    const chips = versionChips(claude({ in_use: null, in_tested_range: null, in_use_error: 'not found on PATH' }), NOW)!
    expect(chips.inUse.text).toBe('unknown')
    expect(chips.inUse.tooltip).toContain('not found on PATH')
  })

  test('the published chip shows latest, with stable in its tooltip and the age of the check', () => {
    const published = { latest: '2.1.330', stable: '2.1.311', fetched_ms: NOW - 3 * 3_600_000 }
    const chips = versionChips(claude({ published }), NOW)!
    expect(chips.published?.text).toBe('latest 2.1.330')
    expect(chips.published?.tooltip).toBe('Stable is 2.1.311. Checked 3 h ago.')
  })

  test('a failed fetch shows the last value and says the last check failed', () => {
    const chips = versionChips(
      claude({ published: { latest: '2.1.330', fetched_ms: NOW - 60_000, error: 'timed out' } }),
      NOW,
    )!
    expect(chips.published?.text).toBe('latest 2.1.330')
    expect(chips.published?.tooltip).toBe('Checked 1 min ago. The last check failed: timed out.')
  })

  test('a fetch that never worked says unknown', () => {
    const chips = versionChips(claude({ published: { error: 'offline' } }), NOW)!
    expect(chips.published?.text).toBe('latest unknown')
  })

  test('no published versions hides the published chip', () => {
    expect(versionChips(claude({ published: null }), NOW)!.published).toBeNull()
  })
})
