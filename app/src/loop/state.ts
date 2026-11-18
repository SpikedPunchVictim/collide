// The thin-loop page's state and its reducer (ts-0081-wm). Stage 9a replaces the page, so the reducer sits beside it
// and the board UI's (`../ui/`) stays as it is. The reducer is plain functions over plain data, so its tests need no
// browser (0.15). What the page does that calls the owner is in `loop.ts`, which dispatches these actions.
import type {
  AgentVersions,
  BoardSummary,
  DiffResult,
  FilePatch,
  Followed,
  QueueItem,
  RunSummary,
} from './types.ts'

export interface LoopState {
  /** True until the first load has finished. */
  readonly loading: boolean
  /** A load that failed, in words. It stays until a load works. */
  readonly problem: string | null
  /** Whether the daemon's event stream is up. While it is down the page may be behind. */
  readonly connection: 'live' | 'lost'
  readonly boards: readonly BoardSummary[]
  /** The board a new prompt goes to. */
  readonly boardId: string | null
  /** Why the last folder was not opened, as Collide's own words. */
  readonly folderProblems: readonly string[]
  /** The `claude` entry of `daemon.agent_versions`, or null when the daemon gave none. */
  readonly versions: AgentVersions | null
  readonly followed: Followed | null
  readonly run: RunSummary | null
  readonly items: readonly QueueItem[]
  readonly diff: DiffResult | null
  readonly picked: string | null
  readonly patch: FilePatch | null
  /** A start or an answer is on its way. */
  readonly sending: boolean
  /** The daemon's reason for an answer it declined, or the failure of a call. The item stays. */
  readonly refusal: string | null
  /** What the last answer did, in words. */
  readonly notice: string | null
  /** A re-prompt went through. Its notice outlives the run change to the new run, until that run needs the person or ends. `from` is the run that was sent back. */
  readonly sentBack: { readonly from: string | null } | null
  /** Discard was pressed once: the page asks again, and says what is deleted. */
  readonly confirmingDiscard: boolean
}

export function initialLoopState(): LoopState {
  return {
    loading: true,
    problem: null,
    connection: 'live',
    boards: [],
    boardId: null,
    folderProblems: [],
    versions: null,
    followed: null,
    run: null,
    items: [],
    diff: null,
    picked: null,
    patch: null,
    sending: false,
    refusal: null,
    notice: null,
    sentBack: null,
    confirmingDiscard: false,
  }
}

export type LoopAction =
  /** One reload of what the daemon holds. `boards` and `versions` are left out by a reload that does not ask. */
  | {
      type: 'synced'
      boards?: readonly BoardSummary[]
      versions?: AgentVersions | null
      items: readonly QueueItem[]
      followed: Followed | null
      run: RunSummary | null
      diff: DiffResult | null
    }
  | { type: 'sync_failed'; problem: string }
  | { type: 'connection'; state: 'live' | 'lost' }
  | { type: 'board_chosen'; id: string }
  | { type: 'board_opened'; board: BoardSummary }
  | { type: 'folder_refused'; problems: readonly string[] }
  | { type: 'sending' }
  | { type: 'run_started'; followed: Followed }
  | { type: 'refused'; message: string }
  | { type: 'answered'; notice: string | null; sentBack: boolean }
  | { type: 'file_picked'; path: string | null }
  | { type: 'patch_loaded'; patch: FilePatch }
  | { type: 'discard_asked' }
  | { type: 'discard_kept' }

export function loopReducer(state: LoopState, action: LoopAction): LoopState {
  switch (action.type) {
    case 'synced':
      return synced(state, action)
    case 'sync_failed':
      return { ...state, loading: false, problem: action.problem }
    case 'connection':
      return { ...state, connection: action.state }
    case 'board_chosen':
      return { ...state, boardId: action.id, folderProblems: [] }
    case 'board_opened': {
      const known = state.boards.some((b) => b.id === action.board.id)
      const boards = known
        ? state.boards.map((b) => (b.id === action.board.id ? action.board : b))
        : [...state.boards, action.board]
      return { ...state, boards, boardId: action.board.id, folderProblems: [] }
    }
    case 'folder_refused':
      return { ...state, folderProblems: action.problems }
    case 'sending':
      return {
        ...state,
        sending: true,
        refusal: null,
        notice: null,
        sentBack: null,
        problem: null,
        confirmingDiscard: false,
      }
    case 'run_started':
      return { ...afterRunChange(state), followed: action.followed, sending: false }
    case 'refused':
      return { ...state, sending: false, refusal: action.message }
    case 'answered':
      return {
        ...state,
        sending: false,
        notice: action.notice,
        sentBack: action.sentBack ? { from: state.run?.id ?? null } : null,
      }
    case 'file_picked':
      return { ...state, picked: action.path, patch: action.path === null ? null : state.patch }
    case 'patch_loaded':
      return state.picked === action.patch.path ? { ...state, patch: action.patch } : state
    case 'discard_asked':
      return { ...state, confirmingDiscard: true }
    case 'discard_kept':
      return { ...state, confirmingDiscard: false }
  }
}

/** What belongs to one run, which another run does not share. A re-prompt's notice is kept: the new run is still working. */
function afterRunChange(state: LoopState): LoopState {
  return {
    ...state,
    run: null,
    items: [],
    diff: null,
    picked: null,
    patch: null,
    notice: state.sentBack === null ? null : state.notice,
    refusal: null,
    confirmingDiscard: false,
  }
}

function synced(state: LoopState, action: Extract<LoopAction, { type: 'synced' }>): LoopState {
  const boards = action.boards ?? state.boards
  const changed = action.followed?.run !== state.followed?.run
  const base = changed ? afterRunChange(state) : state
  const stillThere = base.boardId !== null && boards.some((b) => b.id === base.boardId)
  const followedBoard = changed ? (action.followed?.board ?? null) : null
  const boardId = followedBoard ?? (stillThere ? base.boardId : (boards[0]?.id ?? null))
  const picked = action.diff?.files.some((f) => f.path === base.picked) ? base.picked : null
  const sentBackDone = reachedPerson(base.sentBack, action.items, action.run)
  return {
    ...base,
    loading: false,
    problem: null,
    boards,
    boardId,
    versions: action.versions === undefined ? base.versions : action.versions,
    items: action.items,
    followed: action.followed,
    run: action.run,
    diff: action.diff,
    picked,
    patch: picked === null ? null : base.patch,
    // A reload that leaves the run as it was leaves what the person was told about it.
    confirmingDiscard: base.confirmingDiscard && reviewWaits(action.items, action.run),
    notice: sentBackDone ? null : base.notice,
    sentBack: sentBackDone ? null : base.sentBack,
  }
}

/** Whether the run a re-prompt started has reached a state that needs the person, or has ended. The run sent back does not count. */
function reachedPerson(
  sentBack: LoopState['sentBack'],
  items: readonly QueueItem[],
  run: RunSummary | null,
): boolean {
  if (sentBack === null || run === null || run.id === sentBack.from) return false
  if (run.state.state === 'ended') return true
  return items.some((i) => (i.kind === 'review' || i.kind === 'permission') && i.run === run.id)
}

/** Whether a review item waits on the run, which is what a question about discarding is about. */
function reviewWaits(items: readonly QueueItem[], run: RunSummary | null): boolean {
  return items.some((i) => i.kind === 'review' && i.run === run?.id)
}
