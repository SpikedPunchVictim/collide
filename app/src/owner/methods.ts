// The methods the window may call over `rpc`, with the type of each one's params and result. The params are the
// generated `<Name>Input` (what a caller may send) and the result the generated `<Name>Output` (what comes back), so
// a method is written once, and a name that is not a generated type fails the typecheck. `test/owner-methods.test.ts`
// keeps `OWNER_METHODS` equal to the `Method` list of `core/src/owner.rs`, less `initialize`.
//
// Left out on purpose:
// - `initialize`: it carries the per-client token. The shell makes the handshake, so the window has no way to send it.
// - `ask.start` and `ask.test` in the daemon's shape: that holds the ask token and the endpoint, which the shell adds.
//   The window's shape is `ASK_METHODS`.
import type * as G from './generated.ts'

/** The types of one method, carried by a value so that a table of these is both a list of names and a table of types. */
export interface Call<Params, Result> {
  readonly params: Params
  readonly result: Result
}

const call = <Params, Result>(): Call<Params, Result> => null as never

export const OWNER_METHODS = {
  'daemon.status': call<G.DaemonStatusParamsInput, G.DaemonStatusResultOutput>(),
  'daemon.recapture_environment': call<G.DaemonRecaptureEnvironmentParamsInput, G.DaemonRecaptureEnvironmentResultOutput>(),
  'daemon.agent_versions': call<G.DaemonAgentVersionsParamsInput, G.DaemonAgentVersionsResultOutput>(),
  'daemon.subscribe': call<G.DaemonSubscribeParamsInput, G.DaemonSubscribeResultOutput>(),
  'board.list': call<G.BoardListParamsInput, G.BoardListResultOutput>(),
  'board.open': call<G.BoardOpenParamsInput, G.BoardOpenResultOutput>(),
  'board.checks': call<G.BoardChecksParamsInput, G.BoardChecksResultOutput>(),
  'board.locate': call<G.BoardLocateParamsInput, G.BoardLocateResultOutput>(),
  'board.init': call<G.BoardInitParamsInput, G.BoardInitResultOutput>(),
  'board.trust': call<G.BoardTrustParamsInput, G.BoardTrustResultOutput>(),
  'board.trust_review': call<G.BoardTrustReviewParamsInput, G.BoardTrustReviewResultOutput>(),
  'task.list': call<G.TaskListParamsInput, G.TaskListResultOutput>(),
  'task.get': call<G.TaskGetParamsInput, G.TaskGetResultOutput>(),
  'task.apply': call<G.TaskApplyParamsInput, G.TaskApplyResultOutput>(),
  'task.create': call<G.TaskCreateParamsInput, G.TaskApplyResultOutput>(),
  'task.reid': call<G.TaskReidParamsInput, G.TaskReidResultOutput>(),
  'task.preview': call<G.TaskPreviewParamsInput, G.PreviewResultOutput>(),
  'task.views': call<G.TaskViewsParamsInput, G.TaskViewsResultOutput>(),
  'task.save_view': call<G.TaskSaveViewParamsInput, G.TaskSaveViewResultOutput>(),
  'task.sections': call<G.TaskSectionsParamsInput, G.SectionsOutput>(),
  'task.changes': call<G.TaskChangesParamsInput, G.TaskChangesResultOutput>(),
  'task.diff': call<G.TaskDiffParamsInput, G.TaskDiffResultOutput>(),
  'run.start': call<G.RunStartParamsInput, G.RunStartResultOutput>(),
  // `RunPreviewParams` is a type alias of `RunStartParams` in `core`, so it has no schema of its own.
  'run.preview': call<G.RunStartParamsInput, G.RunPreviewResultOutput>(),
  'run.start_file': call<G.RunStartFileParamsInput, G.RunStartResultOutput>(),
  'run.list': call<G.RunListParamsInput, G.RunListResultOutput>(),
  'run.get': call<G.RunGetParamsInput, G.RunGetResultOutput>(),
  'run.record': call<G.RunRecordParamsInput, G.RunRecordResultOutput>(),
  'run.stop': call<G.RunStopParamsInput, G.RunStopResultOutput>(),
  'run.diff': call<G.RunDiffParamsInput, G.RunDiffResultOutput>(),
  'run.output': call<G.RunOutputParamsInput, G.RunOutputResultOutput>(),
  'run.intervene': call<G.RunIntervenParamsInput, G.AckOutput>(),
  'run.take_over': call<G.RunTakeOverParamsInput, G.RunTakeOverResultOutput>(),
  'run.terminal_input': call<G.RunTerminalInputParamsInput, G.AckOutput>(),
  'run.terminal_resize': call<G.RunTerminalResizeParamsInput, G.AckOutput>(),
  'run.detach': call<G.RunDetachParamsInput, G.AckOutput>(),
  'run.end_take_over': call<G.RunEndTakeOverParamsInput, G.AckOutput>(),
  'queue.list': call<G.QueueListParamsInput, G.QueueListResultOutput>(),
  'queue.context': call<G.QueueContextParamsInput, G.ContextRecordOutput>(),
  'queue.answer': call<G.QueueAnswerParamsInput, G.QueueAnswerResultOutput>(),
  'workspace.list': call<G.WorkspaceListParamsInput, G.WorkspaceListResultOutput>(),
  'workspace.remove': call<G.WorkspaceRemoveParamsInput, G.WorkspaceRemoveResultOutput>(),
  'workspace.resume': call<G.WorkspaceResumeParamsInput, G.WorkspaceResumeResultOutput>(),
  'config.get': call<G.ConfigGetParamsInput, G.ConfigGetResultOutput>(),
  'config.set': call<G.ConfigSetParamsInput, G.ConfigSetResultOutput>(),
  'config.pack_list': call<G.ConfigPackListParamsInput, G.ConfigPackListResultOutput>(),
  'config.pack_add': call<G.ConfigPackAddParamsInput, G.ConfigPackResultOutput>(),
  'config.pack_link': call<G.ConfigPackLinkParamsInput, G.ConfigPackResultOutput>(),
  'config.pack_enable': call<G.ConfigPackEnableParamsInput, G.ConfigPackResultOutput>(),
  'config.pack_update': call<G.ConfigPackUpdateParamsInput, G.ConfigPackUpdateResultOutput>(),
  'config.reset': call<G.ConfigResetParamsInput, G.ConfigResetResultOutput>(),
}

