import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { loadField } from '../src/field/load.ts'
import { PlanningView, type PlanningViewProps } from '../src/planning/PlanningView.tsx'
import { planningWorld } from './support/planning-world.ts'

// The Planning tab drawn to text over the scripted world (ts-0195-ju). What a click does is in
// `browser/fake-planning.spec.ts`, and the counts against the real CLI are in `browser/planning.spec.ts`.

const noop = () => {}

async function page(over: Partial<PlanningViewProps> = {}): Promise<string> {
  const world = planningWorld()
  const data = await loadField(world.owner, 'b-alpha', null)
  const sections = await world.owner.rpc('task.sections', { board: 'b-alpha' })
  return renderToString(
    <PlanningView data={data} sections={sections} picked={null} onPick={noop} onOpenTask={noop} {...over} />,
  )
}

/** The text of the part of the page under the heading `name`, up to the next heading of its level. */
function under(html: string, name: string): string {
  const from = html.indexOf(`>${name}<`)
  expect(from, `a heading "${name}"`).toBeGreaterThan(-1)
  const rest = html.slice(from + 1)
  const next = rest.search(/<h2/)
  return (next === -1 ? rest : rest.slice(0, next)).replace(/<[^>]+>/g, ' ').replace(/\s+/g, ' ')
}

describe('the head of the page', () => {
  test('says Planning, that it is read only, and where changes are made', async () => {
    const html = await page()
    expect(html).toContain('>Planning<')
    expect(html).toContain('read only')
    expect(html).toContain('Changes are made in the files or by a planning session (Stage 9d).')
  })

  test('has a Plan button that is off and says planning sessions arrive in Stage 9d', async () => {
    const html = await page()
    const button = html.match(/<button[^>]*>Plan…<\/button>/)?.[0]
    expect(button).toBeDefined()
    expect(button).toContain('disabled')
    expect(html).toContain('Planning sessions arrive in Stage 9d.')
  })

  test('has no Sort and no merge', async () => {
    const words = (await page()).replace(/<[^>]+>/g, ' ')
    expect(words).not.toMatch(/\b(Sort|Merge)\b/i)
  })
})

describe('the epics', () => {
  test('are a tile for each, with how many are done of how many, and the next ready task', async () => {
    const words = under(await page(), 'Epics')
    expect(words).toContain('Board UI first 1 of 5 done · next: Show each copy')
    expect(words).toContain('GitHub source 2 of 4 done · next: Read more issues')
  })
})

describe('the proposals', () => {
  test('are each with its state and its section and feature counts', async () => {
    const words = under(await page(), 'Proposals')
    expect(words).toContain('Board UI before GitHub draft · 3 sections, 4 features')
    expect(words).toContain('GitHub source accepted · 2 sections, 2 features')
  })

  test('say only on branch <branch> for the one that is only on a branch', async () => {
    const words = under(await page(), 'Proposals')
    expect(words).toContain('Loose ideas draft · 0 sections, 1 feature only on branch plan/ui-first')
    expect(words.match(/only on branch/g)).toHaveLength(1)
  })

  test('mark the one Readiness shows', async () => {
    const html = await page({ picked: 'plan:pr-0002-bb' })
    expect(html.match(/<button[^>]*aria-pressed="true"[^>]*data-proposal="plan:pr-0002-bb"/)).not.toBeNull()
    expect(html.match(/aria-pressed="true"/g)).toHaveLength(1)
  })
})

describe('the features by section', () => {
  test('are a row for each, under Section, Feature, From, State and Task', async () => {
    const html = await page()
    for (const name of ['Section', 'Feature', 'From', 'State', 'Task']) {
      expect(html).toMatch(new RegExp(`<th[^>]*>${name}</th>`))
    }
    expect(html.match(/data-feature="/g)).toHaveLength(7)
  })

  test('name the tasks by short id and the state each feature has come to', async () => {
    const words = under(await page(), 'Features by section')
    expect(words).toContain('Issues as tasks GitHub source 1 of 2 done 0005-ee 0007-gg')
    expect(words).toContain('Settings view GitHub source accepted')
  })

  test('say a near-duplicate looks like the other', async () => {
    const words = under(await page(), 'Features by section')
    expect(words).toContain('Setting looks like Settings')
    expect(words).toContain('Settings looks like Setting')
  })

  test('say Unsorted for a feature with no section', async () => {
    const html = await page()
    expect(html).toMatch(/data-section="Unsorted"/)
  })
})

describe('readiness', () => {
  test('is the picked proposal, with its checklist, disabled', async () => {
    const html = await page({ picked: 'plan:pr-0001-aa' })
    const words = under(html, 'Readiness')
    expect(words).toContain('Board UI before GitHub')
    expect(words).toContain('The Features part has no problems')
    expect(words).toContain('F2 has no task; ts-0004-dd has no feature')
    const part = html.slice(html.indexOf('>Readiness<'))
    const boxes = [...part.matchAll(/<input[^>]*>/g)].map((each) => each[0])
    expect(boxes).toHaveLength(5)
    expect(boxes.every((box) => box.includes('disabled'))).toBe(true)
  })

  test('is the first open proposal when none is picked', async () => {
    const words = under(await page(), 'Readiness')
    expect(words).toContain('Board UI before GitHub')
  })

  test('has a proposal to show only when the board has one', async () => {
    const world = planningWorld()
    world.listing = world.listing.filter((each) => each.kind !== 'proposal')
    const data = await loadField(world.owner, 'b-alpha', null)
    const html = renderToString(
      <PlanningView data={data} sections={null} picked={null} onPick={noop} onOpenTask={noop} />,
    )
    expect(under(html, 'Readiness')).toContain('No proposal yet.')
  })
})
