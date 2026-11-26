// What the Planning tab shows, as plain functions over what `task.list` and `task.sections` gave (ts-0195-ju; 0.11,
// the Planning surface; note 2 of the board mock). Nothing here reads the owner or the page, so each rule is tested
// alone. The plan's own kinds and its `implements` link are named here, as the CLI's text view names them
// (`host/src/task_view.rs`); no status name is read, only a task's category and a proposal's six statuses, which core
// derives a feature's state from (`core/src/feature.rs`, `derived_state`).
import { type EpicCard, boardOf, cardOf, distinct, progressWords, rankOf, tasksIn } from '../epics/model.ts'
import { indexOf, openBlockers, shortIdOf, sourceOf, tailOf } from '../field/model.ts'
import type { FieldData, Task } from '../field/types.ts'
import type { SectionsOutput } from '../owner/generated.ts'

export type Sections = SectionsOutput

/** The kind of a proposal, and the link type with which a task builds one of its features. */
export const PROPOSAL = 'proposal'
const IMPLEMENTS = 'implements'

const isOpen = (task: Task) => task.state.category !== 'done' && task.state.category !== 'dropped'

/** What a source reports of a task that names no kind is a plain task. */
const isTask = (task: Task) => task.kind === undefined || task.kind === null || task.kind === 'task'

const plural = (count: number, one: string, many: string) => `${count} ${count === 1 ? one : many}`

// ---- the epics -----------------------------------------------------------------------------------------------------

/** An epic's tile: the strip's own card, and the next task of the epic that can start. */
export interface Tile {
  readonly card: EpicCard
  readonly next: Task | null
}

/**
 * One tile for each epic the board lists, in the strip's order (open work first, then by id), counted over the whole
 * board as the strip counts. The next task is the first of the epic's tasks that is ready and waits on nothing open.
 */
export function tilesOf(data: FieldData): Tile[] {
  const board = boardOf(data)
  const index = indexOf(board)
  return data.collections
    .map((collection) => cardOf(collection, board))
    .sort((a, b) => rankOf(a) - rankOf(b) || a.id.localeCompare(b.id))
    .map((card) => ({
      card,
      next:
        tasksIn(board, card.id).find(
          (task) => isTask(task) && task.state.category === 'ready' && openBlockers(task, data.sources, index) === 0,
        ) ?? null,
    }))
}

/** "2 of 5 done", and "· next: <title>" when a task can start. */
export function tileWords({ card, next }: Tile): string {
  const done = progressWords(card).done
  return next === null ? done : `${done} · next: ${next.title}`
}

// ---- the proposals -------------------------------------------------------------------------------------------------

export interface ProposalRow {
  readonly key: string
  readonly title: string
  readonly status: string
  /** Not done and not dropped. */
  readonly open: boolean
  /** The sections its features are under, the unsorted ones not counted. */
  readonly sections: number
  readonly features: number
  /** "only on branch <branch>" for a proposal that is in no checkout of the default line, else null. */
  readonly only: string | null
}

const folder = (path: string) => path.split(/[\\/]/).filter((part) => part !== '').pop() ?? path

/** Where a branch-only task is: its branch, or else the name of its checkout. */
function onlyWords(key: string, data: FieldData): string | null {
  const found = data.branchOnly.find((each) => each.key === key)
  if (found === undefined) return null
  return found.branch == null ? `only in ${folder(found.checkout)}` : `only on branch ${found.branch}`
}

/** A section's key: what makes two names one section (`core/src/section.rs`). Empty for a feature that is unsorted. */
const sectionKey = (name: string) => name.trim().toLowerCase()

/** The proposals of the board, in the order the board lists them. */
export function proposalsOf(data: FieldData): ProposalRow[] {
  return distinct(boardOf(data))
    .filter((task) => task.kind === PROPOSAL)
    .map((task) => {
      const features = task.features ?? []
      const sections = new Set(features.map((feature) => sectionKey(feature.section)).filter((key) => key !== ''))
      return {
        key: task.key,
        title: task.title,
        status: task.state.status,
        open: isOpen(task),
        sections: sections.size,
        features: features.length,
        only: onlyWords(task.key, data),
      }
    })
}

