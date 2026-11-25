// Scripted worlds for the Field (ts-0181-ft): a `World` of `loop-world.ts` that also answers `task.list` and
// `task.views` over plain data, as the daemon does. A world says which sources the board declares, the tasks, the
// views with the keys each holds and what each reports. The fake-owner page and the vitest tests share them.
import type { TaskListResultOutput, TaskOutput, TaskViewsResultOutput } from '../../src/owner/generated.ts'
import { World, board } from './loop-world.ts'

export type Category = TaskOutput['state']['category']
export type SourceDecl = TaskListResultOutput['sources'][number]
export type ViewEntry = TaskViewsResultOutput['views'][number]
export type Report = NonNullable<TaskListResultOutput['view']>

export const CATEGORY_ORDER: readonly Category[] = ['backlog', 'ready', 'active', 'review', 'done', 'dropped']

/** A task as a source lists it: its key is `source:short`, and its category is the one its status maps to. */
export function task(key: string, title: string, status: string, category: Category, over: Partial<TaskOutput> = {}) {
  const [, short] = key.split(':') as [string, string]
  return {
    aliases: [],
    collections: [],
    fields: {},
    id: short,
    key,
    labels: [],
    links: [],
    owners: [],
    state: { category, status },
    templateFields: {},
    title,
    version: 'v1',
    ...over,
  } satisfies TaskOutput
}

/** The plan's own source: its statuses are its categories but `backlog`, so a column is a category by another name. */
const PLAN_CATEGORIES = CATEGORY_ORDER.filter((category) => category !== 'backlog')

export const PLAN: SourceDecl = {
  name: 'plan',
  pack: 'collide/plan',
  scope: 'ref',
  statuses: PLAN_CATEGORIES.map((category) => ({ name: category, category })),
  category_status: Object.fromEntries(PLAN_CATEGORIES.map((category) => [category, category])),
  fields: [],
  blocking_links: true,
  id_pattern: '^[a-z]{2}-[0-9]{4}-[a-z]{2}$',
  short_id_pattern: '^[0-9]+(-[a-z]{2})?$',
}

/** A source with eight statuses, two of them in `review`: the fixture of `host/tests/daemon_corpus.rs`. */
export function eight(names: readonly string[] = EIGHT_NAMES): SourceDecl {
  return {
    name: 'eight',
    pack: 'octo',
    scope: 'ref',
    statuses: names.map((name, index) => ({ name, category: EIGHT_CATEGORIES[index]! })),
    category_status: { ready: names[2]!, active: names[3]!, review: names[4]! },
    fields: [],
    blocking_links: true,
    id_pattern: '^[a-z0-9-]+$',
    short_id_pattern: '^[0-9]+$',
  }
}

export const EIGHT_NAMES = ['idea', 'triage', 'queued', 'doing', 'in_review', 'qa', 'done', 'dropped'] as const
export const EIGHT_CATEGORIES: readonly Category[] = [
  'backlog',
  'backlog',
  'ready',
  'active',
  'review',
  'review',
  'done',
  'dropped',
]

export const SCRATCH: SourceDecl = {
  name: 'scratch',
  scope: 'global',
  statuses: CATEGORY_ORDER.map((category) => ({ name: category, category })),
  category_status: Object.fromEntries(CATEGORY_ORDER.map((category) => [category, category])),
  fields: [],
  blocking_links: false,
  id_pattern: '^[0-9]+$',
  short_id_pattern: '^[0-9]+$',
}

export function view(name: string, source: ViewEntry['source'], over: Partial<ViewEntry['view']> = {}): ViewEntry {
  return { error_count: 0, source, task_count: 0, view: { name, ...over } }
}

export function report(name: string, members: number, over: Partial<Report> = {}): Report {
  return {
    name,
    members,
    cycle: [],
    errors: {},
    invalid_items: 0,
    not_reached: 0,
    over_task_budget: 0,
    pass_budget_exhausted: false,
    steps: [],
    unmatched_exclude: [],
    unmatched_include: [],
    ...over,
  }
}

/** What one view holds, and what it says of itself. */
export interface Held {
  readonly keys: readonly string[]
  readonly report: Report
}

