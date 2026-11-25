import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { DrawerView, type DrawerViewProps } from '../src/drawer/DrawerView.tsx'
import { STARTS_SHUT } from '../src/drawer/folds.ts'
import { blockingLinks, partsOf } from '../src/drawer/model.ts'
import { loadField } from '../src/field/load.ts'
import {
  type FieldWorld,
  PLAN,
  SCRATCH,
  TRACKER,
  drawerWorld,
  task,
  view,
} from './support/field-world.ts'
import { HOSTILE_BODY } from './support/hostile.ts'

// The task drawer, drawn to text over the scripted worlds (ts-0184-hv). The same wire the daemon gives: `task.list`
// for the Field and `task.get` for the task. What a click, a key and a drag do is in `browser/fake-drawer.spec.ts`.

const noop = () => {}

/** Which parts are drawn, in order, by the `data-part` of each. */
function partsOfHtml(html: string): string[] {
  return [...html.matchAll(/data-part="([^"]+)"/g)].map((each) => each[1]!)
}

/** The drawer for the first listed task with `key`, with `shut` as its folded parts (none by default). */
async function drawer(world: FieldWorld, key: string, over: Partial<DrawerViewProps> = {}): Promise<string> {
  const data = await loadField(world.owner, 'b-alpha', null)
  const shown = data.tasks.find((each) => each.key === key)!
  const got = await world.owner.rpc('task.get', { board: 'b-alpha', id: key })
  return renderToString(
    <DrawerView
      shown={shown}
      got={got}
      problem={null}
      data={data}
      board={{ name: 'alpha', line: 'main' }}
      shut={[]}
      onFlip={noop}
      onOpenTask={noop}
      onClose={noop}
      {...over}
    />,
  )
}

/** A plan that declares one field, so that every part has something to say. */
function planWithField(): FieldWorld {
  const world = drawerWorld()
  world.sources = [{ ...PLAN, fields: [{ id: 'goal_text', type: 'string', filterable: false }] }, SCRATCH, TRACKER]
  world.edit('plan:ts-0002-bb', { fields: { goal_text: 'Make the board' } })
  return world
}

