// The worlds for Changes, the diff window and the id peek (ts-0189-fg): a `FieldWorld` that also answers `task.changes`
// and `task.diff` over plain data, as the daemon does. The fake-owner page and the vitest tests share them.
import type { TaskChangesResultOutput, TaskDiffResultOutput } from '../../src/owner/generated.ts'
import { FieldWorld, PLAN, diffKey, task } from './field-world.ts'

type RunChanges = TaskChangesResultOutput['runs'][number]
type CommitChanges = TaskChangesResultOutput['commits'][number]
type DiffFile = RunChanges['files'][number]
type Whole = NonNullable<TaskDiffResultOutput['whole']>

export const PLACE = 'Your checkout, line main'

/** A changed file with its counts. A file with no counts is binary. */
export function file(
  path: string,
  additions: number | null,
  deletions: number | null,
  status: DiffFile['status'] = 'modified',
): DiffFile {
  const counts = additions === null || deletions === null ? {} : { additions, deletions }
  return { path, status, ...counts }
}

/** A run's entry as `task.changes` gives it. A worktree run, running, with nothing listed, unless `over` says. */
export function runEntry(run: string, over: Partial<RunChanges> = {}): RunChanges {
  return {
    run,
    state: { state: 'running' },
    place: 'worktree',
    workspace: `/tmp/work/${run}`,
    words: {
      place: `Run ${run}'s worktree`,
      how: 'since the agent phase began, including edits not committed yet',
    },
    files: [],
    collapsed: [],
    more: 0,
    ...over,
  }
}

export function commitEntry(
  id: string,
  subject: string,
  files: DiffFile[],
  over: Partial<CommitChanges> = {},
): CommitChanges {
  return { id, subject, date_ms: Date.UTC(2026, 10, 20, 10, 0), files, collapsed: [], more: 0, ...over }
}

export function changesOf(
  runs: RunChanges[],
  commits: CommitChanges[] = [],
  how = 'no commit on this line names the task',
): TaskChangesResultOutput {
  return { runs, commits, commits_words: { place: PLACE, how } }
}

/** `n` lines of `line 1`, `line 2`, and so on, with the lines in `changed` marked as the changes made them. */
export function wholeOf(n: number, changed: readonly number[]): Whole {
  const lines = Array.from({ length: n }, (_, at) => ({
    n: at + 1,
    text: `line ${at + 1}`,
    changed: changed.includes(at + 1),
  }))
  return { binary: false, bytes: n * 8, lines, truncated: false }
}

/** A patch with two hunks: the first changes line 2, the second adds a line at 41. */
export const PARSER_PATCH = [
  'diff --git a/src/parser.ts b/src/parser.ts',
  'index 1111111..2222222 100644',
  '--- a/src/parser.ts',
  '+++ b/src/parser.ts',
  '@@ -1,4 +1,4 @@',
  " import { lex } from './lexer'",
  '-export const VERSION = 1',
  '+export const VERSION = 2',
  ' ',
  ' export function parse(text: string) {',
  '@@ -40,3 +40,4 @@ function tail() {',
  '   return 1',
  '+  // a new line',
  ' }',
  '',
].join('\n')

export const RUN_LIVE = 'r-12'
export const RUN_DONE = 'r-9'
export const COMMIT = 'abc1234def5678abc1234def5678abc1234def56'

type Target = { run: string } | { commit: string }

function addDiffs(world: FieldWorld, target: Target, path: string, text: string, whole: Whole) {
  const run = 'run' in target ? target.run : null
  const commit = 'commit' in target ? target.commit : null
  world.fileDiffs.set(diffKey({ run, commit, path, whole: false }), {
    path,
    patch: { path, text, bytes: text.length, truncated: false },
  })
  world.fileDiffs.set(diffKey({ run, commit, path, whole: true }), { path, whole })
}

/**
 * The Changes world (ts-0189-fg): the plan's tasks with, for each, what `task.changes` says. `ts-0001-aa` has a live
 * run with files, a finished run and a commit made by hand. `ts-0002-bb` has a run from before heads were kept.
 * `ts-0003-cc` has nothing. `ts-0004-dd` has a run in the person's checkout. `ts-0005-ee` has a commit and no run.
 */
