// Scripted worlds for the Field (ts-0181-ft): a `World` of `loop-world.ts` that also answers `task.list` and
// `task.views` over plain data, as the daemon does. A world says which sources the board declares, the tasks, the
// views with the keys each holds and what each reports. The fake-owner page and the vitest tests share them.
import type {
  TaskChangesResultOutput,
  TaskDiffParamsInput,
  TaskDiffResultOutput,
  TaskGetParamsInput,
  TaskGetResultOutput,
  TaskListResultOutput,
  TaskOutput,
  TaskViewsResultOutput,
} from '../../src/owner/generated.ts'
import { OwnerError } from '../../src/owner/index.ts'
import { HOSTILE_BODY } from './hostile.ts'
import { tailOf } from '../../src/field/model.ts'
import { passes } from './cel-lite.ts'
import { World, board } from './loop-world.ts'

export type Category = TaskOutput['state']['category']
export type SourceDecl = TaskListResultOutput['sources'][number]
export type ViewEntry = TaskViewsResultOutput['views'][number]
export type Report = NonNullable<TaskListResultOutput['view']>
export type Copy = TaskGetResultOutput['copies'][number]
export type HookLink = TaskGetResultOutput['links'][number]
export type LinkType = NonNullable<TaskGetResultOutput['link_types']>[number]

/** What `task.diff` is asked of: a run or a commit, the file, and the patch or the whole file. */
export const diffKey = (params: Pick<TaskDiffParamsInput, 'run' | 'commit' | 'path' | 'whole'>): string =>
  `${params.run ?? params.commit}|${params.path}|${params.whole ? 'whole' : 'patch'}`

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