// The ask methods, in the window's shape. They are not in `OWNER_METHODS`: the owner protocol answers `method not
// found` to them until Stage 6b, and the window's params differ from the daemon's (they hold no token).
export const ASK_METHODS = {
  'ask.start': call<G.AskStartParamsInput, G.AckOutput>(),
  'ask.cancel': call<G.AskCancelParamsInput, G.AckOutput>(),
  'ask.test': call<G.AskTestParamsInput, G.AskTestResultOutput>(),
}

// The shell's own methods. It answers them and never forwards them (`ts-0035-re`).
export const UI_TOOLS_METHODS = {
  'ui_tools.reply': call<G.UiToolsReplyParamsInput, G.AckOutput>(),
  'ui_tools.token': call<G.UiToolsTokenParamsInput, G.UiToolsTokenResultOutput>(),
  'ui_tools.rotate': call<G.UiToolsRotateParamsInput, G.UiToolsRotateResultOutput>(),
}

// The shell's own methods that are not the UI-tools endpoint's: it answers them and never forwards them
// (`ts-0183-ee`). Their types are written here, not generated, because only the shell has them.
export const SHELL_METHODS = {
  /** Opens a link in the person's browser. The shell opens only `http:`, `https:` and `mailto:`. */
  'shell.open_external': call<{ url: string }, G.AckOutput>(),
  /**
   * Asks the person for a folder with the system's picker (`ts-0191-nv`). `path` is null when they cancel. The bridge
   * and the fake owner have no picker, and the window shows the picker only in the shell.
   */
  'shell.pick_folder': call<Record<never, never>, { path: string | null }>(),
  /**
   * Tells the shell that this window started with defaults (`true`), or left that mode (`false`) (`ts-0215-gt`). The
   * window cannot send `initialize`, so the shell makes its connection to the daemon with the flag. The bridge's shell
   * and the fake owner answer it too.
   */
  'shell.set_defaults': call<{ defaults: boolean }, G.AckOutput>(),
}