describe('the drawer of a plan task', () => {
  test('shows the parts the source has something for, in 0.11 order', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0002-bb')
    expect(partsOfHtml(html)).toEqual(['description', 'waits', 'unblocks', 'part-of', 'copies', 'details'])
  })

  test('shows every part, Fields from the source among them, in 0.11 order', async () => {
    const html = await drawer(planWithField(), 'plan:ts-0002-bb')
    expect(partsOfHtml(html)).toEqual(['description', 'waits', 'unblocks', 'part-of', 'fields', 'copies', 'details'])
  })

  test('names each part on its bar', async () => {
    const html = await drawer(planWithField(), 'plan:ts-0002-bb')
    const names = [...html.matchAll(/<h3[^>]*><button[^>]*>[^]*?<span[^>]*>([^<]+)<\/span>/g)].map((each) => each[1])
    expect(names).toEqual([
      'Description',
      'Waits on',
      'Unblocks',
      'Part of',
      'Fields from plan',
      'In each copy of the project',
      'Details for engineers',
    ])
  })

  test('opens with the status chip, its category in words, the id and Close, then the title', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0003-cc')
    expect(html).toMatch(/aria-label="Task drawer"/)
    const header = html.slice(0, html.indexOf('data-part='))
    expect(header).toContain('active')
    expect(header).toContain('being worked on')
    expect(header).toContain('ts-0003-cc')
    expect(header).toContain('>Close<')
    expect(header).toMatch(/<h2[^>]*>Build the Field<\/h2>/)
    expect(header.indexOf('being worked on')).toBeLessThan(header.indexOf('>Close<'))
    expect(header.indexOf('>Close<')).toBeLessThan(header.indexOf('<h2'))
  })

  test('says it is read only, and that the file or an agent changes it', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0003-cc')
    expect(html).toContain('Read only. Edit the file, or ask an agent; the board follows it.')
  })

  test('shows no form, no question box and no Discuss button', async () => {
    const html = await drawer(planWithField(), 'plan:ts-0002-bb')
    expect(html).not.toMatch(/<form|<input|<textarea|<select/)
    expect(html).not.toContain('Discuss with the Desk')
    expect(html).not.toContain('Questions')
  })

  test('draws the description through Markdown', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0001-aa')
    expect(html).toContain('<strong>files</strong>')
    expect(html).toContain('<li>two</li>')
    expect(html).toMatch(/<h4>Goal<\/h4>/)
  })

  test('leaves the description out when the task has no body', async () => {
    const world = drawerWorld()
    world.edit('plan:ts-0004-dd', { body: null })
    expect(partsOfHtml(await drawer(world, 'plan:ts-0004-dd'))).not.toContain('description')
  })

  test('shows Waits on with each task it waits on, its state and its title', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0002-bb')
    const waits = html.slice(html.indexOf('data-part="waits"'), html.indexOf('data-part="unblocks"'))
    expect(waits).toContain('Write the parser')
    expect(waits).toContain('ts-0001-aa')
    expect(waits).toContain('ready')
    expect(waits).toContain('data-open="plan:ts-0001-aa"')
  })

  test('shows Unblocks with each task that waits on this one', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0002-bb')
    const unblocks = html.slice(html.indexOf('data-part="unblocks"'), html.indexOf('data-part="part-of"'))
    expect(unblocks).toContain('Build the Field')
    expect(unblocks).toContain('active')
  })

  test('says so when nothing is waited on, and when nothing waits', async () => {
    const first = await drawer(drawerWorld(), 'plan:ts-0001-aa')
    expect(first).toContain('Nothing. It can start.')
    expect(first).not.toContain('Nothing waits on it.')
    const last = await drawer(drawerWorld(), 'plan:ts-0003-cc')
    expect(last).toContain('Nothing waits on it.')
    expect(last).not.toContain('Nothing. It can start.')
  })

  test('shows a link to a key that is no task as text, with no way to open it', async () => {
    const world = drawerWorld()
    world.edit('plan:ts-0004-dd', { links: [{ type: 'blocked-by', to: 'ts-9999-zz' }] })
    const html = await drawer(world, 'plan:ts-0004-dd')
    const waits = html.slice(html.indexOf('data-part="waits"'), html.indexOf('data-part="unblocks"'))
    expect(waits).toContain('plan:ts-9999-zz')
    expect(waits).toContain('not a task')
    expect(waits).not.toContain('data-open')
  })

  test('shows Part of with the epic by its title, the collections and a chip for each view that holds it', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0001-aa')
    const part = html.slice(html.indexOf('data-part="part-of"'), html.indexOf('data-part="copies"'))
    expect(part).toContain('The first epic')
    expect(part).toContain('stage-6a')
    expect(part).toContain('view: ready')
    expect(part).toContain('view: mine')
  })

  test('names the epic by its link when the epic is no task of the board', async () => {
    const world = drawerWorld()
    world.listing = world.listing.filter((each) => each.key !== 'plan:ep-001-aa')
    const html = await drawer(world, 'plan:ts-0002-bb')
    expect(html.slice(html.indexOf('data-part="part-of"'))).toContain('ep-001-aa')
  })

  test('leaves Part of out when the task is part of nothing, in no collection and in no view', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0003-cc')
    expect(partsOfHtml(html)).not.toContain('part-of')
  })

  test('shows Part of for a task that is in a view and part of nothing else', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0004-dd')
    expect(html.slice(html.indexOf('data-part="part-of"'))).toContain('view: ready')
  })

  test('shows one row for each copy, saying which is on the board and which differ', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0004-dd')
    const copies = html.slice(html.indexOf('data-part="copies"'), html.indexOf('data-part="details"'))
    expect(copies.match(/<tr[ >]/g)).toHaveLength(4)
    expect(copies).toContain('shown on the board')
    expect(copies).toContain('differs from main')
    expect(copies).toContain('same as main')
    expect(copies).toContain('Run r-121')
    expect(copies).toContain('wt-a')
  })

  test('shows the state of each copy, in words', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0004-dd')
    const copies = html.slice(html.indexOf('data-part="copies"'))
    expect(copies.match(/>\s*review\s*</g)).toHaveLength(1)
    expect(copies.match(/>\s*ready\s*</g)).toHaveLength(2)
  })
})

