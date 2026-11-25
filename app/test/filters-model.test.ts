import { describe, expect, test } from 'vitest'
import {
  type Filters,
  NO_FILTERS,
  celOf,
  epicWords,
  groupsOf,
  isFiltering,
  saveOf,
  summaryOf,
  toggle,
} from '../src/filters/model.ts'
import { readBarFolded, setBarFolded } from '../src/filters/fold.ts'
import { memoryStore } from '../src/field/folds.ts'
import { rowsOf } from '../src/field/ViewPicker.tsx'
import { task, view } from './support/field-world.ts'

// The filter bar's rules, with no page (ts-0190-tz; Stage 6a, Filter bar; 0.11, Filter bar to view): the CEL each
// set of choices writes, what a search text may hold, which groups show and with what counts, what a saved view is
// made of, and which rows the picker lists.

const on = (over: Partial<Filters>): Filters => ({ ...NO_FILTERS, ...over })

describe('the expression the chips write', () => {
  test('is nothing when no choice is made', () => {
    expect(isFiltering(NO_FILTERS)).toBe(false)
    expect(celOf(NO_FILTERS)).toBeNull()
  })

  test('is one term for a group with one choice, and the choices of a group are or-ed', () => {
    expect(celOf(on({ categories: ['ready'] }))).toBe('category in ["ready"]')
    expect(celOf(on({ categories: ['ready', 'active'] }))).toBe('category in ["ready", "active"]')
    expect(celOf(on({ sources: ['plan', 'eight'] }))).toBe('source in ["plan", "eight"]')
    expect(celOf(on({ epics: ['plan:ep-001-aa'] }))).toBe('"plan:ep-001-aa" in collections')
    expect(celOf(on({ epics: ['plan:ep-001-aa', 'plan:ep-002-bb'] }))).toBe(
      '("plan:ep-001-aa" in collections || "plan:ep-002-bb" in collections)',
    )
    expect(celOf(on({ labels: ['perf'] }))).toBe('"perf" in labels')
    expect(celOf(on({ labels: ['perf', 'ux'] }))).toBe('("perf" in labels || "ux" in labels)')
  })

  test('and-s the groups, one to a line, in the order of the bar', () => {
    const filters = on({ epics: ['plan:ep-001-aa'], categories: ['active'], sources: ['plan'], labels: ['perf'] })
    expect(celOf(filters)).toBe(
      [
        '"plan:ep-001-aa" in collections',
        'category in ["active"]',
        'source in ["plan"]',
        '"perf" in labels',
      ].join('\n  && '),
    )
  })

  test("puts the view's own `where` first, in brackets, when one is given", () => {
    expect(celOf(on({ categories: ['ready'] }), "status == 'todo' || status == 'doing'")).toBe(
      ['(status == \'todo\' || status == \'doing\')', 'category in ["ready"]'].join('\n  && '),
    )
    expect(celOf(NO_FILTERS, "status == 'todo'")).toBe("(status == 'todo')")
  })

  test('searches the title and the key, in any case', () => {
    expect(celOf(on({ text: 'Parser' }))).toBe('(title.matches("(?i)Parser") || key.matches("(?i)Parser"))')
  })

  test('takes the white space off a search, and a search of only white space is no choice', () => {
    expect(celOf(on({ text: '  parser ' }))).toBe(celOf(on({ text: 'parser' })))
    expect(isFiltering(on({ text: '   ' }))).toBe(false)
    expect(celOf(on({ text: '   ' }))).toBeNull()
  })

  test('keeps a search text with a quote or a backslash in one string', () => {
    expect(celOf(on({ text: 'say "hi"' }))).toBe(
      '(title.matches("(?i)say \\"hi\\"") || key.matches("(?i)say \\"hi\\""))',
    )
    expect(celOf(on({ text: 'a\\b' }))).toBe('(title.matches("(?i)a\\\\\\\\b") || key.matches("(?i)a\\\\\\\\b"))')
  })

  test('keeps a search text with a pattern in it a plain text', () => {
    // `.` and `(` would be a pattern in a regex: the bar escapes them, so they are the characters typed.
    expect(celOf(on({ text: 'v1.2 (beta)' }))).toContain('(?i)v1\\\\.2 \\\\(beta\\\\)')
  })

  test('quotes a label or an epic that holds a quote', () => {
    expect(celOf(on({ labels: ['the "big" one'] }))).toBe('"the \\"big\\" one" in labels')
  })
})

