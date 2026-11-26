import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { EmptyBoard } from '../src/firstrun/EmptyBoard.tsx'
import { FirstRunPage, type FirstRunPageProps } from '../src/firstrun/FirstRunPage.tsx'
import { GitInitWindow } from '../src/firstrun/GitInitWindow.tsx'
import { ShareBanner, ShareWindow, shareNotice } from '../src/firstrun/Share.tsx'
import { TrustBanner, TrustWindow } from '../src/firstrun/TrustWindow.tsx'
import { canOpen, checkRows, needsGit, parts } from '../src/firstrun/model.ts'
import { makeScratch } from '../src/firstrun/scratch.ts'
import { bringsWords, broughtBy, kindsWords } from '../src/firstrun/trust.ts'
import { createLoop } from '../src/loop/index.ts'
import { PLAN, SCRATCH } from './support/field-world.ts'
import { CHECKS, FirstRunWorld, INSTALL_COMMAND, firstRunWorld, reviewOf } from './support/first-run-world.ts'
import { board } from './support/loop-world.ts'

// The first-run page, the empty board, Share with team and the trust prompt (ts-0194-qp) are functions of their props,
// so they render for every state with no browser. Clicks, the clipboard, the dialogs and the daemon's answers need
// Chrome: `browser/fake-first-run.spec.ts` drives them over the same worlds, and `browser/first-run.spec.ts` over a
// real daemon.

const noop = () => {}

function page(over: Partial<FirstRunPageProps> = {}): string {
  return renderToString(
    <FirstRunPage
      folder="/work/fresh"
      checks={CHECKS.allPass}
      problem={null}
      onFolder={noop}
      onOpen={noop}
      onCheckAgain={noop}
      onMakeRepo={noop}
      {...over}
    />,
  )
}

/** The row of one check, as markup. */
function rowOf(html: string, id: string): string {
  const found = new RegExp(`<li data-check="${id}"[^>]*>.*?</li>`, 's').exec(html)
  expect(found, `a row for ${id}`).not.toBeNull()
  return found![0]
}

describe('the check rows', () => {
  test('all three pass: each is marked passed, and the board can open', () => {
    const rows = checkRows(CHECKS.allPass)
    expect(rows.map((row) => [row.id, row.mark, row.title])).toEqual([
      ['git_repo', 'ok', 'A git repository'],
      ['claude_installed', 'ok', 'Claude Code is installed'],
      ['claude_logged_in', 'ok', 'Claude Code is logged in'],
    ])
    expect(canOpen(CHECKS.allPass)).toBe(true)
  })

  test('not a git repo: Make it one is offered, the board cannot open, and the fix says what git init does', () => {
    const html = page({ checks: CHECKS.notARepo })
    expect(rowOf(html, 'git_repo')).toContain('Not a git repository')
    expect(rowOf(html, 'git_repo')).toContain('Make it one…')
    expect(rowOf(html, 'git_repo')).toContain('changes nothing else')
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Open the board/)
    expect(html).toContain('A folder that is not a git repo cannot open.')
    expect(canOpen(CHECKS.notARepo)).toBe(false)
    expect(needsGit(CHECKS.notARepo)).toBe(true)
  })

  test('claude missing: the fix is the daemon\'s install command with a Copy button, and the board can open', () => {
    const html = page({ checks: CHECKS.noClaude })
    const row = rowOf(html, 'claude_installed')
    expect(row).toContain('Claude Code is not installed')
    expect(row).toContain(`<code class="font-mono [overflow-wrap:anywhere]">${INSTALL_COMMAND}</code>`)
    expect(row).toContain('>Copy</button>')
    expect(html).not.toContain('data-check="claude_logged_in"')
    expect(html).not.toMatch(/<button[^>]*disabled=""[^>]*>Open the board/)
    expect(canOpen(CHECKS.noClaude)).toBe(true)
  })

  test('claude logged out: the fix says to run claude, with its own Copy button, and the board can open', () => {
    const html = page({ checks: CHECKS.loggedOut })
    const row = rowOf(html, 'claude_logged_in')
    expect(row).toContain('Claude Code is not logged in')
    expect(row).toContain('<code class="font-mono [overflow-wrap:anywhere]">claude</code>')
    expect(row).toContain('>Copy</button>')
    expect(row).toContain('You can open the board now; runs need it.')
    expect(rowOf(html, 'claude_installed')).not.toContain('>Copy</button>')
    expect(canOpen(CHECKS.loggedOut)).toBe(true)
  })

  test('a login that could not be checked is shown as neither passed nor failed, with no Copy button', () => {
    const html = page({ checks: CHECKS.couldNotCheck })
    const row = rowOf(html, 'claude_logged_in')
    expect(row).toContain('data-mark="unknown"')
    expect(row).toContain('Could not check that Claude Code is logged in')
    expect(row).not.toContain('>Copy</button>')
    expect(row).not.toContain('Claude Code is not logged in')
    expect(canOpen(CHECKS.couldNotCheck)).toBe(true)
  })

  test('the GitHub row is text that says when it is checked, and has no button', () => {
    const row = rowOf(page(), 'gh_logged_in')
    expect(row).toContain('GitHub')
    expect(row).toContain('Checked when GitHub sources arrive, in Stage 5.')
    expect(row).not.toContain('<button')
  })

  test('a message with backticks splits into plain and code parts', () => {
    expect(parts('Run `claude` in a terminal')).toEqual([
      { text: 'Run ', code: false },
      { text: 'claude', code: true },
      { text: ' in a terminal', code: false },
    ])
  })
})