describe('the drawer of a collided id', () => {
  test('shows both copies, and says the other is another task, with its title and path', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0009-ii')
    const copies = html.slice(html.indexOf('data-part="copies"'), html.indexOf('data-part="details"'))
    expect(copies.match(/<tr[ >]/g)).toHaveLength(3)
    expect(copies).toContain('shown on the board')
    expect(copies).toContain('Same id, another task')
    expect(copies).toContain('Same id, another place')
    expect(copies).toContain('b/ts-0009-ii.md')
  })

  test('never says a copy differs from main', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0009-ii')
    expect(html).not.toContain('differs from main')
    expect(html).not.toContain('same as main')
  })

  test('still shows the task as the board lists it, though the daemon names no task for an ambiguous key', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0009-ii')
    expect(html).toMatch(/<h2[^>]*>Same id, one place<\/h2>/)
    expect(partsOfHtml(html)).not.toContain('description')
  })

  test('shows the copy of the other entry as the other task', async () => {
    const data = await loadField(drawerWorld().owner, 'b-alpha', null)
    const second = data.tasks.filter((each) => each.key === 'plan:ts-0009-ii')[1]!
    const world = drawerWorld()
    const got = await world.owner.rpc('task.get', { board: 'b-alpha', id: second.key })
    const html = renderToString(
      <DrawerView
        shown={second}
        got={got}
        problem={null}
        data={data}
        board={null}
        shut={[]}
        onFlip={noop}
        onOpenTask={noop}
        onClose={noop}
      />,
    )
    expect(html).toContain('Same id, one place')
    expect(html).toContain('a/ts-0009-ii.md')
  })
})

describe('the drawer of a task of another source', () => {
  test('a scratch task shows no Waits on and no Unblocks, and no copies, which are not in the repo', async () => {
    const html = await drawer(drawerWorld(), 'scratch:1')
    expect(partsOfHtml(html)).toEqual(['description', 'details'])
    expect(html).not.toContain('Nothing. It can start.')
    expect(html).not.toContain('Nothing waits on it.')
  })

  test('says to change it in the source', async () => {
    const html = await drawer(drawerWorld(), 'scratch:1')
    expect(html).toContain('Read only. Change it in scratch.')
    expect(html).not.toContain('Edit the file')
  })

  test('shows a field by its declared label, and by its id in words when it declares none', async () => {
    const html = await drawer(drawerWorld(), 'tracker:12')
    const fields = html.slice(html.indexOf('data-part="fields"'), html.indexOf('data-part="details"'))
    expect(fields).toContain('Priority level')
    expect(fields).not.toContain('>Priority<')
    expect(fields).toContain('Story points')
    expect(fields).not.toContain('story_points')
    expect(fields).toMatch(/Priority level[^]*?high/)
  })

  test('says "not set" for a field that has no value, or an empty one', async () => {
    const html = await drawer(drawerWorld(), 'tracker:12')
    const fields = html.slice(html.indexOf('data-part="fields"'), html.indexOf('data-part="details"'))
    expect(fields.match(/not set/g)).toHaveLength(2)
    expect(fields).toMatch(/Story points[^]*?not set/)
    expect(fields).toMatch(/Component[^]*?not set/)
  })

  test('says which pack declares the fields', async () => {
    const html = await drawer(drawerWorld(), 'tracker:12')
    expect(html).toContain('Declared by demo/tracker.')
  })

  test('shows Waits on and Unblocks for a source that blocks, and no copies for a tracker', async () => {
    const html = await drawer(drawerWorld(), 'tracker:12')
    expect(partsOfHtml(html)).toEqual(['description', 'waits', 'unblocks', 'fields', 'details'])
    expect(html).toContain('Read only. Change it in tracker.')
  })

  test('shows a value that is not text as JSON, and text as it is', async () => {
    const world = drawerWorld()
    world.edit('tracker:12', { fields: { priority: 3, story_points: [1, 2], component: 'a <b>b</b>' } })
    const html = await drawer(world, 'tracker:12')
    expect(html).toContain('>3<')
    expect(html).toContain('[1,2]')
    expect(html).toContain('a &lt;b&gt;b&lt;/b&gt;')
  })
})

