import { describe, expect, test } from 'vitest'
import { loadField } from '../src/field/load.ts'
import {
  defaultProposal,
  featureRowsOf,
  proposalWords,
  proposalsOf,
  readinessOf,
  tileWords,
  tilesOf,
} from '../src/planning/model.ts'
import { planningWorld } from './support/planning-world.ts'

// What the Planning tab says, from the same wire the daemon gives: `task.list` and `task.sections`
// (ts-0195-ju). The scripted sections agree with the scripted features; `browser/planning.spec.ts` checks the real
// ones against `collide sections --json`.

async function read() {
  const world = planningWorld()
  const data = await loadField(world.owner, 'b-alpha', null)
  const sections = await world.owner.rpc('task.sections', { board: 'b-alpha' })
  return { world, data, sections }
}

const key = (full: string) => full.replace('plan:', '')

describe('the epic tiles', () => {
  test('are one for each epic the board lists, open work first, counted as the strip counts', async () => {
    const { data } = await read()
    const tiles = tilesOf(data)
    expect(tiles.map((tile) => key(tile.card.id))).toEqual(['ep-001-aa', 'ep-002-bb'])
    expect(tiles.map((tile) => [tile.card.done, tile.card.total])).toEqual([
      [1, 5],
      [2, 4],
    ])
  })

  test('name the next ready task: a task that is ready and waits on nothing open', async () => {
    const { data } = await read()
    const [first, second] = tilesOf(data)
    // `ts-0002-bb` is ready too, but waits on `ts-0003-cc`, which is under way.
    expect(key(first!.next!.key)).toBe('ts-0004-dd')
    expect(key(second!.next!.key)).toBe('ts-0007-gg')
  })

  test('say how many are done, and what comes next', async () => {
    const { data } = await read()
    expect(tilesOf(data).map(tileWords)).toEqual([
      '1 of 5 done · next: Show each copy',
      '2 of 4 done · next: Read more issues',
    ])
  })

  test('have no next task for an epic that has no ready task', async () => {
    const { world } = await read()
    world.listing = world.listing.filter((each) => each.key !== 'plan:ts-0007-gg')
    const data = await loadField(world.owner, 'b-alpha', null)
    const second = tilesOf(data)[1]!
    expect(second.next).toBeNull()
    expect(tileWords(second)).toBe('2 of 3 done')
  })
})

describe('the proposals', () => {
  test('are the proposals of the board in key order, with their state and counts', async () => {
    const { data } = await read()
    const rows = proposalsOf(data)
    expect(rows.map((row) => [key(row.key), row.status, row.sections, row.features])).toEqual([
      ['pr-0001-aa', 'draft', 3, 4],
      ['pr-0002-bb', 'accepted', 2, 2],
      ['pr-0003-cc', 'draft', 0, 1],
    ])
    expect(rows.map(proposalWords)).toEqual([
      'draft · 3 sections, 4 features',
      'accepted · 2 sections, 2 features',
      'draft · 0 sections, 1 feature',
    ])
  })

  test('say only on branch <branch> for a proposal that is on no other line', async () => {
    const { data } = await read()
    expect(proposalsOf(data).map((row) => row.only)).toEqual([null, null, 'only on branch plan/ui-first'])
  })

  test('say where a branch-only proposal is when its checkout is on no branch', async () => {
    const { world } = await read()
    world.branchOnly = [{ key: 'plan:pr-0003-cc', checkout: '/work/alpha-plan' }]
    const data = await loadField(world.owner, 'b-alpha', null)
    expect(proposalsOf(data)[2]!.only).toBe('only in alpha-plan')
  })

  test('open the first one that is not finished, and the first of all when none is open', async () => {
    const { data } = await read()
    const rows = proposalsOf(data)
    expect(defaultProposal(rows)).toBe('plan:pr-0001-aa')
    const finished = rows.map((row) => ({ ...row, open: false }))
    expect(defaultProposal(finished)).toBe('plan:pr-0001-aa')
    expect(defaultProposal(rows.slice(1, 2).map((row) => ({ ...row, open: false })))).toBe('plan:pr-0002-bb')
    expect(defaultProposal([])).toBeNull()
  })

  test('skip the epics, the tasks and the other kinds', async () => {
    const { data } = await read()
    expect(proposalsOf(data)).toHaveLength(3)
  })
})