describe('the page', () => {
  test('names the folder, says what is found in it, and offers Open the board and Check again', () => {
    const html = page()
    expect(html).toContain('Open a project')
    expect(html).toContain('<code class="font-mono [overflow-wrap:anywhere]">/work/fresh</code>')
    expect(html).toContain('Scratch tasks')
    expect(html).toContain('Planning (collide/plan)')
    expect(html).toContain('>Open the board</button>')
    expect(html).toContain('>Check again</button>')
    expect(html).not.toMatch(/<button[^>]*disabled=""[^>]*>Open the board/)
  })

  test('says nothing is written into the folder until Share with team', () => {
    expect(page()).toContain('Nothing is written into the folder until you choose Share with team.')
  })

  test('while the checks are read it says so and cannot open', () => {
    const html = page({ checks: null })
    expect(html).toContain('Checking the folder…')
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Open the board/)
  })

  test('with no folder yet it asks for one and shows no checks', () => {
    const html = page({ folder: null, checks: null })
    expect(html).toContain('Check this folder')
    expect(html).not.toContain('Checks')
    expect(html).not.toContain('Open the board')
  })

  test('has a typed path where there is no picker, and the picker button in the shell', () => {
    expect(page()).toContain('<input')
    expect(page()).not.toContain('Choose a folder…')
    const shell = page({ pickFolder: async () => null })
    expect(shell).toContain('Choose another…')
    expect(shell).not.toContain('<input')
    expect(page({ folder: null, checks: null, pickFolder: async () => null })).toContain('Choose a folder…')
  })

  test('shows the daemon\'s words when the folder could not be opened', () => {
    expect(page({ problem: 'the daemon says no' })).toMatch(/role="alert"[^>]*>the daemon says no</)
  })

  test('the git window says what git init does, in the folder', () => {
    const html = renderToString(<GitInitWindow folder="/work/plain" onRun={async () => null} onClosed={noop} />)
    expect(html).toContain('Make this folder a git repository?')
    expect(html).toContain('<code>git init</code>')
    expect(html).toContain('/work/plain')
    expect(html).toContain('changes nothing else')
    expect(html).toContain('Run git init')
  })
})

describe('the empty board', () => {
  const empty = (over: Partial<Parameters<typeof EmptyBoard>[0]> = {}) =>
    renderToString(
      <EmptyBoard
        folder="/work/alpha"
        sources={[SCRATCH, PLAN]}
        onScratch={async () => null}
        onAddPack={noop}
        {...over}
      />,
    )

  test('says the board is empty and which folder opened with the built-in packs', () => {
    const html = empty()
    expect(html).toContain('Nothing on this board yet')
    expect(html).toContain('<b>/work/alpha</b>')
    expect(html).toContain('with the built-in packs')
  })

  test('puts Plan it first first, with its 9d tag, and its button off and saying it arrives in Stage 9d', () => {
    const html = empty()
    expect(html.indexOf('Plan it first')).toBeGreaterThan(-1)
    expect(html.indexOf('Plan it first')).toBeLessThan(html.indexOf('Start with a task'))
    expect(html).toContain('>9d</span>')
    expect(html).toMatch(
      /<button[^>]*disabled=""[^>]*title="Planning sessions arrive in Stage 9d"[^>]*>Start a planning session/,
    )
    expect(html).toContain('Arrives in Stage 9d.')
  })

  test('offers Start with a task, which is off until something is written', () => {
    const html = empty()
    expect(html).toContain('A scratch task stays on your machine until you share it.')
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Add as a scratch task/)
  })

  test('lists the sources Collide found, with the pack of each', () => {
    const html = empty()
    expect(html).toContain('Sources Collide found')
    expect(html).toContain('scratch')
    expect(html).toContain('plan')
    expect(html).toContain(' · collide/plan')
  })

  test('says its sources are being read when none is listed yet', () => {
    expect(empty({ sources: [] })).toContain('Reading the sources…')
  })

  test('offers Add a pack, and says GitHub arrives in Stage 5', () => {
    const html = empty()
    expect(html).toContain('>Add a pack…</button>')
    expect(html).toContain('GitHub arrives in Stage 5.')
  })

  test('a scratch task is made through task.apply on the scratch source, with the words trimmed', async () => {
    const world = new FirstRunWorld()
    world.boards = [board('b-1', '/work/one')]
    expect(await makeScratch(world.owner, 'b-1', '  fix the sign-in page  ')).toBeNull()
    expect(world.callsTo('task.apply')).toEqual([
      { board: 'b-1', source: 'scratch', apply: { op: 'create', args: { prompt: 'fix the sign-in page' } } },
    ])
    expect(world.listing.map((each) => each.title)).toEqual(['fix the sign-in page'])
  })

  test('a scratch task the daemon refuses gives its words, and no words make no call', async () => {
    const world = new FirstRunWorld()
    world.refuseCreate = 'the scratch file is read only'
    expect(await makeScratch(world.owner, 'b-1', 'x')).toBe('the scratch file is read only')
    expect(await makeScratch(world.owner, 'b-1', '   ')).toBe('Say what needs doing first.')
    expect(world.callsTo('task.apply')).toHaveLength(1)
  })
})

