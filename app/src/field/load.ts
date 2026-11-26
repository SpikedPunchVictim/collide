// What the Field reads from the owner (ts-0181-ft): `task.views` for the picker, and `task.list` for the board.
import type { OwnerClient } from '../owner/client.ts'
import type { FieldData, ListResult, ViewEntry } from './types.ts'
import type { ViewInput as View } from '../owner/generated.ts'

/**
 * The board's views, the built-in ones first and then the others in the order the daemon lists them, and whether the
 * board's line has a Collide folder, which a personal view can be shared into.
 */
export async function loadBoardViews(owner: OwnerClient, boardId: string) {
  const { views, shared } = await owner.rpc('task.views', { board: boardId })
  const builtin = (entry: ViewEntry) => (entry.source.type === 'builtin' ? 0 : 1)
  return { views: [...views].sort((a, b) => builtin(a) - builtin(b)), shared }
}

export async function loadViews(owner: OwnerClient, boardId: string): Promise<ViewEntry[]> {
  return (await loadBoardViews(owner, boardId)).views
}

/** Every page of a listing: the board-wide lists are on the first, the tasks are on all. */
async function listAll(
  owner: OwnerClient,
  boardId: string,
  view: string | null,
  filter: string | null = null,
): Promise<ListResult> {
  const ask = (cursor?: string | null) =>
    owner.rpc('task.list', {
      board: boardId,
      ...(cursor === undefined || cursor === null ? {} : { cursor }),
      ...(view === null ? {} : { view: { type: 'named' as const, name: view } }),
      ...(filter === null ? {} : { filter }),
    })
  const first = await ask()
  const tasks = [...first.tasks]
  let cursor = first.next_cursor
  while (cursor !== undefined && cursor !== null) {
    const page = await ask(cursor)
    tasks.push(...page.tasks)
    cursor = page.next_cursor
  }
  return { ...first, tasks }
}

/**
 * What the daemon says of a view with a goal's closure added: its report, whose steps and cycle the run order and graph
 * draw. The tasks are on the board already, so one is asked for. A walk the daemon refuses (the goal is gone, or the
 * view does not compile) is words to show, not a failure of the board. The drawer's epic asks the same of the whole
 * board.
 */
export async function loadWalk(owner: OwnerClient, boardId: string, walk: View, filter: string | null) {
  try {
    const answer = await owner.rpc('task.list', {
      board: boardId,
      view: { type: 'inline', view: walk },
      limit: 1,
      ...(filter === null ? {} : { filter }),
    })
    return { report: answer.view ?? null, problem: null }
  } catch (failure) {
    return { report: null, problem: words(failure) }
  }
}

const words = (failure: unknown) => (failure instanceof Error ? failure.message : String(failure))

/**
 * The board through a view, or whole when `view` is null, narrowed by the filter bar's expression when there is one
 * (`filters/model.ts`). A view lists only its members, but a card says how many tasks of the board it waits on, so the
 * board is read as well when a view is. The groups of the bar are counted over the view as it was before the filter,
 * so a filter reads the view a second time; with none the view is read once, and the counts are over what is shown.
 * A filter the daemon refuses is words to show, and no task: a board that showed the view whole would not be what the
 * bar says.
 */
export async function loadField(
  owner: OwnerClient,
  boardId: string,
  view: string | null,
  walk: View | null = null,
  filter: string | null = null,
): Promise<FieldData> {
  const all = await listAll(owner, boardId, null)
  const inView = view === null ? all : await listAll(owner, boardId, view)
  let shown = inView
  let filterProblem: string | null = null
  if (filter !== null) {
    try {
      shown = await listAll(owner, boardId, view, filter)
    } catch (failure) {
      shown = { ...inView, tasks: [], view: inView.view }
      filterProblem = words(failure)
    }
  }
  const walked = walk === null ? { report: null, problem: null } : await loadWalk(owner, boardId, walk, filter)
  return {
    tasks: shown.tasks,
    inView: inView.tasks,
    corpus: all.tasks,
    sources: all.sources,
    collisions: all.collisions,
    divergence: all.divergence,
    branchOnly: all.branch_only,
    report: shown.view ?? null,
    collections: all.collections,
    walked: walked.report,
    walkProblem: walked.problem,
    filterProblem,
    settling: all.refreshing || all.not_started.length > 0,
  }
}
