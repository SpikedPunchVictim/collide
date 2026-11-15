// Every user action and every in-app agent tool call is one member of `UiAction` (0.15; 0.11). The reducer switches
// over `type` with an exhaustiveness check, so a member without a branch fails `tsc --noEmit`.
import type {
  ActionSource,
  Corpus,
  FieldFilter,
  SurfaceId,
  ToolName,
  TracksFilter,
  UiState,
  ViewSurface,
} from './state.ts'

/** A person's actions are typed by the window itself, so the reducer trusts their shape. */
export type UserAction =
  | { readonly type: 'user/corpus_loaded'; readonly corpus: Corpus }
  | { readonly type: 'user/surface_shown'; readonly surface: SurfaceId }
  | { readonly type: 'user/task_toggled'; readonly id: string }
  | { readonly type: 'user/filter_set'; readonly surface: 'field'; readonly filter: FieldFilter }
  | { readonly type: 'user/filter_set'; readonly surface: 'tracks'; readonly filter: TracksFilter }
  | { readonly type: 'user/lens_cleared'; readonly surface: ViewSurface }
  | { readonly type: 'user/undo' }

/**
 * One call of an agent tool. `input` is what the agent sent, and it is untrusted (0.9): the reducer validates it
 * against the state before it applies anything, and a call that fails changes nothing. The tools' definitions and
 * the mapping from an MCP `tools/call` to this action are Stage 6 and 6b.
 */
export type AgentAction = {
  readonly [T in ToolName]: {
    readonly type: `agent/${T}`
    readonly source: ActionSource
    /** When the call was made, as an ISO time. It comes with the action so the reducer stays a pure function. */
    readonly at: string
    readonly input: unknown
  }
}[ToolName]

export type UiAction = UserAction | AgentAction
export type ActionType = UiAction['type']

/** Why an action was refused. The message tells the agent what to do and never holds a token or a path (0.9). */
export interface ActionError {
  readonly code: 'invalid_input' | 'unknown_id' | 'duplicate_id' | 'unknown_action'
  readonly message: string
}

/**
 * What applying an action gives. `state` is always the state to keep: on a refusal it is the very state that came in,
 * and `changed` is false when an action was valid and altered nothing, so that it left no chip and no snapshot.
 */
export type ActionResult =
  | { readonly ok: true; readonly state: UiState; readonly changed: boolean }
  | { readonly ok: false; readonly state: UiState; readonly error: ActionError }