describe('the Share with team banner', () => {
  test('is offered only when the views are read and the line has no Collide folder', () => {
    const notice = (sharedRead: boolean, shared: boolean, written = false) =>
      shareNotice({ sharedRead, shared, written })
    expect(notice(true, false)).toBe('offer')
    expect(notice(true, true)).toBe('none')
    expect(notice(false, false)).toBe('none')
  })

  test('says the files wait for a commit once they are written, until the line holds them', () => {
    expect(shareNotice({ sharedRead: true, shared: false, written: true })).toBe('written')
    expect(shareNotice({ sharedRead: true, shared: true, written: true })).toBe('none')
  })

  test('the offer says the settings are yours only and has the button', () => {
    const html = renderToString(<ShareBanner notice="offer" onShare={noop} />)
    expect(html).toContain('This board&#x27;s settings are yours only.')
    expect(html).toContain('<code>.collide/</code>')
    expect(html).toContain('>Share with team</button>')
  })

  test('the written notice asks for a commit and has no button', () => {
    const html = renderToString(<ShareBanner notice="written" onShare={noop} />)
    expect(html).toContain('Commit it')
    expect(html).not.toContain('<button')
  })

  test('no notice draws nothing', () => {
    expect(renderToString(<ShareBanner notice="none" onShare={noop} />)).toBe('')
  })

  test('the window asks before it writes, and says nothing is committed', () => {
    const world = new FirstRunWorld()
    const html = renderToString(<ShareWindow owner={world.owner} boardId="b-1" onWritten={noop} onClosed={noop} />)
    expect(html).toContain('Share this board with your team?')
    expect(html).toContain('>Not now</button>')
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Write \.collide\//)
  })
})

describe('what the repo brings', () => {
  test('a pack, two views and settings, in words', () => {
    const review = reviewOf({ packs: ['repo:.collide/packs/tracker'], views: ['mine', 'team'], settings: true })
    const brought = broughtBy(review)
    expect(brought).toEqual({ packs: ['repo:.collide/packs/tracker'], views: ['mine', 'team'], settings: true })
    expect(bringsWords(brought!)).toBe('one pack of its own, two views and its settings')
    expect(kindsWords(brought!)).toBe('packs, views and settings')
  })

  test('two packs and nothing else', () => {
    const brought = broughtBy(reviewOf({ packs: ['repo:a', 'repo:b'] }))!
    expect(bringsWords(brought)).toBe('two packs of its own')
    expect(kindsWords(brought)).toBe('packs')
  })

  test('one view alone, and settings alone', () => {
    expect(bringsWords(broughtBy(reviewOf({ views: ['team'] }))!)).toBe('one view')
    expect(bringsWords(broughtBy(reviewOf({ settings: true }))!)).toBe('its settings')
  })

  test('a board file that lists no pack brings nothing that waits on trust', () => {
    expect(broughtBy(reviewOf({}))).toBeNull()
  })

  test('a view or settings the line no longer has are not brought', () => {
    const review = reviewOf({})
    review.changes = [
      { name: 'views/old.yaml', kind: 'removed' },
      { name: 'config.yaml', kind: 'removed' },
    ]
    expect(broughtBy(review)).toBeNull()
  })
})