describe('the details for engineers', () => {
  test('give the source and pack, the file, how copies work, the board and line, and the raw fields', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0001-aa')
    const details = html.slice(html.indexOf('data-part="details"'))
    expect(details).toContain('plan · collide/plan')
    expect(details).toContain('.collide/planning/epics/ep-001-aa/tasks/ts-0001-aa.md')
    expect(details).toContain('one per checkout: it lives in the repo')
    expect(details).toContain('alpha · line main')
    expect(details).toContain('&quot;key&quot;: &quot;plan:ts-0001-aa&quot;')
    expect(details).toContain('&quot;category&quot;: &quot;ready&quot;')
    expect(details).toContain('&quot;links&quot;')
  })

  test('say that a task outside the repo has one copy for every checkout, and scratch is built in', async () => {
    const html = await drawer(drawerWorld(), 'scratch:1')
    const details = html.slice(html.indexOf('data-part="details"'))
    expect(details).toContain('scratch · built in')
    expect(details).toContain('one for every checkout: it lives outside the repo')
  })

  test('show the task as the source gave it, fields among them', async () => {
    const html = await drawer(drawerWorld(), 'tracker:12')
    const details = html.slice(html.indexOf('data-part="details"'))
    expect(details).toContain('&quot;priority&quot;: &quot;high&quot;')
  })
})

describe('the folds of the parts', () => {
  test('a folded part shows its bar and none of its content, and says it is folded', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0001-aa', { shut: ['description', 'waits'] })
    expect(html).not.toContain('<strong>files</strong>')
    expect(html).not.toContain('Nothing. It can start.')
    expect(html).toContain('Design the board')
    expect(html).toMatch(/aria-expanded="false"[^>]*>[^]*?Description/)
    expect(partsOfHtml(html)).toContain('description')
  })

  test('an open part says it is open', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0001-aa')
    expect(html).toMatch(/aria-expanded="true"/)
    expect(html).not.toMatch(/aria-expanded="false"/)
  })

  test('Details for engineers starts folded, and nothing else does', async () => {
    expect(STARTS_SHUT).toEqual(['details'])
  })
})

describe('the drawer while it waits, and when it cannot read', () => {
  test('says it is reading, under the header, before the task has come', async () => {
    const world = drawerWorld()
    const data = await loadField(world.owner, 'b-alpha', null)
    const shown = data.tasks.find((each) => each.key === 'plan:ts-0001-aa')!
    const html = renderToString(
      <DrawerView
        shown={shown}
        got={null}
        problem={null}
        data={data}
        board={null}
        shut={[]}
        onFlip={noop}
        onOpenTask={noop}
        onClose={noop}
      />,
    )
    expect(html).toContain('Reading the task…')
    expect(html).toMatch(/<h2[^>]*>Write the parser<\/h2>/)
    expect(partsOfHtml(html)).toEqual([])
  })

  test('says why it could not read, as an alert', async () => {
    const world = drawerWorld()
    const data = await loadField(world.owner, 'b-alpha', null)
    const shown = data.tasks.find((each) => each.key === 'plan:ts-0001-aa')!
    const html = renderToString(
      <DrawerView
        shown={shown}
        got={null}
        problem="the daemon is gone"
        data={data}
        board={null}
        shut={[]}
        onFlip={noop}
        onOpenTask={noop}
        onClose={noop}
      />,
    )
    expect(html).toMatch(/role="alert"[^>]*>The task could not be read: the daemon is gone</)
  })
})