describe('the features by section', () => {
  test('are in the order of the sections the daemon reports, the unsorted ones last', async () => {
    const { data, sections } = await read()
    const rows = featureRowsOf(data, sections)
    expect(rows.map((row) => [row.section, key(row.proposal), row.featureId])).toEqual([
      ['Field', 'Board UI before GitHub', 'F1'],
      ['Field', 'Board UI before GitHub', 'F2'],
      ['Drawer', 'Board UI before GitHub', 'F3'],
      ['Setting', 'GitHub source', 'F2'],
      ['Settings', 'Board UI before GitHub', 'F4'],
      ['Sources', 'GitHub source', 'F1'],
      ['Unsorted', 'Loose ideas', 'F1'],
    ])
  })

  test('add up, for each section, to the feature count the daemon reports for it', async () => {
    const { data, sections } = await read()
    const rows = featureRowsOf(data, sections)
    for (const section of sections.sections) {
      const named = section.name === '' ? 'Unsorted' : section.name
      expect(rows.filter((row) => row.section === named)).toHaveLength(section.featureCount)
    }
    expect(rows).toHaveLength(sections.sections.reduce((all, each) => all + each.featureCount, 0))
  })

  test('mark only the features with no section as unsorted', async () => {
    const { data, sections } = await read()
    expect(featureRowsOf(data, sections).filter((row) => row.unsorted).map((row) => row.title)).toEqual([
      'Order by hand',
    ])
  })

  test('say what each feature has come to: planned until accepted, then by its tasks', async () => {
    const { data, sections } = await read()
    const state = (proposal: string, feature: string) =>
      featureRowsOf(data, sections).find((row) => key(row.id).startsWith(`${proposal}#${feature}`))!.state
    expect(state('pr-0001-aa', 'F1')).toBe('planned')
    // Accepted, with a task that is done and one that is not; a dropped task is not counted.
    expect(state('pr-0002-bb', 'F1')).toBe('1 of 2 done')
    // Accepted, and no task yet.
    expect(state('pr-0002-bb', 'F2')).toBe('accepted')
  })

  test('name the tasks that implement a feature by their short ids, and none that are dropped', async () => {
    const { data, sections } = await read()
    const tasks = (proposal: string, feature: string) =>
      featureRowsOf(data, sections)
        .find((row) => key(row.id).startsWith(`${proposal}#${feature}`))!
        .tasks.map((task) => task.shortId)
    expect(tasks('pr-0001-aa', 'F1')).toEqual(['0001-aa'])
    expect(tasks('pr-0001-aa', 'F3')).toEqual(['0002-bb', '0003-cc'])
    expect(tasks('pr-0002-bb', 'F1')).toEqual(['0005-ee', '0007-gg'])
    expect(tasks('pr-0002-bb', 'F2')).toEqual([])
  })

  test('say which section a near-duplicate looks like', async () => {
    const { data, sections } = await read()
    const looks = (section: string) =>
      featureRowsOf(data, sections)
        .filter((row) => row.section === section)
        .map((row) => row.looksLike)
    expect(looks('Setting')).toEqual([['Settings']])
    expect(looks('Settings')).toEqual([['Setting']])
    expect(looks('Field')).toEqual([[], []])
  })

  test('leave out the features of a proposal that is dropped, as the daemon does', async () => {
    const { world, sections } = await read()
    world.edit('plan:pr-0002-bb', { state: { status: 'rejected', category: 'dropped' } })
    const data = await loadField(world.owner, 'b-alpha', null)
    const rows = featureRowsOf(data, sections)
    expect(rows).toHaveLength(5)
    expect(rows.some((row) => key(row.proposal) === 'GitHub source')).toBe(false)
  })

  test('are still listed, in the order they come, before the sections are read', async () => {
    const { data } = await read()
    const rows = featureRowsOf(data, null)
    expect(rows).toHaveLength(7)
    expect(rows.at(-1)!.unsorted).toBe(true)
  })
})

describe('the readiness lines', () => {
  const proposal = async (id: string) => {
    const { data } = await read()
    return data.corpus.find((each) => each.key === id)!
  }

  test('are advice from the proposal: what is right and what is not', async () => {
    const lines = readinessOf(await proposal('plan:pr-0001-aa'))
    expect(lines.map((line) => [line.text, line.ok])).toEqual([
      ['The Features part has no problems', true],
      ['No feature is Unsorted', true],
      ['Every feature has a task, and every included task has a feature', false],
      ['Each assumption is verified, or an accepted risk', false],
      ['Every decision names an alternative it passed on', true],
    ])
  })

  test('say why a line is not right', async () => {
    const lines = readinessOf(await proposal('plan:pr-0001-aa'))
    const why = (id: string) => lines.find((line) => line.id === id)!.why
    expect(why('coverage')).toBe('F2 has no task; ts-0004-dd has no feature')
    expect(why('assumptions')).toBe('A1 is unverified')
  })

  test('catch a feature that did not parse and a feature with no section', async () => {
    const lines = readinessOf(await proposal('plan:pr-0003-cc'))
    expect(lines.slice(0, 2).map((line) => [line.id, line.ok, line.why])).toEqual([
      ['features', false, 'line 9: a feature has no title'],
      ['unsorted', false, 'F1'],
    ])
  })
})