describe('the trust window', () => {
  const review = reviewOf({ packs: ['repo:.collide/packs/tracker'], views: ['mine', 'team'], settings: true })
  const brought = broughtBy(review)!
  // The server's markup separates the pieces of a sentence with comments, which no person sees.
  const html = renderToString(
    <TrustWindow boardName="pantry" review={review} brought={brought} onTrust={async () => null} onWithout={noop} />,
  ).replaceAll('<!-- -->', '')

  test('says what the repo brings, by name', () => {
    expect(html).toContain('Trust the packs, views and settings this repo brings?')
    expect(html).toContain('<b>pantry</b> brings one pack of its own, two views and its settings.')
  })

  test('says a pack runs code on this Mac, and shows where it is from and what it runs', () => {
    expect(html).toContain('A pack runs code on this Mac, with your permissions, to read its tasks.')
    expect(html).toContain('<code>repo:.collide/packs/tracker</code>')
    expect(html).toContain('providers/tasks.mjs')
    expect(html).toContain('tracker.example')
  })

  test('names the views and the settings file', () => {
    expect(html).toContain('<code>mine</code>')
    expect(html).toContain('<code>team</code>')
    expect(html).toContain('.collide/config.yaml')
  })

  test('says the built-in packs need no trust, and offers both ways on', () => {
    expect(html).toContain('The built-in packs need no trust.')
    expect(html).toContain('>Open without it</button>')
    expect(html).toContain('>Trust and open</button>')
  })

  test('a repo that brings only views does not talk of packs running code', () => {
    const only = reviewOf({ views: ['team'] })
    const text = renderToString(
      <TrustWindow
        boardName="pantry"
        review={only}
        brought={broughtBy(only)!}
        onTrust={async () => null}
        onWithout={noop}
      />,
    )
    expect(text).toContain('Trust the views this repo brings?')
    expect(text).not.toContain('runs code')
  })

  test('the daemon\'s problems with the line are shown', () => {
    const problem = { ...review, problems: ['packs.lock is out of date'] }
    const text = renderToString(
      <TrustWindow boardName="pantry" review={problem} brought={brought} onTrust={async () => null} onWithout={noop} />,
    )
    expect(text).toContain('packs.lock is out of date')
  })

  test('the banner names what is not applied and offers the review again', () => {
    const text = renderToString(<TrustBanner brought={brought} onReview={noop} />).replaceAll('<!-- -->', '')
    expect(text).toContain('Not applied: one pack of its own, two views and its settings.')
    expect(text).toContain('The built-in packs run either way.')
    expect(text).toContain('Review and trust…')
  })
})

describe('opening a folder', () => {
  async function loopOf(world: FirstRunWorld) {
    const loop = createLoop(world.owner)
    await loop.idle()
    return loop
  }

  test('says it opened, and the board is chosen', async () => {
    const world = firstRunWorld()
    const loop = await loopOf(world)
    const outcome = await loop.openChecked('/work/fresh')
    expect(outcome).toEqual({ opened: true, problems: [] })
    expect(loop.getState().boardId).toBe('b-fresh')
    expect(world.callsTo('board.open')).toEqual([{ root: '/work/fresh' }])
  })

  test('a folder that is not a repo does not open, and gives the daemon\'s words', async () => {
    const world = firstRunWorld()
    const loop = await loopOf(world)
    const outcome = await loop.openChecked('/work/plain')
    expect(outcome.opened).toBe(false)
    expect(outcome.problems.join(' ')).toContain('not a git repository')
    expect(loop.getState().boardId).toBeNull()
  })

  test('with init_git it makes the folder a repository and opens it', async () => {
    const world = firstRunWorld()
    const loop = await loopOf(world)
    const outcome = await loop.openChecked('/work/plain', true)
    expect(outcome.opened).toBe(true)
    expect(world.callsTo('board.open')).toEqual([{ root: '/work/plain', init_git: true }])
    expect(loop.getState().boards.map((each) => each.root)).toEqual(['/work/plain'])
  })

  test('a blank folder makes no call', async () => {
    const world = firstRunWorld()
    const loop = await loopOf(world)
    expect(await loop.openChecked('  ')).toEqual({ opened: false, problems: [] })
    expect(world.callsTo('board.open')).toEqual([])
  })
})

describe('the world\'s checks are the daemon\'s', () => {
  test('board.checks gives what the folder is set to, and a repo passes by default', async () => {
    const world = firstRunWorld()
    const loggedOut = await world.owner.rpc('board.checks', { root: '/work/loggedout' })
    expect(loggedOut.checks.map((check) => check.ok)).toEqual([true, true, false])
    world.folders.set('/work/other', board('b-other', '/work/other'))
    const other = await world.owner.rpc('board.checks', { root: '/work/other' })
    expect(other.checks.every((check) => check.ok)).toBe(true)
  })
})
