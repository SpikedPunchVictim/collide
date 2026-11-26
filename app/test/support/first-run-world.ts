// Scripted worlds for the first-run page, the empty board, Share with team and the trust prompt (ts-0194-qp): a Field
// world that also answers `board.checks`, `board.init`, `board.trust_review` and `board.trust` over plain data, as the
// daemon does. A folder says what its checks are; a board says whether its line is trusted and what its Collide
// folder brings. The words of the checks are the daemon's (`host/src/first_run.rs`), so the page shows what it will
// show against the real daemon. The fake-owner page and the vitest tests share them.
import type {
  BoardChecksResultOutput,
  BoardInitResultOutput,
  BoardTrustReviewResultOutput,
} from '../../src/owner/generated.ts'
import { OwnerError } from '../../src/owner/index.ts'
import { FieldWorld, PLAN, SCRATCH, task } from './field-world.ts'
import { type Board, board } from './loop-world.ts'

export type Check = BoardChecksResultOutput['checks'][number]
export type Review = BoardTrustReviewResultOutput

export const INSTALL_COMMAND = 'curl -fsSL https://claude.ai/install.sh | bash'
const CONFLICT = -32011

const GIT_OK: Check = { id: 'git_repo', ok: true, required: true, message: 'This folder is a git repository.' }
const GIT_NO: Check = {
  id: 'git_repo',
  ok: false,
  required: true,
  message: 'This folder is not a git repository. Collide needs one: `git init` makes it one.',
}
const INSTALLED: Check = {
  id: 'claude_installed',
  ok: true,
  required: false,
  message: 'Claude Code is installed, version 2.1.330.',
}
const MISSING: Check = {
  id: 'claude_installed',
  ok: false,
  required: false,
  message: `Claude Code is not installed. Install it, then check again: \`${INSTALL_COMMAND}\``,
  copy: INSTALL_COMMAND,
}
const LOGGED_IN: Check = {
  id: 'claude_logged_in',
  ok: true,
  required: false,
  message: 'Claude Code is logged in.',
}
const LOGGED_OUT: Check = {
  id: 'claude_logged_in',
  ok: false,
  required: false,
  message:
    'Claude Code is not logged in. Run `claude` in a terminal and log in. You can open the board now; runs need it.',
  copy: 'claude',
}
const UNKNOWN: Check = {
  id: 'claude_logged_in',
  ok: true,
  required: false,
  could_not_check: true,
  message:
    'Could not check whether Claude Code is logged in: the check did not answer in time. Check again, or run ' +
    '`claude` in a terminal to see. You can open the board now; runs need it.',
}

/** The checks for a folder, in the daemon's order: git, then `claude`, then its login (left out when it is missing). */
export const CHECKS = {
  allPass: [GIT_OK, INSTALLED, LOGGED_IN],
  notARepo: [GIT_NO, INSTALLED, LOGGED_IN],
  noClaude: [GIT_OK, MISSING],
  loggedOut: [GIT_OK, INSTALLED, LOGGED_OUT],
  couldNotCheck: [GIT_OK, INSTALLED, UNKNOWN],
} as const satisfies Record<string, readonly Check[]>

/** What a review says for a repo that brings these, as `board.trust_review` does. */
export function reviewOf(brings: {
  packs?: readonly string[]
  views?: readonly string[]
  settings?: boolean
  hash?: string
}): Review {
  const changes: NonNullable<Review['changes']> = [
    { name: 'board.yaml', kind: 'added', lines: ['+packs:'] },
    ...(brings.views ?? []).map((name) => ({ name: `views/${name}.yaml`, kind: 'added' as const, lines: [] })),
    ...(brings.settings === true ? [{ name: 'config.yaml', kind: 'added' as const, lines: ['+config:'] }] : []),
  ]
  return {
    trust: { state: 'untrusted', hash: brings.hash ?? 'a'.repeat(40) },
    changes,
    packs: (brings.packs ?? []).map((source) => ({
      source,
      review: {
        name: source.split('/').pop() ?? source,
        version: '1.0.0',
        tree: '0123456789abcdef0123456789abcdef01234567',
        hooks: [],
        providers: [{ key: 'tasks', script: 'providers/tasks.mjs' }],
        observers: [],
        requires: ['tracker.example'],
      },
    })),
  }
}

export class FirstRunWorld extends FieldWorld {
  /** The checks of a folder. A folder the world has none for passes if it is a repo, and fails git if it is not. */
  checkSets = new Map<string, readonly Check[]>()
  /** What a folder's board is trusted as, by board id: set to make the board's line untrusted with this review. */
  reviews = new Map<string, Review>()
  /** The files a Share would write, as the dry run lists them. */
  shareFiles: string[] = ['.collide/board.yaml']
  /** Set to make `board.init` refuse with these words. */
  refuseInit: string | null = null
  /** The boards `board.init` has written, by id. The files wait for a commit, as they do in a repo. */
  written = new Set<string>()
  /** Set to make `board.trust` refuse with these words. */
  refuseTrust: string | null = null
  private madeBoards = 0

