import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { FieldPage } from '../src/field/FieldPage.tsx'
import { loadField, loadViews } from '../src/field/load.ts'
import { memoryStore } from '../src/field/folds.ts'
import { ViewPickerBody, viewSource } from '../src/field/ViewPicker.tsx'
import type { FieldState } from '../src/field/useField.ts'
import { type FieldWorld, eightStatuses, mixedSources, onePlanSource } from './support/field-world.ts'

// The Field's page and the view picker, drawn to text over the scripted worlds (ts-0181-ft). What a click, a key and a
// reload do is in `browser/fake-field.spec.ts`.

const noop = () => {}

async function page(world: FieldWorld, view: string | null = null, over: Partial<FieldState> = {}) {
  const views = await loadViews(world.owner, 'b-alpha')
  const data = await loadField(world.owner, 'b-alpha', view)
  const field: FieldState = { views, view, select: noop, data, problem: null, ...over }
  return renderToString(<FieldPage boardId="b-alpha" field={field} store={memoryStore()} />)
}

describe('the Field page', () => {
  test('says no board is open when none is', () => {
    const field: FieldState = { views: [], view: null, select: noop, data: null, problem: null }
    expect(renderToString(<FieldPage boardId={null} field={field} />)).toContain('No board is open')
  })

  test('says why the board could not be read', () => {
    const field: FieldState = { views: [], view: null, select: noop, data: null, problem: 'no daemon' }
    const html = renderToString(<FieldPage boardId="b-alpha" field={field} />)
    expect(html).toContain('The board could not be read: no daemon')
  })

  test('says it is reading while nothing has come', () => {
    const field: FieldState = { views: [], view: null, select: noop, data: null, problem: null }
    expect(renderToString(<FieldPage boardId="b-alpha" field={field} />)).toContain('Reading the board')
  })

  test('draws a column for each category of the plan, each with its words and its count', async () => {
    const html = await page(onePlanSource())
    for (const word of ['Ready', 'Active', 'Review']) expect(html).toContain(`aria-label="${word}"`)
    // The plan has no backlog status, so it has no such column.
    expect(html).not.toContain('aria-label="Backlog"')
    expect(html).toContain('can start')
    expect(html).toContain('being worked on')
  })

  test('draws Done and Dropped folded, with their counts and none of their cards', async () => {
    const html = await page(onePlanSource())
    expect(html).not.toContain('aria-label="Done"')
    expect(html).not.toContain('aria-label="Dropped"')
    expect(html).not.toContain('Ship the shell')
    expect(html).toMatch(/Done<[^]*?>2</)
  })

  test('draws a card with its title, its short id and its stage', async () => {
    const html = await page(onePlanSource())
    expect(html).toContain('Design the board')
    expect(html).toContain('ts-0002-bb')
    expect(html).toContain('ep-006-wq')
  })

  test('gives a card that is being worked on the lit border, and no other', async () => {
    const html = await page(onePlanSource())
    const lit = html.match(/<button[^>]*ring-c-active[^>]*>/g) ?? []
    expect(lit).toHaveLength(1)
    expect(lit[0]).toContain('data-card="plan:ts-0003-cc|"')
  })

  test('draws chips only for what a card says: waits, a difference, a collision', async () => {
    const html = await page(onePlanSource())
    expect(html).toContain('Waits on 1')
    expect(html).toContain('r-121')
    expect(html).toMatch(/collid|same|twice|two places/i)
  })

  test('draws the eight statuses as columns, each saying what it counts as', async () => {
    const html = await page(eightStatuses())
    for (const name of ['Idea', 'Triage', 'Queued', 'Doing', 'In_review', 'Qa']) {
      expect(html).toContain(`aria-label="${name}"`)
    }
    expect(html).toContain('counts as backlog')
    expect(html).toContain('counts as in review')
  })

  test('draws a mixed board as the six categories', async () => {
    const html = await page(mixedSources())
    expect(html).toContain('aria-label="In review"')
    expect(html).not.toContain('counts as')
  })

  test('says what the view says of itself, above the board', async () => {
    const html = await page(onePlanSource(), 'ready')
    expect(html).toContain('aria-label="About this view"')
    expect(html).toContain('Tasks that wait on each other')
    expect(html.indexOf('About this view')).toBeLessThan(html.indexOf('aria-label="Columns"'))
  })

  test('says nothing of a view that has nothing to say', async () => {
    expect(await page(onePlanSource(), 'by-status')).not.toContain('About this view')
    expect(await page(onePlanSource())).not.toContain('About this view')
  })

  test('names the view above the board, and counts its tasks', async () => {
    expect(await page(onePlanSource(), 'by-status')).toMatch(/<h1[^>]*>by-status<\/h1>/)
    expect(await page(onePlanSource())).toMatch(/<h1[^>]*>All tasks<\/h1>/)
  })

  test('has no drawer place until a card is selected', async () => {
    expect(await page(onePlanSource())).not.toContain('The drawer comes next')
  })
})

describe('the view picker', () => {
  const body = async (view: string | null = null) => {
    const world = onePlanSource()
    const views = await loadViews(world.owner, 'b-alpha')
    return renderToString(<ViewPickerBody boardName="alpha" views={views} view={view} onChoose={noop} />)
  }

  test('lists the whole board first, then the built-in view, then the rest', async () => {
    const html = await body()
    const at = (text: string) => html.indexOf(text)
    expect(at('All tasks')).toBeGreaterThan(-1)
    expect(at('All tasks')).toBeLessThan(at('>ready<'))
    expect(at('>ready<')).toBeLessThan(at('>mine<'))
  })

  test('says where each view is kept', async () => {
    const html = await body()
    expect(html).toContain('built in')
    expect(html).toContain('from collide/plan')
    expect(html).toContain('in this repo')
  })

  test('marks the view that is shown, and only it', async () => {
    const html = await body('mine')
    expect(html.match(/aria-current="true"/g)).toHaveLength(1)
    expect(html).toMatch(/aria-current="true"[^>]*>(?:(?!<\/button>)[^])*mine/)
  })

  test.each([
    [{ type: 'builtin' }, 'built in'],
    [{ type: 'personal' }, 'yours'],
    [{ type: 'repo' }, 'in this repo'],
    [{ type: 'pack', pack: 'octo' }, 'from octo'],
  ] as const)('words for %j', (source, words) => {
    expect(viewSource({ source } as never)).toBe(words)
  })
})
