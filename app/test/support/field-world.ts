// Scripted worlds for the Field (ts-0181-ft): a `World` of `loop-world.ts` that also answers `task.list` and
// `task.views` over plain data, as the daemon does. A world says which sources the board declares, the tasks, the
// views with the keys each holds and what each reports. The fake-owner page and the vitest tests share them.
import type {
  TaskGetParamsInput,
  TaskGetResultOutput,
  TaskListResultOutput,
  TaskOutput,
  TaskViewsResultOutput,
} from '../../src/owner/generated.ts'
import { HOSTILE_BODY } from './hostile.ts'
import { World, board } from './loop-world.ts'

export type Category = TaskOutput['state']['category']
export type SourceDecl = TaskListResultOutput['sources'][number]
export type ViewEntry = TaskViewsResultOutput['views'][number]
export type Report = NonNullable<TaskListResultOutput['view']>
export type Copy = TaskGetResultOutput['copies'][number]
export type HookLink = TaskGetResultOutput['links'][number]
export type LinkType = NonNullable<TaskGetResultOutput['link_types']>[number]

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
  /** Set to make `task.get` refuse with these words. */
  refuseGet: string | null = null
  /** The copies `task.get` lists for a key. A key with none here has its listed copies, one for each place it is at. */
  copies = new Map<string, Copy[]>()
  /** The link types a source declares, by source name. A source with none here has the plan's two if it blocks. */
  linkTypes = new Map<string, LinkType[]>()

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

  /** How many times `task.get` was called. */
  gets(): number {
    return this.callsTo('task.get').length
  }

  /**
   * `task.get` as the daemon answers it: the task with its body, the links one hop out and in with the other end's
   * title, the source's link types, the views that hold it and its copies. A key listed at two places is a collision,
   * and then there is no task: the answer names the key as ambiguous (`corpus.rs` `get`). A key the world does not
   * list is for `World` (the scratch tasks of a run).
   */
  protected override getTask(params: TaskGetParamsInput): TaskGetResultOutput {
    if (this.refuseGet !== null) throw new Error(this.refuseGet)
    const entries = this.listing.filter((each) => each.key === params.id)
    const [first] = entries
    if (first === undefined) return super.getTask(params)
    const copies = this.copies.get(first.key) ?? entries.map((each) => this.copyOf(each))
    if (entries.length > 1) {
      const tail = first.key.slice(first.key.indexOf(':') + 1)
      const collisions = this.collisions.filter((each) => each.shortId === tail)
      return { ambiguous: [first.key], copies, collisions, runs: [], links: [] }
    }
    const source = first.key.slice(0, first.key.indexOf(':'))
    const held = [...this.held].filter(([, each]) => each.keys.includes(first.key)).map(([name]) => name)
    return {
      task: first,
      ambiguous: [],
      copies,
      collisions: [],
      runs: [],
      links: this.linksOf(first),
      link_types: this.linkTypes.get(source) ?? this.defaultLinkTypes(source),
      views: held,
    }
  }

  private copyOf(task: TaskOutput): Copy {
    return {
      checkout: task.checkout ?? '/work/alpha',
      ...(task.path === undefined ? {} : { path: task.path }),
      state: task.state,
      version: task.version,
    }
  }

  private defaultLinkTypes(source: string): LinkType[] {
    const blocks = this.sources.find((each) => each.name === source)?.blocking_links ?? false
    if (!blocks) return []
    return [
      { name: 'blocked-by', dir: 'out', blocking: true, inverse: 'blocks' },
      { name: 'part-of', dir: 'out', blocking: false, inverse: 'contains' },
    ]
  }

  /** A link as `task.get` gives it: the other end's state and title, and `missing` for a key the board lacks. */
  private hookLink(type: string, dir: 'out' | 'in', key: string, other: TaskOutput | undefined): HookLink {
    if (other === undefined) {
      return { type, dir, key, kind: '', status: '', category: 'backlog', missing: true, stale: false }
    }
    return {
      type,
      dir,
      key: other.key,
      kind: other.kind ?? 'task',
      status: other.state.status,
      category: other.state.category,
      missing: false,
      stale: false,
      title: other.title,
    }
  }

  private linksOf(task: TaskOutput): HookLink[] {
    const source = task.key.slice(0, task.key.indexOf(':'))
    const keyOf = (to: string) => (to.includes(':') ? to : `${source}:${to}`)
    const out = task.links.map((link) => {
      const key = keyOf(link.to)
      return this.hookLink(link.type, 'out', key, this.listing.find((each) => each.key === key))
    })
    const incoming = this.listing.flatMap((holder) =>
      holder.links
        .filter((link) => keyOf(link.to) === task.key)
        .map((link) => this.hookLink(link.type, 'in', holder.key, holder)),
    )
    return [...out, ...incoming]
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
        // The daemon's listing has no body: `task.get` gives the whole task.
        tasks: tasks.map((each) => ({ ...each, body: undefined })),
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

/** A tracker outside the repo (scope `global`): one field with a label, one without, and one that is left empty. */
export const TRACKER: SourceDecl = {
  name: 'tracker',
  pack: 'demo/tracker',
  scope: 'global',
  statuses: CATEGORY_ORDER.map((category) => ({ name: category, category })),
  category_status: Object.fromEntries(CATEGORY_ORDER.map((category) => [category, category])),
  fields: [
    { id: 'priority', type: 'enum', filterable: true, label: 'Priority level' },
    { id: 'story_points', type: 'number', filterable: true },
    { id: 'component', type: 'string', filterable: false },
  ],
  blocking_links: true,
  id_pattern: '^[0-9]+$',
  short_id_pattern: '^[0-9]+$',
}

const PLAN_FILE = (short: string) => `.collide/planning/epics/ep-001-aa/tasks/${short}.md`
const PART_OF = { type: 'part-of', to: 'ep-001-aa' }

/**
 * The drawer's world (ts-0184-hv): the plan's tasks with bodies, links, an epic and a collection, a task whose copies
 * differ, an id held by two tasks, a scratch task, a tracker task with fields, and a task whose body is hostile.
 */
export function drawerWorld(): FieldWorld {
  const world = new FieldWorld()
  world.sources = [PLAN, SCRATCH, TRACKER]
  const at = (short: string, over: Partial<TaskOutput>): Partial<TaskOutput> => ({
    checkout: '/work/alpha',
    path: PLAN_FILE(short),
    ...over,
  })
  world.listing = [
    task('plan:ep-001-aa', 'The first epic', 'active', 'active', at('epic', { kind: 'epic' })),
    task(
      'plan:ts-0001-aa',
      'Write the parser',
      'ready',
      'ready',
      at('ts-0001-aa', {
        body: '## Goal\n\nParse the **files** of the plan.\n\n- one\n- two\n',
        links: [PART_OF],
        collections: ['stage-6a'],
      }),
    ),
    task(
      'plan:ts-0002-bb',
      'Design the board',
      'ready',
      'ready',
      at('ts-0002-bb', { body: 'Draw the board.', links: [PART_OF, { type: 'blocked-by', to: 'plan:ts-0001-aa' }] }),
    ),
    task(
      'plan:ts-0003-cc',
      'Build the Field',
      'active',
      'active',
      at('ts-0003-cc', { body: 'Build it.', links: [{ type: 'blocked-by', to: 'plan:ts-0002-bb' }] }),
    ),
    task('plan:ts-0004-dd', 'Review the wire', 'ready', 'ready', at('ts-0004-dd', { body: 'Look at the wire.' })),
    task(
      'plan:ts-0009-ii',
      'Same id, one place',
      'ready',
      'ready',
      at('a/ts-0009-ii', { path: 'a/ts-0009-ii.md', body: 'The first one.' }),
    ),
    task(
      'plan:ts-0009-ii',
      'Same id, another place',
      'active',
      'active',
      at('b/ts-0009-ii', { path: 'b/ts-0009-ii.md', checkout: '/work/wt-b', body: 'The other one.' }),
    ),
    task('plan:ts-0066-hh', 'A hostile body', 'ready', 'ready', at('ts-0066-hh', { body: HOSTILE_BODY })),
    task('scratch:1', 'Try the thing', 'backlog', 'backlog', { body: 'Just a note.' }),
    task('tracker:12', 'Fix the login', 'ready', 'ready', {
      body: 'Users cannot log in.',
      fields: { priority: 'high', component: '' },
    }),
  ]
  const copy = (checkout: string, status: 'ready' | 'review', version: string): Copy => ({
    checkout,
    path: PLAN_FILE('ts-0004-dd'),
    state: { category: status, status },
    version,
  })
  world.copies.set('plan:ts-0004-dd', [
    copy('/work/alpha', 'ready', 'v1'),
    copy('/work/alpha-run', 'review', 'v2'),
    copy('/work/wt-a', 'ready', 'v1'),
  ])
  world.divergence = [
    {
      key: 'plan:ts-0004-dd',
      copies: [{ status: 'review', category: 'review', checkout: '/work/alpha-run', run: 'r-121' }],
    },
  ]
  world.collisions = [
    {
      shortId: 'ts-0009-ii',
      locations: [
        { checkout: '/work/alpha', path: 'a/ts-0009-ii.md' },
        { checkout: '/work/wt-b', path: 'b/ts-0009-ii.md' },
      ],
    },
  ]
  world.views = [view('ready', { type: 'builtin' }, { where: 'ready' }), view('mine', { type: 'repo' })]
  world.held.set('ready', {
    keys: ['plan:ts-0001-aa', 'plan:ts-0004-dd'],
    report: report('ready', 2),
  })
  world.held.set('mine', { keys: ['plan:ts-0001-aa'], report: report('mine', 1) })
  return world
}