/** "draft · 3 sections, 4 features". */
export function proposalWords(row: ProposalRow): string {
  const counts = [plural(row.sections, 'section', 'sections'), plural(row.features, 'feature', 'features')]
  return `${row.status} · ${counts.join(', ')}`
}

/** The proposal Readiness shows until one is picked: the first that is open, else the first. */
export function defaultProposal(rows: readonly ProposalRow[]): string | null {
  return (rows.find((row) => row.open) ?? rows[0])?.key ?? null
}

// ---- the features --------------------------------------------------------------------------------------------------

export interface FeatureRow {
  /** The proposal's key and the feature, as `plan:pr-0001-aa#F1`. */
  readonly id: string
  /** The section as the daemon names it; "Unsorted" for a feature with none. */
  readonly section: string
  readonly unsorted: boolean
  readonly featureId: string
  readonly title: string
  /** The proposal's title. */
  readonly proposal: string
  readonly proposalKey: string
  /** What the feature has come to, derived: `planned`, `accepted`, "1 of 2 done", `done`. */
  readonly state: string
  /** The tasks that implement it and are not dropped. */
  readonly tasks: readonly { readonly key: string; readonly shortId: string }[]
  /** The other names of sections that are near-duplicates of this one. */
  readonly looksLike: readonly string[]
}

export const UNSORTED = 'Unsorted'

/** A feature's state from its proposal's status and the categories of the tasks that implement it. */
function stateOf(status: string, tasks: readonly Task[]): string {
  switch (status) {
    case 'draft':
    case 'proposed':
      return 'planned'
    case 'deferred':
      return 'deferred'
    case 'rejected':
    case 'superseded':
      return 'dropped'
    case 'accepted': {
      if (tasks.length === 0) return 'accepted'
      const done = tasks.filter((task) => task.state.category === 'done').length
      return done === tasks.length ? 'done' : `${done} of ${tasks.length} done`
    }
    default:
      return 'unknown'
  }
}

/** The tasks that are not dropped and implement each feature, by the feature's id (`plan:pr-0001-aa#F1`). */
function implementers(board: readonly Task[], proposals: readonly Task[]): Map<string, Task[]> {
  const names = new Map<string, Task>()
  for (const proposal of proposals) {
    for (const name of [proposal.key, proposal.id, tailOf(proposal.key), ...proposal.aliases]) {
      if (!names.has(name)) names.set(name, proposal)
    }
  }
  const found = new Map<string, Task[]>()
  for (const task of board) {
    if (task.state.category === 'dropped') continue
    for (const link of task.links) {
      if (link.type !== IMPLEMENTS) continue
      const [item, feature] = link.to.split('#')
      if (item === undefined || feature === undefined) continue
      const proposal = names.get(item) ?? names.get(`${sourceOf(task.key)}:${item}`)
      if (proposal === undefined) continue
      const id = `${proposal.key}#${feature}`
      found.set(id, [...(found.get(id) ?? []), task])
    }
  }
  return found
}

/** Section names in the order the rows are drawn: the daemon's order when it is known, else as they first appear. */
function orderOf(keys: readonly string[], sections: Sections | null): string[] {
  const seen = [...new Set(keys)]
  const first = seen.filter((key) => key !== '')
  const own = seen.includes('') ? [...first, ''] : first
  if (sections === null) return own
  const told = sections.sections.map((each) => sectionKey(each.name))
  return [...told.filter((key) => seen.includes(key)), ...own.filter((key) => !told.includes(key))]
}