  constructor() {
    super()
    this.sources = [SCRATCH, PLAN]
    const owner = this.owner
    owner.handle('board.checks', ({ root }) => {
      const known = this.folders.get(root)
      const fallback = known !== undefined && 'id' in known ? CHECKS.allPass : CHECKS.notARepo
      return { checks: [...(this.checkSets.get(root) ?? fallback)] }
    })
    owner.handle('board.open', ({ root, init_git }) => {
      const known = this.folders.get(root)
      if (known === undefined) return { checks: [GIT_NO] }
      let opened: Board | undefined = 'id' in known ? known : undefined
      if (opened === undefined && init_git === true) {
        opened = board(`b-made-${++this.madeBoards}`, root)
        this.folders.set(root, opened)
      }
      if (opened === undefined) return { checks: [GIT_NO] }
      if (!this.boards.some((each) => each.id === opened!.id)) this.boards.push(opened)
      return { board: opened, checks: [...(this.checkSets.get(root) ?? CHECKS.allPass)] }
    })
    owner.handle('board.init', ({ board: id, dry_run }) => {
      if (this.refuseInit !== null) throw new OwnerError(CONFLICT, this.refuseInit)
      if (this.written.has(id)) {
        throw new OwnerError(CONFLICT, '`.collide` already exists, so `init` leaves it as it is')
      }
      const result: BoardInitResultOutput = { files: this.shareFiles, written: dry_run !== true }
      if (dry_run !== true) this.written.add(id)
      return result
    })
    owner.handle('board.trust_review', ({ board: id }) => this.reviews.get(id) ?? reviewOf({}))
    owner.handle('board.trust', ({ board: id, line, hash }) => {
      if (this.refuseTrust !== null) throw new OwnerError(CONFLICT, this.refuseTrust)
      this.boards = this.boards.map((each) =>
        each.id === id
          ? { ...each, lines: each.lines.map((l) => ({ ...l, trust: { state: 'trusted' as const, hash } })) }
          : each,
      )
      this.reviews.delete(id)
      return { trust: { state: 'trusted', hash } }
    })
  }

  /** A scratch task is listed at once, as the daemon's corpus lists it once the file is written. */
  protected override created(key: string, prompt: string): void {
    this.listing = [...this.listing, task(key, prompt, 'ready', 'ready')]
  }

  /** The files are committed: the board's line now has a Collide folder. */
  commit(): void {
    this.shared = true
  }

  /** The board's line is untrusted, and its Collide folder brings what `review` says. */
  untrust(id: string, review: Review): void {
    this.reviews.set(id, review)
    this.boards = this.boards.map((each) =>
      each.id === id
        ? {
            ...each,
            lines: each.lines.map((l) => ({ ...l, trust: { state: 'untrusted' as const, hash: review.trust.hash } })),
          }
        : each,
    )
  }
}

/** No board yet: a daemon that knows five folders, one for each way the checks come out. */
export function firstRunWorld(): FirstRunWorld {
  const world = new FirstRunWorld()
  world.boards = []
  const folders: [string, readonly Check[] | null][] = [
    ['/work/fresh', CHECKS.allPass],
    ['/work/noclaude', CHECKS.noClaude],
    ['/work/loggedout', CHECKS.loggedOut],
    ['/work/unknown', CHECKS.couldNotCheck],
  ]
  for (const [root, checks] of folders) {
    world.folders.set(root, board(`b-${root.split('/').pop()}`, root))
    if (checks !== null) world.checkSets.set(root, checks)
  }
  world.folders.set('/work/plain', { checks: [{ message: GIT_NO.message }] })
  world.checkSets.set('/work/plain', CHECKS.notARepo)
  return world
}

/** One board with no task, whose line has no Collide folder: the empty board, with the banner to share. */
export function emptyBoardWorld(): FirstRunWorld {
  const world = new FirstRunWorld()
  world.boards = [board('b-alpha', '/work/alpha')]
  world.shareFiles = ['.collide/board.yaml', '.collide/views/mine.yaml']
  return world
}

/** One board with a task whose line has a Collide folder: no banner. */
export function sharedBoardWorld(): FirstRunWorld {
  const world = emptyBoardWorld()
  world.shared = true
  world.listing = [task('scratch:1', 'A task that is here', 'ready', 'ready')]
  return world
}

/** A clone: its line is untrusted, and its Collide folder brings a pack, two views and settings. */
export function trustBoardWorld(): FirstRunWorld {
  const world = emptyBoardWorld()
  world.shared = true
  world.listing = [task('scratch:1', 'A task that is here', 'ready', 'ready')]
  const brings = { packs: ['repo:.collide/packs/tracker'], views: ['mine', 'team'], settings: true }
  world.untrust('b-alpha', reviewOf(brings))
  return world
}

/** A board whose `.collide` is in the folder already, so Share is refused. */
export function shareRefusedWorld(): FirstRunWorld {
  const world = emptyBoardWorld()
  world.refuseInit = '`.collide` already exists, so `init` leaves the repository as it is'
  return world
}
