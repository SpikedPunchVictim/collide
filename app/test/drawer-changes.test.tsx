import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { DrawerView, type DrawerViewProps } from '../src/drawer/DrawerView.tsx'
import { partsOf } from '../src/drawer/model.ts'
import { loadField } from '../src/field/load.ts'
import { changesWorld } from './support/changes-world.ts'
import { type FieldWorld, PLAN, drawerWorld } from './support/field-world.ts'

// The Changes part of the drawer, drawn to text from a scripted world (ts-0189-fg; note 17): a live run, a finished
// one, a commit made by hand and nothing at all, each as `task.changes` gives it. What a click does, and the diff
// window it opens, are in `browser/fake-diff.spec.ts`.

const noop = () => {}

/** The drawer for a task of the world, with what `task.changes` gave for it unless `over` says otherwise. */
async function drawer(world: FieldWorld, key: string, over: Partial<DrawerViewProps> = {}): Promise<string> {
  const data = await loadField(world.owner, 'b-alpha', null)
  const shown = data.tasks.find((each) => each.key === key)!
  const got = await world.owner.rpc('task.get', { board: 'b-alpha', id: key })
  const changes = await world.owner.rpc('task.changes', { board: 'b-alpha', task: key })
  return renderToString(
    <DrawerView
      shown={shown}
      got={got}
      problem={null}
      data={data}
      board={{ name: 'alpha', line: 'main' }}
      shut={[]}
      changes={{ changes, problem: null }}
      onFlip={noop}
      onOpenTask={noop}
      onOpenFile={noop}
      onClose={noop}
      {...over}
    />,
  )
}

/** The part's own markup, up to the next part's. */
function changesPart(html: string): string {
  const from = html.indexOf('data-part="changes"')
  expect(from, 'the drawer has a Changes part').toBeGreaterThan(-1)
  const next = html.indexOf('data-part=', from + 1)
  return html.slice(from, next === -1 ? undefined : next)
}

