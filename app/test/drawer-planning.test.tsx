import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { DrawerView, type DrawerViewProps } from '../src/drawer/DrawerView.tsx'
import { checklistRows, claimRows, coverageLines, linkRows, partsOf } from '../src/drawer/model.ts'
import { loadField } from '../src/field/load.ts'
import { planningWorld } from './support/planning-world.ts'

// The planning parts of the drawer (ts-0195-ju): Links of every type both ways, a checklist, claims and coverage. The
// same wire the daemon gives: `task.list` for the Field and `task.get` for the task.

const noop = () => {}

async function read(key: string) {
  const world = planningWorld()
  const data = await loadField(world.owner, 'b-alpha', null)
  const shown = data.tasks.find((each) => each.key === key)!
  const got = await world.owner.rpc('task.get', { board: 'b-alpha', id: key })
  return { world, data, shown, got, input: { shown, got, data, board: null } }
}

async function drawer(key: string, over: Partial<DrawerViewProps> = {}): Promise<string> {
  const { shown, got, data } = await read(key)
  return renderToString(
    <DrawerView
      shown={shown}
      got={got}
      problem={null}
      data={data}
      board={{ name: 'alpha', line: 'main' }}
      shut={[]}
      changes={null}
      onFlip={noop}
      onOpenTask={noop}
      onOpenFile={noop}
      onClose={noop}
      {...over}
    />,
  )
}

const partsOfHtml = (html: string) => [...html.matchAll(/data-part="([^"]+)"/g)].map((each) => each[1]!)
const part = (html: string, id: string) => {
  const from = html.indexOf(`data-part="${id}"`)
  const next = html.indexOf('data-part="', from + 1)
  return html.slice(from, next === -1 ? undefined : next)
}

describe('the links of a planning item, both ways', () => {
  test('name an outgoing link by its type, with the feature it reaches', async () => {
    const { got } = await read('plan:ts-0002-bb')
    expect(linkRows(got).map((row) => [row.label, row.link.key, row.link.feature])).toEqual([
      ['implements', 'plan:pr-0001-aa', 'F3'],
    ])
  })

  test('name an incoming link by the inverse of its type', async () => {
    const { got } = await read('plan:pr-0001-aa')
    const labels = linkRows(got).map((row) => [row.label, row.link.key, row.link.feature ?? ''])
    expect(labels).toContainEqual(['implemented by', 'plan:ts-0001-aa', 'F1'])
    expect(labels).toContainEqual(['implemented by', 'plan:ts-0003-cc', 'F3'])
    expect(labels).toContainEqual(['informed by', 'plan:sp-0001-aa', ''])
  })

  test('name an incoming includes link "included in"', async () => {
    const { got } = await read('plan:ts-0004-dd')
    expect(linkRows(got).map((row) => [row.label, row.link.key])).toEqual([['included in', 'plan:pr-0001-aa']])
  })

  test('name an incoming link of a type with no inverse by the type and "incoming"', async () => {
    const { world } = await read('plan:pr-0001-aa')
    world.linkTypes.set('plan', [{ name: 'implements', dir: 'out', blocking: false, target: 'feature' }])
    const got = await world.owner.rpc('task.get', { board: 'b-alpha', id: 'plan:pr-0001-aa' })
    expect(linkRows(got).map((row) => row.label)).toContain('implements (incoming)')
  })

  test('leave out the links that block, and the part-of that Part of names', async () => {
    const { got } = await read('plan:ts-0002-bb')
    expect(linkRows(got).map((row) => row.link.type)).toEqual(['implements'])
    const { got: proposal } = await read('plan:pr-0001-aa')
    expect(linkRows(proposal).some((row) => row.link.type === 'part-of' && row.link.dir === 'out')).toBe(false)
  })

  test('are drawn as a part with a row for each, the far side named by its inverse', async () => {
    const html = part(await drawer('plan:pr-0001-aa'), 'links')
    expect(html).toContain('implemented by')
    expect(html).toContain('Draw the columns')
    expect(html).toContain('F1')
    expect(html).toContain('informed by')
  })

  test('are a part only when there is a link to show', async () => {
    const { input } = await read('plan:ts-0001-aa')
    expect(partsOf(input).map((each) => each.id)).toContain('links')
    const { world } = await read('plan:ts-0004-dd')
    world.edit('plan:pr-0001-aa', { links: [{ type: 'part-of', to: 'ep-001-aa' }] })
    const data = await loadField(world.owner, 'b-alpha', null)
    const got = await world.owner.rpc('task.get', { board: 'b-alpha', id: 'plan:ts-0004-dd' })
    const shown = data.tasks.find((each) => each.key === 'plan:ts-0004-dd')!
    expect(partsOf({ shown, got, data, board: null }).map((each) => each.id)).not.toContain('links')
  })
})

describe('the checklist, the claims and the coverage', () => {
  test('a checklist is each item with its state, read only', async () => {
    const { got } = await read('plan:ts-0003-cc')
    expect(checklistRows(got.task!)).toEqual([
      { id: 'tests', state: 'done', checked: true, note: 'by ana, 2026-11-20 · ts-0001-aa' },
      { id: 'review', state: 'suggested', checked: false, note: 'the diff touches the wire' },
      { id: 'docs', state: 'n/a', checked: false, note: 'no user text' },
      { id: 'owner', state: 'open', checked: false, note: '' },
    ])
  })

  test('are each under a part of their own, in order, after Part of', async () => {
    const html = await drawer('plan:ts-0003-cc')
    expect(partsOfHtml(html)).toEqual([
      'waits',
      'unblocks',
      'links',
      'part-of',
      'checklist',
      'changes',
      'copies',
      'details',
    ])
    const boxes = [...part(html, 'checklist').matchAll(/<input[^>]*>/g)].map((each) => each[0])
    expect(boxes).toHaveLength(4)
    expect(boxes.every((box) => box.includes('disabled'))).toBe(true)
  })

  test('a spike has its claims', async () => {
    const { got } = await read('plan:sp-0001-aa')
    expect(claimRows(got.task!)).toEqual([
      { id: 'C1', words: 'measured · load-bearing · verifiable' },
      { id: 'C2', words: 'inferred' },
    ])
    const html = part(await drawer('plan:sp-0001-aa'), 'claims')
    expect(html).toContain('C1')
    expect(html).toContain('load-bearing')
  })

  test('a proposal has its coverage: the features with no task, the tasks with no feature', async () => {
    const { got } = await read('plan:pr-0001-aa')
    expect(coverageLines(got.task!)).toEqual([
      'F2 has no task',
      'ts-0004-dd has no feature',
    ])
    const html = part(await drawer('plan:pr-0001-aa'), 'coverage')
    expect(html).toContain('F2 has no task')
  })

  test('a proposal that is covered says so', async () => {
    const { world } = await read('plan:pr-0001-aa')
    world.edit('plan:pr-0001-aa', { coverage: { featuresWithoutTask: [], tasksWithoutFeature: [] } })
    const got = await world.owner.rpc('task.get', { board: 'b-alpha', id: 'plan:pr-0001-aa' })
    expect(coverageLines(got.task!)).toEqual(['Every feature has a task, and every included task has a feature'])
  })

  test('a task with none of them has none of the parts', async () => {
    const { input } = await read('plan:ts-0004-dd')
    const ids = partsOf(input).map((each) => each.id)
    expect(ids).not.toContain('checklist')
    expect(ids).not.toContain('claims')
    expect(ids).not.toContain('coverage')
  })
})