/**
 * The features of the proposals that are not dropped, grouped by section: in the order of `task.sections` (the biggest
 * section first, the unsorted ones last), and each section's features in the order of their proposals. The daemon's
 * count for a section is the number of rows under it. Without the sections the order is the order the sections first
 * appear in.
 */
export function featureRowsOf(data: FieldData, sections: Sections | null): FeatureRow[] {
  const board = distinct(boardOf(data))
  const proposals = board.filter((task) => task.kind === PROPOSAL && task.state.category !== 'dropped')
  const built = implementers(board, proposals)
  const names = new Map((sections?.sections ?? []).map((each) => [sectionKey(each.name), each.name]))
  const rows = proposals.flatMap((proposal) =>
    (proposal.features ?? []).map((feature) => {
      const id = `${proposal.key}#${feature.id}`
      const tasks = built.get(id) ?? []
      const key = sectionKey(feature.section)
      const named = names.get(key) ?? feature.section.trim()
      return {
        key,
        row: {
          id,
          section: key === '' ? UNSORTED : named,
          unsorted: key === '',
          featureId: feature.id,
          title: feature.title,
          proposal: proposal.title,
          proposalKey: proposal.key,
          state: stateOf(proposal.state.status, tasks),
          tasks: tasks.map((task) => ({ key: task.key, shortId: shortIdOf(task, data.sources) })),
          looksLike: (sections?.nearDuplicates ?? [])
            .filter((group) => group.names.includes(named))
            .flatMap((group) => group.names.filter((name) => name !== named)),
        } satisfies FeatureRow,
      }
    }),
  )
  return orderOf(
    rows.map((each) => each.key),
    sections,
  ).flatMap((key) => rows.filter((each) => each.key === key).map((each) => each.row))
}

// ---- readiness -----------------------------------------------------------------------------------------------------

export interface ReadinessLine {
  readonly id: 'features' | 'unsorted' | 'coverage' | 'assumptions' | 'decisions'
  readonly text: string
  readonly ok: boolean
  /** What is not right, or an empty string. */
  readonly why: string
}

const ids = (list: readonly string[], one: string, many: string) =>
  list.length === 0 ? '' : `${list.join(', ')} ${list.length === 1 ? one : many}`

/**
 * The Readiness checklist of a proposal (0.11, Plan…), which is advice and enforces nothing. It holds the lines that
 * the proposal's own parts can answer. The lines about spikes, questions and a red-team pass wait for the planning
 * sessions of Stage 9d.
 */
export function readinessOf(proposal: Task): ReadinessLine[] {
  const problems = (proposal.featureProblems ?? []).map((each) => `line ${each.line}: ${each.reason}`)
  const unsorted = (proposal.features ?? []).filter((each) => sectionKey(each.section) === '').map((each) => each.id)
  const uncovered = [
    ids(proposal.coverage?.featuresWithoutTask ?? [], 'has no task', 'have no task'),
    ids(proposal.coverage?.tasksWithoutFeature ?? [], 'has no feature', 'have no feature'),
  ].filter((each) => each !== '')
  const unproven = (proposal.assumptions ?? []).filter(
    (each) => each.state !== 'verified' && each.state !== 'accepted risk',
  )
  const unweighed = (proposal.decisions ?? []).filter((each) => each.passedOn.length === 0).map((each) => each.id)
  const line = (id: ReadinessLine['id'], text: string, why: string): ReadinessLine => ({
    id,
    text,
    ok: why === '',
    why,
  })
  return [
    line('features', 'The Features part has no problems', problems.join('; ')),
    line('unsorted', 'No feature is Unsorted', unsorted.join(', ')),
    line('coverage', 'Every feature has a task, and every included task has a feature', uncovered.join('; ')),
    line(
      'assumptions',
      'Each assumption is verified, or an accepted risk',
      unproven.map((each) => `${each.id} is ${each.state}`).join('; '),
    ),
    line('decisions', 'Every decision names an alternative it passed on', ids(unweighed, 'names none', 'name none')),
  ]
}