describe('text from the task is text', () => {
  test('the hostile body shows as text: no script, no image, no link that runs', async () => {
    const html = await drawer(drawerWorld(), 'plan:ts-0066-hh')
    expect(html).not.toMatch(/<script|<img|<a[ >]/i)
    expect(html).toContain('&lt;script&gt;')
    expect(html).toContain('link not opened')
  })

  test('a title, a field and a link title that hold markup are shown as text', async () => {
    const world = drawerWorld()
    const mark = '<img src=x onerror=alert(1)>'
    world.edit('tracker:12', { title: mark, fields: { priority: mark } })
    world.edit('plan:ts-0001-aa', { title: mark })
    const html = await drawer(world, 'tracker:12')
    expect(html).not.toMatch(/<img/i)
    expect(html).toContain('&lt;img src=x onerror=alert(1)&gt;')
    const linked = await drawer(world, 'plan:ts-0002-bb')
    expect(linked).not.toMatch(/<img/i)
  })

  test('the raw fields hold the hostile text as text', async () => {
    const world = drawerWorld()
    world.edit('tracker:12', { fields: { priority: HOSTILE_BODY } })
    const html = await drawer(world, 'tracker:12')
    expect(html).not.toMatch(/<script|<img/i)
  })
})

describe('what blocks', () => {
  test('is read from the declared link types, and a source that declares none blocks by blocked-by', async () => {
    const world = drawerWorld()
    const got = await world.owner.rpc('task.get', { board: 'b-alpha', id: 'plan:ts-0002-bb' })
    const data = await loadField(world.owner, 'b-alpha', null)
    const rows = blockingLinks(got, data.sources.find((each) => each.name === 'plan'))
    expect(rows?.waits.map((each) => each.key)).toEqual(['plan:ts-0001-aa'])
    expect(rows?.unblocks.map((each) => each.key)).toEqual(['plan:ts-0003-cc'])
    // The same links with no declaration: `blocked-by` is read as blocking, `part-of` is not.
    const bare = { ...got, link_types: [] }
    const same = blockingLinks(bare, data.sources.find((each) => each.name === 'plan'))
    expect(same?.waits.map((each) => each.key)).toEqual(['plan:ts-0001-aa'])
  })

  test('is nothing for a source with no blocking link type', async () => {
    const world = drawerWorld()
    const got = await world.owner.rpc('task.get', { board: 'b-alpha', id: 'scratch:1' })
    const data = await loadField(world.owner, 'b-alpha', null)
    expect(blockingLinks(got, data.sources.find((each) => each.name === 'scratch'))).toBeNull()
  })

  test('a link type that does not block is not a wait, even when it is incoming', async () => {
    const world = drawerWorld()
    world.listing = [
      task('plan:ts-0001-aa', 'One', 'ready', 'ready', { links: [{ type: 'part-of', to: 'plan:ts-0002-bb' }] }),
      task('plan:ts-0002-bb', 'Two', 'ready', 'ready'),
    ]
    world.views = [view('ready', { type: 'builtin' })]
    const got = await world.owner.rpc('task.get', { board: 'b-alpha', id: 'plan:ts-0002-bb' })
    const data = await loadField(world.owner, 'b-alpha', null)
    const rows = blockingLinks(got, data.sources.find((each) => each.name === 'plan'))
    expect(rows?.unblocks).toEqual([])
    expect(rows?.waits).toEqual([])
  })
})

describe('the parts as data', () => {
  test('a task with nothing but a title has only Details for engineers', async () => {
    const world = drawerWorld()
    world.listing = [task('scratch:2', 'Bare', 'backlog', 'backlog')]
    const data = await loadField(world.owner, 'b-alpha', null)
    const got = await world.owner.rpc('task.get', { board: 'b-alpha', id: 'scratch:2' })
    const parts = partsOf({ shown: data.tasks[0]!, got, data, board: null })
    expect(parts.map((each) => each.id)).toEqual(['details'])
  })
})