export class FieldWorld extends World {
  sources: SourceDecl[] = []
  listing: TaskOutput[] = []
  views: ViewEntry[] = []
  held = new Map<string, Held>()
  collisions: TaskListResultOutput['collisions'] = []
  divergence: TaskListResultOutput['divergence'] = []
  /** What the daemon says while its first listing runs, and of a source that did not start. */
  refreshing = false
  notStarted: TaskListResultOutput['not_started'] = []
  /** Set to make `task.list` refuse with these words. */
  refuseList: string | null = null

  /** Changes a listed task, as an edit in a checkout would. Nothing is announced: the daemon's event does that. */
  edit(key: string, over: Partial<TaskOutput>): void {
    this.listing = this.listing.map((each) => (each.key === key ? { ...each, ...over } : each))
  }

  /** The daemon's event that a task of `source` changed, as the corpus service records it. */
  announce(source: string, key = `${source}:ts-0001-aa`, event: 'task.changed' | 'task.removed' = 'task.changed') {
    const task = { key, version: 'v2', cause: 'worktree' as const, checkout: '/work/alpha' }
    const body =
      event === 'task.changed'
        ? { event, source, task: { ...task, from: null, to: null, diff: {} } }
        : { event, source, task: { key, cause: task.cause, checkout: task.checkout } }
    this.owner.push({
      method: 'daemon.event',
      params: { seq: 1, wall_ms: 1, actor: { kind: 'workflow' }, experiment: null, body },
    } as never)
  }

  /** How many times `task.list` was called. */
  lists(): number {
    return this.callsTo('task.list').length
  }

  constructor() {
    super()
    this.boards = [board('b-alpha', '/work/alpha')]
    this.owner.handle('task.views', () => ({ views: this.views }))
    this.owner.handle('task.list', (params) => {
      if (this.refuseList !== null) throw new Error(this.refuseList)
      const ref = params.view
      let tasks = this.listing
      let view: Report | undefined
      if (ref !== undefined && ref !== null && ref.type === 'named') {
        const held = this.held.get(ref.name)
        if (held === undefined) throw new Error(`no view named ${ref.name}`)
        tasks = tasks.filter((each) => held.keys.includes(each.key))
        view = held.report
      }
      return {
        tasks,
        collections: [],
        collisions: this.collisions,
        divergence: this.divergence,
        divergent: this.divergence.map((entry) => entry.key),
        invalid: [],
        not_started: this.notStarted,
        refreshing: this.refreshing,
        sources: this.sources,
        sources_started: this.sources.length,
        stale_sources: [],
        ...(view === undefined ? {} : { view }),
      }
    })
  }
}

const LINK = (to: string) => ({ type: 'blocked-by', to })