describe('what the choices are made with', () => {
  test('toggles a value in and out of a group, keeping the order they were chosen in', () => {
    expect(toggle(['a'], 'b')).toEqual(['a', 'b'])
    expect(toggle(['a', 'b'], 'a')).toEqual(['b'])
    expect(toggle([], 'a')).toEqual(['a'])
  })
})

const EPIC_A = 'plan:ep-001-aa'
const EPIC_B = 'plan:ep-002-bb'
const COLLECTIONS = [
  { id: EPIC_A, title: 'The Field', groups: [] },
  { id: EPIC_B, title: 'The drawer', groups: [] },
  { id: 'plan:ep-003-cc', title: 'Nothing in it', groups: [] },
]

const TASKS = [
  task('plan:ts-0001-aa', 'One', 'ready', 'ready', { collections: [EPIC_A], labels: ['perf'] }),
  task('plan:ts-0002-bb', 'Two', 'active', 'active', { collections: [EPIC_A], labels: ['perf', 'ux'] }),
  task('plan:ts-0003-cc', 'Three', 'active', 'active', { collections: [EPIC_B] }),
  task('eight:4', 'Four', 'done', 'done'),
]

describe('which groups show, and their counts', () => {
  test('counts the tasks of each category, all six, even where there are none', () => {
    const { categories } = groupsOf(TASKS, COLLECTIONS)
    expect(categories.map((each) => [each.id, each.count])).toEqual([
      ['backlog', 0],
      ['ready', 1],
      ['active', 2],
      ['review', 0],
      ['done', 1],
      ['dropped', 0],
    ])
    expect(categories.map((each) => each.word)).toEqual([
      'Backlog',
      'Ready',
      'Active',
      'In review',
      'Done',
      'Dropped',
    ])
  })

  test('lists the epics the tasks are in, with their titles and counts, and leaves out an epic with no task', () => {
    const { epics } = groupsOf(TASKS, COLLECTIONS)
    expect(epics).toEqual([
      { id: EPIC_A, title: 'The Field', count: 2 },
      { id: EPIC_B, title: 'The drawer', count: 1 },
    ])
  })

  test('has no epics to choose between when the tasks are in one epic or none', () => {
    expect(groupsOf(TASKS.slice(0, 2), COLLECTIONS).epics).toEqual([])
    expect(groupsOf(TASKS.slice(3), COLLECTIONS).epics).toEqual([])
  })

  test('lists the sources when there is more than one, with counts', () => {
    expect(groupsOf(TASKS, COLLECTIONS).sources).toEqual([
      { name: 'plan', count: 3 },
      { name: 'eight', count: 1 },
    ])
    expect(groupsOf(TASKS.slice(0, 3), COLLECTIONS).sources).toEqual([])
  })

  test('lists the labels the tasks carry, with counts, and none when no task carries one', () => {
    expect(groupsOf(TASKS, COLLECTIONS).labels).toEqual([
      { label: 'perf', count: 2 },
      { label: 'ux', count: 1 },
    ])
    expect(groupsOf(TASKS.slice(2), COLLECTIONS).labels).toEqual([])
  })

  test('counts the tasks the view holds, whatever the other choices are', () => {
    // The counts are made over the view's tasks and not over what the bar shows, so a count never shrinks as you click.
    const all = groupsOf(TASKS, COLLECTIONS)
    expect(all.categories.find((each) => each.id === 'active')?.count).toBe(2)
  })
})

describe('the words of the Epic box and of the folded bar', () => {
  test('say Any, the epic chosen by its title, or how many', () => {
    expect(epicWords(NO_FILTERS, COLLECTIONS)).toBe('Any')
    expect(epicWords(on({ epics: [EPIC_A] }), COLLECTIONS)).toBe('The Field')
    expect(epicWords(on({ epics: [EPIC_A, EPIC_B] }), COLLECTIONS)).toBe('2 epics')
    expect(epicWords(on({ epics: ['plan:ep-gone'] }), COLLECTIONS)).toBe('plan:ep-gone')
  })

  test('name each group in force, in plain words', () => {
    const filters = on({
      epics: [EPIC_A],
      categories: ['ready', 'active'],
      sources: ['plan'],
      labels: ['perf'],
      text: 'parser',
    })
    expect(summaryOf(filters, COLLECTIONS)).toEqual([
      'Epic: The Field',
      'Where it stands: Ready, Active',
      'Comes from: plan',
      'Labels: perf',
      'Search: "parser"',
    ])
    expect(summaryOf(NO_FILTERS, COLLECTIONS)).toEqual([])
  })
})

