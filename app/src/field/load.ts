// What the Field reads from the owner (ts-0181-ft): `task.views` for the picker, and `task.list` for the board.
import type { OwnerClient } from '../owner/client.ts'
import type { FieldData, ListResult, ViewEntry } from './types.ts'

/** The board's views, the built-in ones first and then the others in the order the daemon lists them. */
export async function loadViews(owner: OwnerClient, boardId: string): Promise<ViewEntry[]> {
  const { views } = await owner.rpc('task.views', { board: boardId })
  const builtin = (entry: ViewEntry) => (entry.source.type === 'builtin' ? 0 : 1)
  return [...views].sort((a, b) => builtin(a) - builtin(b))
}

/** Every page of a listing: the board-wide lists are on the first, the tasks are on all. */
async function listAll(owner: OwnerClient, boardId: string, view: string | null): Promise<ListResult> {
  const first = await owner.rpc('task.list', {
    board: boardId,
    ...(view === null ? {} : { view: { type: 'named' as const, name: view } }),
  })
  const tasks = [...first.tasks]
  let cursor = first.next_cursor
  while (cursor !== undefined && cursor !== null) {
    const page = await owner.rpc('task.list', {
      board: boardId,
      cursor,
      ...(view === null ? {} : { view: { type: 'named' as const, name: view } }),
    })
    tasks.push(...page.tasks)
    cursor = page.next_cursor
  }
  return { ...first, tasks }
}

/**
 * The board through a view, or whole when `view` is null. A view lists only its members, but a card says how many
 * tasks of the board it waits on, so the board is read as well when a view is.
 */
export async function loadField(owner: OwnerClient, boardId: string, view: string | null): Promise<FieldData> {
  const all = await listAll(owner, boardId, null)
  const shown = view === null ? all : await listAll(owner, boardId, view)
  return {
    tasks: shown.tasks,
    corpus: all.tasks,
    sources: all.sources,
    collisions: all.collisions,
    divergence: all.divergence,
    report: shown.view ?? null,
  }
}