/** One source, the plan's: a task in each category, and every thing a card can say. */
export function onePlanSource(): FieldWorld {
  const world = new FieldWorld()
  world.sources = [PLAN]
  world.listing = [
    task('plan:ts-0001-aa', 'Write the parser', 'ready', 'ready'),
    task('plan:ts-0002-bb', 'Design the board', 'ready', 'ready', {
      links: [{ type: 'part-of', to: 'ep-006-wq' }, LINK('plan:ts-0001-aa'), LINK('ts-0006-ff')],
    }),
    task('plan:ts-0003-cc', 'Build the Field', 'active', 'active', { links: [{ type: 'part-of', to: 'ep-006-wq' }] }),
    task('plan:ts-0004-dd', 'Review the wire', 'ready', 'ready'),
    task('plan:ts-0005-ee', 'Plan the drawer', 'review', 'review'),
    task('plan:ts-0006-ff', 'Ship the shell', 'done', 'done'),
    task('plan:ts-0007-gg', 'Draw the tabs', 'done', 'done'),
    task('plan:ts-0008-hh', 'A thing not done', 'dropped', 'dropped'),
    task('plan:ts-0009-ii', 'Same short id, one place', 'ready', 'ready', { path: 'a/ts-0009-ii.md' }),
    task('plan:ts-0009-ii', 'Same short id, another place', 'ready', 'ready', { path: 'b/ts-0009-ii.md' }),
  ]
  world.divergence = [
    {
      key: 'plan:ts-0004-dd',
      copies: [{ status: 'review', category: 'review', checkout: '/work/alpha-run', run: 'r-121' }],
    },
    { key: 'plan:ts-0005-ee', copies: [{ status: 'done', category: 'done', checkout: '/work/wt-a' }] },
    // A collided key is never shown as a difference, even when the daemon lists one.
    { key: 'plan:ts-0009-ii', copies: [{ status: 'active', category: 'active', checkout: '/work/wt-b' }] },
  ]
  world.collisions = [
    {
      shortId: 'ts-0009-ii',
      locations: [
        { checkout: '/work/alpha', path: 'a/ts-0009-ii.md' },
        { checkout: '/work/alpha', path: 'b/ts-0009-ii.md' },
      ],
    },
  ]
  const everything = world.listing.map((each) => each.key)
  world.views = [
    view('ready', { type: 'builtin' }, { where: 'ready' }),
    view('collide/plan/spikes', { type: 'pack', pack: 'collide/plan' }),
    view('mine', { type: 'repo' }),
    view('by-status', { type: 'repo' }, { layout: { groupBy: 'status' } }),
    view('by-category', { type: 'repo' }, { layout: { groupBy: 'category' } }),
    view('cyclic', { type: 'repo' }),
  ]
  world.held.set('ready', {
    keys: ['plan:ts-0004-dd', 'plan:ts-0006-ff'],
    report: report('ready', 2, {
      cycle: ['plan:ts-0004-dd', 'plan:ts-0005-ee'],
      unknown_blockers: [{ from: 'plan:ts-0003-cc', to: 'ts-9999-zz' }],
    }),
  })
  world.held.set('collide/plan/spikes', { keys: [], report: report('collide/plan/spikes', 0) })
  world.held.set('mine', {
    keys: ['plan:ts-0001-aa', 'plan:ts-0002-bb'],
    report: report('mine', 2, { unmatched_include: ['github:acme/edge#212'] }),
  })
  world.held.set('by-status', { keys: everything, report: report('by-status', everything.length) })
  world.held.set('by-category', { keys: everything, report: report('by-category', everything.length) })
  world.held.set('cyclic', {
    keys: ['plan:ts-0001-aa'],
    report: report('cyclic', 1, { error: 'where: no such field `colour`' }),
  })
  return world
}

/** The eight-status source alone: eight columns, in its order. */
export function eightStatuses(names: readonly string[] = EIGHT_NAMES): FieldWorld {
  const world = new FieldWorld()
  world.sources = [eight(names)]
  world.listing = [
    task('eight:1', 'Think about it', names[0]!, 'backlog'),
    task('eight:2', 'Check it twice', names[5]!, 'review'),
    task('eight:3', 'Check it once', names[4]!, 'review'),
    task('eight:4', 'Finished work', names[6]!, 'done'),
    task('eight:5', 'Waits on the unfinished', names[2]!, 'ready', { links: [LINK('eight:1')] }),
  ]
  world.views = [view('ready', { type: 'builtin' })]
  world.held.set('ready', { keys: ['eight:5'], report: report('ready', 1) })
  return world
}

/** The eight-status source and the plan source in one board: a mixed view shows the six categories. */
export function mixedSources(): FieldWorld {
  const world = new FieldWorld()
  world.sources = [eight(), PLAN]
  world.listing = [
    task('eight:2', 'Check it twice', 'qa', 'review'),
    task('eight:4', 'Finished work', 'done', 'done'),
    task('plan:ts-0001-aa', 'Write the parser', 'ready', 'ready'),
    task('plan:ts-0003-cc', 'Build the Field', 'active', 'active'),
  ]
  world.views = [view('only-eight', { type: 'repo' }), view('both', { type: 'repo' })]
  world.held.set('only-eight', { keys: ['eight:2', 'eight:4'], report: report('only-eight', 2) })
  world.held.set('both', { keys: world.listing.map((each) => each.key), report: report('both', 4) })
  return world
}