const partsOfHtml = (html: string) => [...html.matchAll(/data-part="([^"]+)"/g)].map((each) => each[1]!)
const text = (html: string) =>
  html
    .replace(/<[^>]+>/g, ' ')
    .replace(/&#x27;/g, "'")
    .replace(/\s+/g, ' ')

describe('where Changes sits', () => {
  test('after Part of and before Fields from the source, in 0.11 order', async () => {
    const world = drawerWorld()
    const fields = [{ id: 'goal_text', type: 'string' as const, filterable: false }]
    world.sources = [{ ...PLAN, fields }, ...world.sources.slice(1)]
    world.edit('plan:ts-0002-bb', { fields: { goal_text: 'Make the board' } })
    world.changes.clear()
    const html = await drawer(world, 'plan:ts-0002-bb')
    expect(partsOfHtml(html)).toEqual([
      'description',
      'waits',
      'unblocks',
      'part-of',
      'changes',
      'fields',
      'copies',
      'details',
    ])
  })

  test('a plan task has the part in partsOf, titled Changes', async () => {
    const world = drawerWorld()
    const data = await loadField(world.owner, 'b-alpha', null)
    const shown = data.tasks.find((each) => each.key === 'plan:ts-0002-bb')!
    const got = await world.owner.rpc('task.get', { board: 'b-alpha', id: 'plan:ts-0002-bb' })
    const parts = partsOf({ shown, got, data, board: null })
    expect(parts.find((part) => part.id === 'changes')?.title).toBe('Changes')
  })

  test('a scratch task has it too, since a run on it has changes', async () => {
    const html = await drawer(drawerWorld(), 'scratch:1')
    expect(partsOfHtml(html)).toContain('changes')
  })

  test('a key two tasks share has none: the daemon names no task to ask for', async () => {
    const world = drawerWorld()
    const html = await drawer(world, 'plan:ts-0009-ii')
    expect(partsOfHtml(html)).not.toContain('changes')
  })

  test('a folded Changes shows its bar and nothing else of it', async () => {
    const html = await drawer(changesWorld(), 'plan:ts-0001-aa', { shut: ['changes'] })
    expect(changesPart(html)).not.toContain('Run r-12')
    expect(changesPart(html)).toContain('Changes')
  })
})

describe('a run that is live', () => {
  const part = async () => changesPart(await drawer(changesWorld(), 'plan:ts-0001-aa'))

  test('names the run and its state, and says it is live', async () => {
    const words = text(await part())
    expect(words).toContain('Run r-12 · Working')
    expect(words).toContain('Live, as the agent edits')
  })

  test('says where it ran and how, in the words the daemon gives', async () => {
    expect(text(await part())).toContain(
      "Run r-12's worktree, since the agent phase began, including edits not committed yet.",
    )
  })

  test('lists each file, with its added and removed lines', async () => {
    const html = await part()
    expect(html).toMatch(/data-file="src\/parser\.ts"/)
    expect(text(html)).toMatch(/src\/parser\.ts \+30 −4/)
    expect(text(html)).toMatch(/docs\/guide\.md \+12 −0/)
  })

  test('says how many files, and the lines, of the run', async () => {
    expect(text(await part())).toContain('6 files, +42 −4 in the 3 listed')
  })

  test('a binary file has no counts, and says so', async () => {
    expect(text(await part())).toMatch(/assets\/logo\.png binary file/)
  })

  test('marks a new file as new', async () => {
    expect(text(await part())).toMatch(/docs\/guide\.md \+12 −0 new/)
  })

  test('says the files it left out, and the folder it collapsed with its size', async () => {
    const words = text(await part())
    expect(words).toContain('3 more files are not listed.')
    expect(words).toContain('node_modules: 1203 files, 4.3 MB, not listed')
  })

  test('each file is a button that opens its diff', async () => {
    const html = await part()
    expect(html).toMatch(/<button[^>]*data-file="src\/parser\.ts"[^>]*title="Open the diff"/)
  })

  test('names the run only: there is no link to Tracks yet', async () => {
    const html = await part()
    expect(html).not.toContain('Tracks')
    expect(html).not.toMatch(/<a[ >]/)
  })
})

describe('a run that is over', () => {
  const part = async () => changesPart(await drawer(changesWorld(), 'plan:ts-0001-aa'))

  test('names the run and how it ended, and is not live', async () => {
    const html = await part()
    const done = html.slice(html.indexOf('data-run="r-9"'))
    expect(text(done)).toContain('Run r-9 · Finished: done')
    expect(done).not.toContain('Live, as the agent edits')
  })

  test('says where it ran and how, from where the phase began to where it ended', async () => {
    expect(text(await part())).toContain("Run r-9's worktree, from where the agent phase began to where it ended.")
  })

  test('lists its files', async () => {
    expect(text(await part())).toMatch(/src\/lexer\.ts \+10 −2/)
  })

  test('the live run comes first, as the daemon lists them, newest first', async () => {
    const html = await part()
    expect(html.indexOf('data-run="r-12"')).toBeLessThan(html.indexOf('data-run="r-9"'))
  })
})

describe('a run in the person\'s checkout', () => {
  test('says so in the words of the wire: the edits include the person\'s own', async () => {
    const html = await drawer(changesWorld(), 'plan:ts-0004-dd')
    expect(text(changesPart(html))).toContain(
      'Your checkout, since the agent phase began, including edits not committed yet and your own edits.',
    )
  })
})

describe('a run from before heads were kept', () => {
  test('lists no files and shows the wire\'s note as it comes', async () => {
    const html = changesPart(await drawer(changesWorld(), 'plan:ts-0002-bb'))
    expect(text(html)).toContain('Run r-3 · Finished: done')
    expect(text(html)).toContain('no changes recorded')
    expect(html).not.toContain('data-file=')
    expect(text(html)).not.toContain('No changes yet.')
  })
})

describe('a commit made by hand', () => {
  const part = async () => changesPart(await drawer(changesWorld(), 'plan:ts-0001-aa'))

  test('is listed with its subject, its short id and its files', async () => {
    const words = text(await part())
    expect(words).toContain('Fix the typo that ts-0001-aa left in the README')
    expect(words).toContain('abc1234')
    expect(words).toMatch(/README\.md \+1 −1/)
  })

  test('says where the commits are and how they were found, in the wire\'s words', async () => {
    expect(text(await part())).toContain('Your checkout, line main, 1 commit that names ts-0001-aa: abc1234.')
  })

  test('its files open the diff of the commit', async () => {
    const html = await part()
    expect(html).toMatch(/data-commit="abc1234def5678abc1234def5678abc1234def56"/)
    expect(html).toMatch(/<button[^>]*data-file="README\.md"/)
  })

  test('comes after the runs', async () => {
    const html = await part()
    expect(html.indexOf('data-run="r-9"')).toBeLessThan(html.indexOf('data-commit='))
  })

  test('a task with a commit and no run lists the commit and says no run built it', async () => {
    const html = changesPart(await drawer(changesWorld(), 'plan:ts-0005-ee'))
    expect(html).not.toContain('data-run=')
    expect(text(html)).toContain('Draw the drawer ts-0005-ee')
    expect(text(html)).toContain('No run built it.')
    expect(text(html)).not.toContain('No changes yet.')
  })
})

describe('a task with no change', () => {
  test('says "No changes yet." and lists nothing', async () => {
    const html = changesPart(await drawer(changesWorld(), 'plan:ts-0003-cc'))
    expect(text(html)).toContain('No changes yet.')
    expect(html).not.toContain('data-run=')
    expect(html).not.toContain('data-commit=')
  })
})

describe('before and after a read', () => {
  test('says it is reading before the first read of the changes', async () => {
    const html = await drawer(changesWorld(), 'plan:ts-0001-aa', { changes: null })
    expect(text(changesPart(html))).toContain('Reading the changes…')
  })

  test('says why when the first read failed', async () => {
    const html = await drawer(changesWorld(), 'plan:ts-0001-aa', { changes: { changes: null, problem: 'git is busy' } })
    expect(text(changesPart(html))).toContain('The changes could not be read: git is busy')
    expect(changesPart(html)).toContain('role="alert"')
  })

  test('keeps what was read when a later read failed, and says why', async () => {
    const world = changesWorld()
    const changes = await world.owner.rpc('task.changes', { board: 'b-alpha', task: 'plan:ts-0001-aa' })
    const html = await drawer(world, 'plan:ts-0001-aa', { changes: { changes, problem: 'git is busy' } })
    expect(text(changesPart(html))).toContain('The changes could not be read: git is busy')
    expect(text(changesPart(html))).toContain('Run r-12 · Working')
  })
})