describe('Save as a view', () => {
  const chips = on({ categories: ['active'] })

  test('over the whole board is the chips as a `where`', () => {
    expect(saveOf(undefined, chips, 'Active ones')).toEqual({
      view: { name: 'Active ones', where: 'category in ["active"]' },
    })
  })

  test('over a view with a `where` ands the chips onto it, and keeps its `exclude` and `layout`', () => {
    const base = view(
      'open',
      { type: 'repo' },
      { where: "status != 'done'", exclude: ['plan:ts-0009-ii'], layout: { default: 'graph', groupBy: 'category' } },
    )
    expect(saveOf(base, chips, 'Open and active')).toEqual({
      view: {
        name: 'Open and active',
        where: ["(status != 'done')", 'category in ["active"]'].join('\n  && '),
        exclude: ['plan:ts-0009-ii'],
        layout: { default: 'graph', groupBy: 'category' },
      },
    })
  })

  test('is refused over a built-in view, which no file can say, and the words say why', () => {
    const base = view('ready', { type: 'builtin' })
    const refused = saveOf(base, chips, 'x')
    expect(refused).toHaveProperty('why')
    expect((refused as { why: string }).why).toContain('built-in')
  })

  test('is refused over a view that names tasks or walks to a goal, whose members the chips would not reach', () => {
    const named = saveOf(view('picked', { type: 'repo' }, { include: ['plan:ts-0001-aa'] }), chips, 'x')
    const walked = saveOf(
      view('to-the-wire', { type: 'repo' }, { closure: { goal: 'plan:ts-0004-dd', follow: 'depends_on' } }),
      chips,
      'x',
    )
    for (const refused of [named, walked]) {
      expect(refused).toHaveProperty('why')
      expect((refused as { why: string }).why).toContain('names tasks')
    }
  })

  test('puts the name in as typed, less the white space round it', () => {
    expect(saveOf(undefined, chips, '  Mine  ')).toEqual({ view: { name: 'Mine', where: 'category in ["active"]' } })
  })

  test('is refused with no name', () => {
    expect(saveOf(undefined, chips, '  ')).toHaveProperty('why')
  })
})

describe('the picker rows', () => {
  const personal = view('mine', { type: 'personal' }, { where: 'true' })
  const repo = view('mine', { type: 'repo' }, { where: 'true' })

  test('lists a personal view and the repo view of the same name once, as the repo view, when they match', () => {
    const rows = rowsOf([view('ready', { type: 'builtin' }), personal, repo])
    expect(rows.map((row) => [row.view.name, row.source.type])).toEqual([
      ['ready', 'builtin'],
      ['mine', 'repo'],
    ])
  })

  test('says what the personal view, which the daemon uses, says: its error and its count', () => {
    const used = { ...personal, task_count: 7, error_count: 2 }
    const shadowed = { ...repo, error: '`mine` is also a personal view, and that one is used' }
    const [row] = rowsOf([used, shadowed])
    expect(row?.task_count).toBe(7)
    expect(row?.error_count).toBe(2)
    expect(row?.error).toBeUndefined()
  })

  test('keeps both rows when the two differ, since the one the daemon uses is not the one in the repo', () => {
    const changed = view('mine', { type: 'repo' }, { where: 'false' })
    expect(rowsOf([personal, changed]).map((row) => row.source.type)).toEqual(['personal', 'repo'])
  })

  test('keeps a personal view with no repo view, and a repo view with no personal one', () => {
    expect(rowsOf([personal]).map((row) => row.source.type)).toEqual(['personal'])
    expect(rowsOf([repo]).map((row) => row.source.type)).toEqual(['repo'])
  })
})

describe('the fold of the bar', () => {
  test('starts open, and is kept for each board', () => {
    const store = memoryStore()
    expect(readBarFolded(store, 'b-alpha')).toBe(false)
    setBarFolded(store, 'b-alpha', true)
    expect(readBarFolded(store, 'b-alpha')).toBe(true)
    expect(readBarFolded(store, 'b-beta')).toBe(false)
    setBarFolded(store, 'b-alpha', false)
    expect(readBarFolded(store, 'b-alpha')).toBe(false)
  })

  test('is open when there is no store, or the store fails', () => {
    expect(readBarFolded(null, 'b-alpha')).toBe(false)
    const broken = {
      getItem: () => {
        throw new Error('no storage')
      },
      setItem: () => {
        throw new Error('no storage')
      },
    }
    expect(readBarFolded(broken, 'b-alpha')).toBe(false)
    expect(() => setBarFolded(broken, 'b-alpha', true)).not.toThrow()
  })
})