/** The entry `task.views` lists for a view that was saved. */
function viewEntry(view: ViewEntry['view'], target: 'repo' | 'personal', count: number): ViewEntry {
  return { error_count: 0, source: { type: target }, task_count: count, view }
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
  /** What the sources list as collections, on the first page of `task.list`. */
  collections: TaskListResultOutput['collections'] = []
  /**
   * What the daemon answers when `task.list` is asked for an inline view with a closure, by the closure's goal. A goal
   * with none here matches no task, and the daemon refuses the view as it does then.
   */
  walks = new Map<string, Held>()
  /** What `task.changes` answers, by task key. A task with none here has no run and no commit. */
  changes = new Map<string, TaskChangesResultOutput>()
  /** What `task.diff` answers, by `diffKey`. A file with none here is `not found`, as a file no run changed is. */
  fileDiffs = new Map<string, TaskDiffResultOutput>()
  /** Set to make `task.changes` refuse with these words. */
  refuseChanges: string | null = null
  /** The board's line has a Collide folder, so a view can be shared (`task.views`). */
  shared = false
  /** Set to make `task.list` refuse a `filter` with these words. */
  refuseFilter: string | null = null

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

  /** The daemon's event for a run: an edit of its workspace sends none, a phase and the run's state do. */
  announceRun(run: string, event: 'run.transition' | 'phase.transition' = 'run.transition') {
    const body =
      event === 'run.transition'
        ? { event, run, from: null, to: { state: 'running' } }
        : { event, run, phase: 'agent', round: 1, from: null, to: { state: 'running' }, log: null, transcript: null }
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
    // `id` may be the short form (`12` or `ts-0001-aa`), as the daemon takes it: an exact key first, then a key's tail.
    const exact = this.listing.filter((each) => each.key === params.id)
    const entries = exact.length > 0 ? exact : this.listing.filter((each) => tailOf(each.key) === params.id)
    const [first] = entries
    if (first === undefined) return super.getTask(params)
    const keys = [...new Set(entries.map((each) => each.key))]
    if (keys.length > 1) return { ambiguous: keys, copies: [], collisions: [], runs: [], links: [] }
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
    this.owner.handle('task.views', () => ({ shared: this.shared, views: this.views }))
    this.owner.handle('task.save_view', ({ view, target, overwrite }) => {
      const found = this.views.find((each) => each.view.name === view.name && each.source.type === target)
      if (found !== undefined && overwrite !== true) throw new OwnerError(-32000, `a view named ${view.name} exists`)
      const keys = this.listing.filter((each) => view.where == null || passes(view.where, each)).map((each) => each.key)
      this.views = [...this.views.filter((each) => each !== found), viewEntry(view, target, keys.length)]
      this.held.set(view.name, { keys, report: report(view.name, keys.length) })
      return { path: `${target}/${view.name}.yaml` }
    })
    this.owner.handle('task.changes', ({ task }) => {
      if (this.refuseChanges !== null) throw new OwnerError(-32000, this.refuseChanges)
      const how = `no commit on this line names ${tailOf(task)}`
      const place = 'Your checkout, line main'
      return this.changes.get(task) ?? { runs: [], commits: [], commits_words: { place, how } }
    })
    this.owner.handle('task.diff', (params) => {
      const found = this.fileDiffs.get(diffKey(params))
      if (found === undefined) throw new OwnerError(-32010, `${params.path} has no change to show`)
      return found
    })
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
      if (ref !== undefined && ref !== null && ref.type === 'inline') {
        const goal = ref.view.closure?.goal ?? ''
        const held = this.walks.get(goal)
        if (held === undefined) throw new Error(`\`closure.goal\` \`${goal}\` matches no task`)
        tasks = tasks.filter((each) => held.keys.includes(each.key))
        view = { ...held.report, name: ref.view.name }
      }
      if (params.filter !== undefined && params.filter !== null) {
        if (this.refuseFilter !== null) throw new OwnerError(-32602, `filter: ${this.refuseFilter}`)
        const filter = params.filter
        tasks = tasks.filter((each) => passes(filter, each))
        const members = new Set(tasks.map((each) => each.key))
        const kept = (view?.steps ?? []).map((step) => step.filter((key) => members.has(key)))
        const steps = kept.filter((step) => step.length > 0)
        view = { ...(view ?? report('filter', 0)), members: members.size, steps }
      }
      return {
        // The daemon's listing has no body: `task.get` gives the whole task.
        tasks: tasks.map((each) => ({ ...each, body: undefined })),
        collections: this.collections,
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

export const EPIC_FIELD = 'plan:ep-006-wq'
export const EPIC_LATER = 'plan:ep-007-xx'
export const EPIC_DONE = 'plan:ep-008-yy'
export const EPIC_KNOT = 'plan:ep-009-zz'

/**
 * The run order and graph's world (ts-0187-hb): the plan with four epics as collections. One has a diamond of tasks, a
 * task whose blocker is finished, a task that waits on something that is not a task, and a task with a part-of link.
 * One has nothing open, and one has two tasks that wait on each other. Views: one with no closure, one with the goal
 * of a task, and two that open in a layout of their own. `walks` answers the inline view of each goal that has tasks.
 */
export function walkWorld(): FieldWorld {
  const world = new FieldWorld()
  world.sources = [PLAN]
  const blocked = (...to: string[]) => to.map((each) => ({ type: 'blocked-by', to: each }))
  const inField = { collections: [EPIC_FIELD] }
  world.collections = [
    { id: EPIC_FIELD, title: 'The Field', groups: [] },
    { id: EPIC_LATER, title: 'Later work', groups: [] },
    { id: EPIC_DONE, title: 'A finished epic', groups: [] },
    { id: EPIC_KNOT, title: 'Tied in a knot', groups: [] },
  ]
  world.listing = [
    task('plan:ts-0001-aa', 'Write the parser', 'ready', 'ready', { ...inField, links: blocked('plan:ts-0006-ff') }),
    task('plan:ts-0002-bb', 'Design the board', 'ready', 'ready', { ...inField, links: blocked('plan:ts-0001-aa') }),
    task('plan:ts-0003-cc', 'Build the Field', 'active', 'active', {
      ...inField,
      links: blocked('plan:ts-0001-aa', 'plan:ts-0002-bb', 'plan:ts-0099-zz'),
    }),
    task('plan:ts-0004-dd', 'Review the wire', 'ready', 'ready', {
      ...inField,
      links: [...blocked('plan:ts-0003-cc', 'plan:ts-0013-mm'), { type: 'part-of', to: 'ep-006-wq' }],
    }),
    task('plan:ts-0005-ee', 'Plan the drawer', 'ready', 'ready', { collections: [EPIC_LATER] }),
    task('plan:ts-0006-ff', 'Ship the shell', 'done', 'done', inField),
    task('plan:ts-0007-gg', 'Tidy up', 'done', 'done', { collections: [EPIC_DONE] }),
    task('plan:ts-0010-jj', 'Chicken', 'ready', 'ready', {
      collections: [EPIC_KNOT],
      links: blocked('plan:ts-0011-kk'),
    }),
    task('plan:ts-0011-kk', 'Egg', 'ready', 'ready', {
      collections: [EPIC_KNOT],
      links: blocked('plan:ts-0010-jj'),
    }),
    task('plan:ts-0012-ll', 'Omelette', 'ready', 'ready', {
      collections: [EPIC_KNOT],
      links: blocked('plan:ts-0011-kk'),
    }),
    task('plan:ts-0013-mm', 'Draw the tabs', 'ready', 'ready', inField),
  ]
  const keys = (...shorts: string[]) => shorts.map((short) => `plan:ts-${short}`)
  const field = keys('0001-aa', '0002-bb', '0003-cc', '0004-dd', '0013-mm')
  const fieldSteps = [keys('0001-aa', '0013-mm'), keys('0002-bb'), keys('0003-cc'), keys('0004-dd')]
  world.walks.set(EPIC_FIELD, { keys: field, report: report('x', 5, { steps: fieldSteps }) })
  world.walks.set(EPIC_LATER, { keys: keys('0005-ee'), report: report('x', 1, { steps: [keys('0005-ee')] }) })
  const knot = keys('0010-jj', '0011-kk', '0012-ll')
  world.walks.set(EPIC_KNOT, {
    keys: knot,
    report: report('x', 3, { steps: [knot], cycle: keys('0010-jj', '0011-kk') }),
  })
  const open = world.listing.filter((each) => each.state.category !== 'done').map((each) => each.key)
  const board = (name: string, over: Partial<ViewEntry['view']> = {}) => {
    world.views.push(view(name, { type: 'repo' }, over))
    world.held.set(name, { keys: open, report: report(name, open.length) })
  }
  board('everything')
  board('graph-first', { layout: { default: 'graph' } })
  board('run-first', { layout: { default: 'run-order' } })
  const closure = { goal: 'plan:ts-0004-dd', follow: 'depends_on' as const }
  world.views.push(view('to-the-wire', { type: 'repo' }, { closure }))
  world.held.set('to-the-wire', { keys: field, report: report('to-the-wire', 5, { steps: fieldSteps }) })
  return world
}

const EPIC_ONE = 'plan:ep-001-aa'
const EPIC_TWO = 'plan:ep-002-bb'

/**
 * A board for the filter bar (ts-0190-tz): two sources, two epics, labels on some tasks, a line with a Collide
 * folder, a built-in view, one that names a task, one that has a `where`, and one of the person's own.
 */
export function filtersWorld(): FieldWorld {
  const world = new FieldWorld()
  world.sources = [PLAN, eight()]
  world.shared = true
  world.collections = [
    { id: EPIC_ONE, title: 'The Field', groups: [] },
    { id: EPIC_TWO, title: 'The drawer', groups: [] },
  ]
  world.listing = [
    task('plan:ts-0001-aa', 'Write the parser', 'ready', 'ready', { collections: [EPIC_ONE], labels: ['perf'] }),
    task('plan:ts-0002-bb', 'Design the board', 'ready', 'ready', { collections: [EPIC_ONE], labels: ['perf', 'ux'] }),
    task('plan:ts-0003-cc', 'Build the Field', 'active', 'active', { collections: [EPIC_TWO] }),
    task('plan:ts-0005-ee', 'Plan the drawer', 'review', 'review', { collections: [EPIC_TWO] }),
    task('plan:ts-0006-ff', 'Ship the shell', 'done', 'done', { collections: [EPIC_ONE] }),
    task('eight:1', 'Think about it', 'idea', 'backlog'),
    task('eight:5', 'Parse the old notes', 'queued', 'ready'),
  ]
  const keys = (include: (each: TaskOutput) => boolean) => world.listing.filter(include).map((each) => each.key)
  const open = keys((each) => each.state.category !== 'done')
  world.views = [
    view('ready', { type: 'builtin' }),
    view('open-work', { type: 'repo' }, { where: 'category in ["ready", "active", "review"]' }),
    view('picked', { type: 'repo' }, { include: ['plan:ts-0001-aa'] }),
    view('mine', { type: 'personal' }, { where: 'category in ["ready"]', layout: { groupBy: 'category' } }),
  ]
  world.held.set('ready', { keys: keys((each) => each.state.category === 'ready'), report: report('ready', 3) })
  world.held.set('open-work', { keys: open, report: report('open-work', open.length) })
  world.held.set('picked', { keys: ['plan:ts-0001-aa'], report: report('picked', 1) })
  world.held.set('mine', { keys: keys((each) => each.state.category === 'ready'), report: report('mine', 3) })
  return world
}

/** Fifty epics, so the Epic box has more than it can show without a find box and a scroll. */
export function manyEpicsWorld(): FieldWorld {
  const world = new FieldWorld()
  world.sources = [PLAN]
  const number = (index: number) => String(index + 1).padStart(3, '0')
  world.collections = Array.from({ length: 50 }, (_, index) => ({
    id: `plan:ep-${number(index)}-aa`,
    title: `Epic ${number(index)} ${index % 2 === 0 ? 'Field work' : 'Daemon work'}`,
    groups: [],
  }))
  world.listing = world.collections.flatMap((collection, index) => [
    task(`plan:ts-${number(index)}-aa`, `Task of epic ${number(index)}`, 'ready', 'ready', {
      collections: [collection.id],
    }),
  ])
  return world
}