export function changesWorld(): FieldWorld {
  const world = new FieldWorld()
  world.sources = [PLAN]
  const open = (short: string, title: string, status: 'ready' | 'active') =>
    task(`plan:${short}`, title, status, status, { body: `Body of ${short}.`, checkout: '/work/alpha' })
  world.listing = [
    open('ts-0001-aa', 'Write the parser', 'active'),
    open('ts-0002-bb', 'Design the board', 'ready'),
    open('ts-0003-cc', 'Build the Field', 'ready'),
    open('ts-0004-dd', 'Review the wire', 'ready'),
    open('ts-0005-ee', 'Plan the drawer', 'ready'),
  ]
  const live = runEntry(RUN_LIVE, {
    files: [
      file('src/parser.ts', 30, 4),
      file('docs/guide.md', 12, 0, 'added'),
      file('assets/logo.png', null, null, 'added'),
    ],
    collapsed: [{ path: 'node_modules', files: 1203, bytes: 4_500_000 }],
    more: 3,
  })
  const done = runEntry(RUN_DONE, {
    state: { state: 'ended', outcome: 'done' },
    words: { place: `Run ${RUN_DONE}'s worktree`, how: 'from where the agent phase began to where it ended' },
    files: [file('src/lexer.ts', 10, 2)],
  })
  const typo = commitEntry(COMMIT, 'Fix the typo that ts-0001-aa left in the README', [file('README.md', 1, 1)])
  world.changes.set(
    'plan:ts-0001-aa',
    changesOf([live, done], [typo], '1 commit that names ts-0001-aa: abc1234'),
  )
  const old = runEntry('r-3', {
    state: { state: 'ended', outcome: 'done' },
    words: { place: "Run r-3's worktree", how: 'this run kept no record of where it began' },
    note: 'no changes recorded',
  })
  world.changes.set('plan:ts-0002-bb', changesOf([old]))
  const mine = runEntry('r-20', {
    place: 'checkout',
    workspace: '/work/alpha',
    words: {
      place: 'Your checkout',
      how: 'since the agent phase began, including edits not committed yet and your own edits',
    },
    files: [file('src/wire.ts', 5, 1)],
  })
  world.changes.set('plan:ts-0004-dd', changesOf([mine]))
  const drawn = commitEntry('def5678abc1234def5678abc1234def5678abc1', 'Draw the drawer ts-0005-ee', [
    file('a.txt', 1, 0, 'added'),
  ])
  const how = '1 commit that names ts-0005-ee: def5678. No run built it.'
  world.changes.set('plan:ts-0005-ee', changesOf([], [drawn], how))
  addDiffs(world, { run: RUN_LIVE }, 'src/parser.ts', PARSER_PATCH, wholeOf(200, [150, 180]))
  addDiffs(world, { run: RUN_DONE }, 'src/lexer.ts', '@@ -1 +1 @@\n-old\n+new\n', wholeOf(5, [1]))
  addDiffs(world, { commit: COMMIT }, 'README.md', '@@ -1 +1 @@\n-Teh\n+The\n', wholeOf(3, [1]))
  // A file whose patch and whole text were cut, and one that is binary.
  const cut = { run: RUN_LIVE, commit: null, path: 'docs/guide.md' }
  world.fileDiffs.set(diffKey({ ...cut, whole: false }), {
    path: 'docs/guide.md',
    patch: { path: 'docs/guide.md', text: '@@ -0,0 +1,2 @@\n+one\n+two\n', bytes: 9_999_999, truncated: true },
    note: 'the patch is 9999999 bytes and is cut at 262144',
  })
  world.fileDiffs.set(diffKey({ ...cut, whole: true }), {
    path: 'docs/guide.md',
    whole: { ...wholeOf(3, [1, 2, 3]), truncated: true, bytes: 9_999_999 },
    note: 'the file is longer than the byte cap and is cut there',
  })
  const binary = { run: RUN_LIVE, commit: null, path: 'assets/logo.png' }
  world.fileDiffs.set(diffKey({ ...binary, whole: true }), {
    path: 'assets/logo.png',
    whole: { binary: true, bytes: 2048, lines: [], truncated: false },
    note: 'the file is not text',
  })
  world.fileDiffs.set(diffKey({ ...binary, whole: false }), {
    path: 'assets/logo.png',
    patch: {
      path: 'assets/logo.png',
      text: 'diff --git a/assets/logo.png b/assets/logo.png\nBinary files /dev/null and b/assets/logo.png differ\n',
      bytes: 96,
      truncated: false,
    },
  })
  return world
}

const PEEK_BODY = [
  'Start with ts-0002-bb, which draws it.',
  '',
  'Also `ts-0003-cc` in code, ts-9999-zz that is no task, version 1.2, and src/ts-0004-dd.md the file.',
  '',
  '[ts-0005-ee as link text](https://example.com/x)',
].join('\n')

/**
 * The peek's world (ts-0189-fg): plan tasks whose bodies name each other. `ts-0001-aa` names `ts-0002-bb`, which waits
 * on it and names `ts-0003-cc`, whose body is long enough to scroll and names `ts-0001-aa` again.
 */
export function peekWorld(): FieldWorld {
  const world = new FieldWorld()
  world.sources = [PLAN]
  const long = Array.from({ length: 60 }, (_, at) => `Line ${at + 1} of a long body.`).join('\n\n')
  const at = { checkout: '/work/alpha' }
  world.listing = [
    task('plan:ts-0001-aa', 'Write the parser', 'ready', 'ready', { body: PEEK_BODY, ...at }),
    task('plan:ts-0002-bb', 'Design the board', 'ready', 'ready', {
      body: 'Draw the board, then ask about ts-0003-cc.',
      links: [{ type: 'blocked-by', to: 'plan:ts-0001-aa' }],
      ...at,
    }),
    task('plan:ts-0003-cc', 'Build the Field', 'active', 'active', { body: `${long}\n\nBack to ts-0001-aa.`, ...at }),
    task('plan:ts-0004-dd', 'Review the wire', 'ready', 'ready', { body: 'A task nobody names.', ...at }),
    task('plan:ts-0005-ee', 'Plan the drawer', 'done', 'done', { body: 'Done long ago.', ...at }),
  ]
  return world
}
