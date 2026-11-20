import { readFileSync, realpathSync, renameSync, rmSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { afterEach, describe, expect, test } from 'vitest'
import type { TaskInput } from '@collide/pack-sdk'
import { compare, diffOf, emptySnapshot } from '../../../fixtures/packs/orchard/providers/plan/changes.ts'
import { DEBOUNCE_MS, RECHECK_MS } from '../../../fixtures/packs/orchard/providers/plan/watcher.ts'
import { fakeClock, fakeFs, noFs } from './fakes.ts'
import { DEADLINE_MS } from './helpers.ts'
import { makeRepo, plan, versionOf } from './repo.ts'
import type { Repo } from './repo.ts'

// `watch` against real git repositories and the fake plan CLI. A test that is about the debounce, the recheck or what
// is watched uses a fake clock and a fake file watcher, so it waits for nothing; the two tests that need a real
// watcher event wait for it with a deadline, never for a set time.

const ARCH = ['--root', 'arch']
const CATEGORY: Record<string, string> = {
  specified: 'backlog',
  ready: 'ready',
  doing: 'active',
  in_review: 'review',
  done: 'done',
  dropped: 'dropped',
}

type Client = Awaited<ReturnType<Repo['provider']>>
const open: Client[] = []
afterEach(async () => {
  await Promise.all(open.splice(0).map((client) => client.close()))
})

interface Watching {
  client: Client
  clock: ReturnType<typeof fakeClock>
  fs: ReturnType<typeof fakeFs>
  /** What the watcher would have seen in this checkout: an event in its `arch/`. */
  touch(repo: Repo): void
  /** The quiet spell ends: every marked checkout is read once. */
  settle(): void
}

/** A provider on a fake clock and a fake file watcher, told to watch `checkouts`. */
async function watching(main: Repo, checkouts: Repo[] = [main], fs = fakeFs()): Promise<Watching> {
  const clock = fakeClock()
  const client = await main.provider({ watch: { timers: clock.timers, watchFs: fs.watchFs } })
  open.push(client)
  const reply = await client.request('watch', { checkouts: checkouts.map((checkout) => checkout.root) })
  expect(reply.error).toBeUndefined()
  return {
    client,
    clock,
    fs,
    touch: (repo) => fs.at(join(repo.root, 'arch')).emit('somewhere.md'),
    settle: () => clock.runWaits(),
  }
}

const setStatus = (repo: Repo, shortId: string, status: string): void => {
  const path = repo.taskPath(shortId)
  repo.write(path, repo.read(path).replace(/^status: .*$/m, `status: ${status}`))
}
const planOk = (repo: Repo, args: string[]): void => {
  const run = plan(repo, [...args, ...ARCH])
  expect(run.code, run.stderr).toBe(0)
}
const state = (status: string) => ({ status, category: CATEGORY[status] })

/** The notifications that are in, after one more round trip, so that nothing is still on its way. */
async function inbox(watch: Watching) {
  await watch.client.request('get', { id: 'T-99-999' })
  return watch.client.pending()
}

describe('watch: what it does when asked', () => {
  test('answers with an empty result, sends nothing for what is already there, and sets the recheck', async () => {
    const watch = await watching(makeRepo())
    expect(await inbox(watch)).toEqual([])
    expect(watch.clock.waits()).toEqual([])
    expect(watch.clock.periods()).toEqual([RECHECK_MS])
    expect(RECHECK_MS).toBe(10_000)
  })

  test('watches the arch folder of each checkout, below it too, and the folder of its HEAD file', async () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    const { fs } = await watching(main, [main, other])
    const real = (path: string) => realpathSync(path)
    const opened = fs.open().map((entry) => [real(entry.target), entry.recursive])
    // A worktree keeps its HEAD file in a folder of the main repository's git folder.
    expect(opened).toHaveLength(4)
    expect(opened).toEqual(
      expect.arrayContaining([
        [real(join(main.root, 'arch')), true],
        [real(join(main.root, '.git')), false],
        [real(join(other.root, 'arch')), true],
        [real(join(main.root, '.git', 'worktrees', 'feature')), false],
      ]),
    )
  })

  test.each([
    ['a list of checkouts', { checkouts: 'nowhere' }],
    ['absolute paths', { checkouts: ['relative/path'] }],
  ])('needs %s', async (_name, params) => {
    const client = await makeRepo().provider()
    open.push(client)
    expect((await client.request('watch', params)).error?.message).toMatch(/absolute paths/)
  })
})

describe('watch: task.changed', () => {
  test('after plan set in a worktree: old and new state, the diff, cause worktree and that checkout', async () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    const watch = await watching(main, [main, other])
    planOk(other, ['set', 'T-03-003', 'status=doing'])
    watch.touch(other)
    expect(watch.clock.waits()).toEqual([DEBOUNCE_MS])
    watch.settle()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params).toEqual({
      key: 'plan:T-03-003',
      version: versionOf(other, other.taskPath('T-03-003')),
      from: state('ready'),
      to: state('doing'),
      diff: { status: { from: 'ready', to: 'doing' } },
      cause: 'worktree',
      checkout: other.root,
    })
    expect(await inbox(watch)).toEqual([])
  })

  test('waits for a quiet spell: each event moves the end of it, so ten events are one read', async () => {
    const main = makeRepo()
    const watch = await watching(main)
    for (let n = 0; n < 10; n++) watch.touch(main)
    expect(watch.clock.waits()).toEqual([DEBOUNCE_MS])
    expect(DEBOUNCE_MS).toBe(300)
  })

  test('ten writes in a burst give one event for each task that changed, with the last state', async () => {
    const main = makeRepo()
    const watch = await watching(main)
    const steps = ['ready', 'doing', 'ready', 'doing', 'dropped', 'ready', 'doing', 'ready', 'doing', 'in_review']
    steps.forEach((status, n) => {
      setStatus(main, 'T-03-003', status)
      if (n % 3 === 0) setStatus(main, 'T-05-002', n === 9 ? 'ready' : 'doing')
      watch.touch(main)
    })
    expect(watch.clock.waits()).toEqual([DEBOUNCE_MS])
    watch.settle()
    const events = await watch.client.notifications('task.changed', 2)
    const byKey = Object.fromEntries(events.map((event) => [event.params.key, event.params]))
    expect(Object.keys(byKey).sort()).toEqual(['plan:T-03-003', 'plan:T-05-002'])
    expect(byKey['plan:T-03-003']).toMatchObject({ from: state('ready'), to: state('in_review') })
    expect(byKey['plan:T-05-002']).toMatchObject({ from: state('specified'), to: state('ready') })
    expect(watch.clock.waits()).toEqual([])
    expect(await inbox(watch)).toEqual([])
  })

  test('a new file is a change from nothing, with its fields as the diff', async () => {
    const main = makeRepo()
    const watch = await watching(main)
    planOk(main, ['new', 'task', 'P03', 'Chart the tides', '--epic', 'E-07', '--depends-on', 'T-03-001'])
    watch.touch(main)
    watch.settle()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params).toMatchObject({
      key: 'plan:T-03-006',
      from: null,
      to: state('specified'),
      diff: {
        status: { from: null, to: 'specified' },
        title: { from: null, to: 'Chart the tides' },
        epic: { from: null, to: 'E-07' },
        depends_on: { added: ['T-03-001'], removed: [] },
      },
      cause: 'worktree',
      checkout: main.root,
    })
  })

  test('an edit of the body is a change with an empty diff, and a rewrite with the same bytes is none', async () => {
    const main = makeRepo()
    const watch = await watching(main)
    const path = main.taskPath('T-03-003')
    main.write(path, main.read(path))
    watch.touch(main)
    watch.settle()
    expect(await inbox(watch)).toEqual([])
    main.write(path, `${main.read(path)}\nA new line of the body.\n`)
    watch.touch(main)
    watch.settle()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params).toMatchObject({ key: 'plan:T-03-003', diff: {}, from: state('ready'), to: state('ready') })
  })

  test('a change of any link is named in the diff, by the type of the link, so that core reads the task again', () => {
    const task = (links: Array<{ type: string; to: string }>): TaskInput => ({
      key: 'plan:ADR-0007',
      id: 'ADR-0007',
      title: 'Keep the ledger',
      state: { status: 'ready', category: 'ready' },
      version: 'a',
      links,
    })
    const before = task([
      { type: 'evidence', to: 'SP-0001' },
      { type: 'supersedes', to: 'ADR-0003' },
    ])
    const after = task([
      { type: 'evidence', to: 'SP-0001' },
      { type: 'evidence', to: 'SP-0002' },
      { type: 'blocks', to: 'OB-0001' },
    ])
    expect(diffOf(before, after)).toEqual({
      evidence: { added: ['SP-0002'], removed: [] },
      supersedes: { added: [], removed: ['ADR-0003'] },
      blocks: { added: ['OB-0001'], removed: [] },
    })
    expect(diffOf(before, before)).toEqual({})
  })

  test('a file that is moved to another name keeps its short id: one change, the path in the diff', async () => {
    const main = makeRepo()
    const watch = await watching(main)
    const path = main.taskPath('T-03-003')
    main.git('mv', path, path.replace('price-the-berths', 'cost-the-berths'))
    watch.touch(main)
    watch.settle()
    const events = await watch.client.notifications(null)
    expect(events.map((event) => event.method)).toEqual(['task.changed'])
    expect(events[0]?.params.diff).toEqual({
      path: { from: path, to: path.replace('price-the-berths', 'cost-the-berths') },
    })
  })
})

describe('watch: cause', () => {
  /** A branch `other` where T-03-003 is `doing`, and the main checkout back on `main` with it `ready`. */
  const branched = (): Repo => {
    const main = makeRepo()
    main.git('checkout', '--quiet', '-b', 'other')
    setStatus(main, 'T-03-003', 'doing')
    main.git('commit', '--quiet', '-a', '-m', 'doing')
    main.git('checkout', '--quiet', 'main')
    return main
  }

  test('is head after a checkout of a branch with another status', async () => {
    const main = branched()
    const watch = await watching(main)
    main.git('checkout', '--quiet', 'other')
    watch.touch(main)
    watch.settle()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params).toMatchObject({
      key: 'plan:T-03-003',
      from: state('ready'),
      to: state('doing'),
      cause: 'head',
    })
  })

  test('is head after a merge that changes the file, and the next edit is worktree again', async () => {
    const main = branched()
    const watch = await watching(main)
    main.git('merge', '--quiet', '--ff-only', 'other')
    watch.touch(main)
    watch.settle()
    const [merged] = await watch.client.notifications('task.changed')
    expect(merged?.params).toMatchObject({ to: state('doing'), cause: 'head' })
    setStatus(main, 'T-03-003', 'in_review')
    watch.touch(main)
    watch.settle()
    const [edited] = await watch.client.notifications('task.changed')
    expect(edited?.params).toMatchObject({ from: state('doing'), to: state('in_review'), cause: 'worktree' })
  })

  test('an event in the folder of HEAD marks the checkout, and one for a lock file does not', async () => {
    const main = makeRepo()
    const watch = await watching(main)
    const head = watch.fs.open().find((entry) => !entry.recursive) as { emit(name: string | null): void }
    head.emit('HEAD.lock')
    head.emit('index')
    expect(watch.clock.waits()).toEqual([])
    head.emit('HEAD')
    expect(watch.clock.waits()).toEqual([DEBOUNCE_MS])
  })
})

describe('watch: task.removed', () => {
  test('a deleted file is removed with cause deleted, for that checkout', async () => {
    const main = makeRepo()
    const watch = await watching(main)
    rmSync(join(main.root, main.taskPath('T-03-005')))
    watch.touch(main)
    watch.settle()
    const [removed] = await watch.client.notifications('task.removed')
    expect(removed?.params).toEqual({ key: 'plan:T-03-005', cause: 'deleted', checkout: main.root })
  })

  test('is unknown when HEAD moved, since the adapter does not read history to say why', async () => {
    const main = makeRepo()
    main.git('checkout', '--quiet', '-b', 'without')
    main.git('rm', '--quiet', main.taskPath('T-03-005'))
    main.git('commit', '--quiet', '-m', 'drop the file')
    main.git('checkout', '--quiet', 'main')
    const watch = await watching(main)
    main.git('checkout', '--quiet', 'without')
    watch.touch(main)
    watch.settle()
    const [removed] = await watch.client.notifications('task.removed')
    expect(removed?.params).toEqual({ key: 'plan:T-03-005', cause: 'unknown', checkout: main.root })
  })
})

describe('watch: task.invalid', () => {
  test('a file that becomes unparseable is invalid, with path, checkout and reason, and not listed', async () => {
    const main = makeRepo()
    const watch = await watching(main)
    const path = main.taskPath('T-03-003')
    main.write(path, '---\ntitle: "Price the berths"\nstatus: [ready\n---\n')
    watch.touch(main)
    watch.settle()
    const events = await watch.client.notifications(null)
    expect(events.map((event) => event.method)).toEqual(['task.invalid'])
    expect(events[0]?.params).toEqual({ path, checkout: main.root, reason: expect.stringMatching(/\S/) })
    const listed = (await watch.client.request('list')).result as any
    expect(listed.tasks.map((task: any) => task.key)).not.toContain('plan:T-03-003')
    expect(listed.invalid.map((item: any) => item.path)).toEqual([path])
  })

  test('says it once, and fixing the file gives task.changed, from nothing', async () => {
    const main = makeRepo()
    const watch = await watching(main)
    const path = main.taskPath('T-03-003')
    const good = main.read(path)
    main.write(path, '<<<<<<< ours\n' + good + '=======\n' + good + '>>>>>>> theirs\n')
    watch.touch(main)
    watch.settle()
    await watch.client.notifications('task.invalid')
    // The task is out of the epics' groups too: the collections are said once, with the invalid file.
    await watch.client.notifications('collections.changed')
    watch.touch(main)
    watch.settle()
    expect(await inbox(watch)).toEqual([])
    main.write(path, good)
    watch.touch(main)
    watch.settle()
    const [fixed] = await watch.client.notifications('task.changed')
    expect(fixed?.params).toMatchObject({ key: 'plan:T-03-003', from: null, to: state('ready') })
    await watch.client.notifications('collections.changed')
    expect(await inbox(watch)).toEqual([])
  })

  test('two files with one short id in one checkout are two invalid files, and no collision', async () => {
    const main = makeRepo()
    const watch = await watching(main)
    const path = main.taskPath('T-03-003')
    main.write(path.replace('price-the-berths', 'cost-the-berths'), main.read(path))
    watch.touch(main)
    watch.settle()
    const invalid = await watch.client.notifications('task.invalid', 2)
    expect(invalid.map((event) => event.params.checkout)).toEqual([main.root, main.root])
    // The task is out of the epics' groups: the one other thing said.
    await watch.client.notifications('collections.changed')
    expect(await inbox(watch)).toEqual([])
  })
})

describe('watch: renames', () => {
  test('a renumber in a checkout is task.renamed with both keys and the checkout, and nothing else', async () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    const watch = await watching(main, [main, other])
    planOk(other, ['renumber', 'T-03-003', 'T-03-050'])
    watch.touch(other)
    watch.settle()
    const [renamed] = await watch.client.notifications('task.renamed')
    expect(renamed?.params).toEqual({ from: 'plan:T-03-003', to: 'plan:T-03-050', checkout: other.root })
    // No task changed: the renumber moved the file and rewrote a citation in an epic, so the epic's groups name the new
    // key, and that is the one other thing said.
    const [collections] = await watch.client.notifications('collections.changed')
    expect(collections?.params.checkout).toBe(other.root)
    expect(collections?.params.members).toEqual([])
    expect(await inbox(watch)).toEqual([])
    const listed = (await watch.client.request('list', { checkout: other.root })).result as any
    const keys = listed.tasks.map((task: any) => task.key)
    expect(keys).toContain('plan:T-03-050')
    expect(keys).not.toContain('plan:T-03-003')
  })

  test('a chain of renumbers read in one go is one rename, from the first id to the last', async () => {
    const main = makeRepo()
    const watch = await watching(main)
    planOk(main, ['renumber', 'T-03-003', 'T-03-050'])
    planOk(main, ['renumber', 'T-03-050', 'T-03-060'])
    watch.touch(main)
    watch.settle()
    const renamed = await watch.client.notifications('task.renamed')
    expect(renamed.map((event) => event.params)).toEqual([
      { from: 'plan:T-03-003', to: 'plan:T-03-060', checkout: main.root },
    ])
    expect((await inbox(watch)).filter((event) => event.method === 'task.removed')).toEqual([])
  })

  test('a file that git reports renamed between two commits is task.renamed, with no id map row', async () => {
    const main = makeRepo()
    const watch = await watching(main)
    const path = main.taskPath('T-03-003')
    main.git('mv', path, path.replace('T-03-003', 'T-03-050'))
    main.git('commit', '--quiet', '-m', 'renumber by hand')
    watch.touch(main)
    watch.settle()
    const [renamed] = await watch.client.notifications('task.renamed')
    expect(renamed?.params).toEqual({ from: 'plan:T-03-003', to: 'plan:T-03-050', checkout: main.root })
    expect((await inbox(watch)).map((event) => event.method)).not.toContain('task.removed')
  })

  test('a file that went and another that came, with nothing to link them, are a removal and a new task', async () => {
    const main = makeRepo()
    const watch = await watching(main)
    const path = main.taskPath('T-03-003')
    main.git('mv', path, path.replace('T-03-003', 'T-03-050'))
    watch.touch(main)
    watch.settle()
    const events = await watch.client.notifications(null, 2)
    expect(events.map((event) => `${event.method} ${event.params.key}`).sort()).toEqual([
      'task.changed plan:T-03-050',
      'task.removed plan:T-03-003',
    ])
    expect(events.find((event) => event.method === 'task.changed')?.params.from).toBeNull()
  })

  test('with no rename from git for the gone file, a new task without a path is not taken for its new name', () => {
    const state = { status: 'ready', category: 'ready' }
    const gone = { key: 'plan:T-03-003', title: 'Gone', state, version: 'a', path: 'arch/x/T-03-003-gone.md' }
    const fresh = { key: 'plan:T-03-050', title: 'Fresh', state, version: 'b' }
    const before = { ...emptySnapshot(), head: 'one', tasks: new Map([['T-03-003', gone as TaskInput]]) }
    const after = { ...emptySnapshot(), head: 'two', tasks: new Map([['T-03-050', fresh as TaskInput]]) }
    const methods = compare('/checkout', before, after, []).map((event) => event.method)
    expect(methods.sort()).toEqual(['task.changed', 'task.removed'])
  })
})

describe('watch: collisions', () => {
  /** Each checkout makes a task of its own: `plan new` gives both the next number of the proposal, T-03-006. */
  const newTasks = async (watch: Watching, main: Repo, other: Repo) => {
    planOk(main, ['new', 'task', 'P03', 'Chart the tides', '--epic', 'E-07'])
    planOk(other, ['new', 'task', 'P03', 'Sound the channel', '--epic', 'E-07'])
    watch.touch(main)
    watch.touch(other)
    watch.settle()
  }

  test('one short id at two paths in two checkouts is a collision with both locations', async () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    const watch = await watching(main, [main, other])
    await newTasks(watch, main, other)
    const [collision] = await watch.client.notifications('task.collision')
    expect(collision?.params).toEqual({
      shortId: 'T-03-006',
      locations: [
        { checkout: other.root, path: 'arch/proposals/03-harbor-index/tasks/T-03-006-sound-the-channel.md' },
        { checkout: main.root, path: 'arch/proposals/03-harbor-index/tasks/T-03-006-chart-the-tides.md' },
      ],
    })
    // Two new tasks, one in each checkout: changes, and no other collision.
    expect((await watch.client.notifications('task.changed', 2)).map((event) => event.params.checkout).sort()).toEqual(
      [main.root, other.root].sort(),
    )
    expect(await inbox(watch)).toEqual([])
  })

  test('is sent when the locations change, not again while they stay as they were', async () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    const watch = await watching(main, [main, other])
    await newTasks(watch, main, other)
    await watch.client.notifications('task.collision')
    setStatus(main, 'T-03-001', 'dropped')
    watch.touch(main)
    watch.settle()
    await watch.client.notifications('task.changed', 3)
    expect(await inbox(watch)).toEqual([])
  })

  test('a resolved collision needs no event: core lists again', async () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    const watch = await watching(main, [main, other])
    await newTasks(watch, main, other)
    await watch.client.notifications('task.collision')
    await watch.client.notifications('task.changed', 2)
    planOk(other, ['renumber', 'T-03-006', 'T-03-007'])
    watch.touch(other)
    watch.settle()
    await watch.client.notifications('task.renamed')
    expect(await inbox(watch)).toEqual([])
  })

  test('the same task at the same path in two checkouts is not one, whatever state each has', async () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    const watch = await watching(main, [main, other])
    setStatus(main, 'T-03-003', 'doing')
    setStatus(other, 'T-03-003', 'dropped')
    watch.touch(main)
    watch.touch(other)
    watch.settle()
    const changes = await watch.client.notifications('task.changed', 2)
    expect(changes).toHaveLength(2)
    expect((await inbox(watch)).map((event) => event.method)).not.toContain('task.collision')
  })

  test('a collision that is there when watch starts is not announced: core finds it from the lists', async () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    planOk(main, ['new', 'task', 'P03', 'Chart the tides', '--epic', 'E-07'])
    planOk(other, ['new', 'task', 'P03', 'Sound the channel', '--epic', 'E-07'])
    const watch = await watching(main, [main, other])
    expect(await inbox(watch)).toEqual([])
  })
})

describe('watch: the recheck', () => {
  test('finds a change that no watcher saw, which is how the plan keeps correct without a watcher', async () => {
    const main = makeRepo()
    const clock = fakeClock()
    const client = await main.provider({ watch: { timers: clock.timers, watchFs: noFs } })
    open.push(client)
    await client.request('watch', { checkouts: [main.root] })
    setStatus(main, 'T-03-003', 'doing')
    expect(clock.waits()).toEqual([])
    clock.runIntervals()
    const [changed] = await client.notifications('task.changed')
    expect(changed?.params).toMatchObject({ key: 'plan:T-03-003', to: state('doing'), cause: 'worktree' })
  })

  test('runs on a real clock at the interval it is given, with no watcher at all', async () => {
    const main = makeRepo()
    const client = await main.provider({ watch: { recheckMs: 40, watchFs: noFs } })
    open.push(client)
    await client.request('watch', { checkouts: [main.root] })
    setStatus(main, 'T-03-003', 'doing')
    const [changed] = await client.notifications('task.changed', 1, DEADLINE_MS)
    expect(changed?.params).toMatchObject({ key: 'plan:T-03-003', to: state('doing') })
  })

  test('opens again a watcher that failed, when it reads the checkout', async () => {
    const main = makeRepo()
    const watch = await watching(main)
    const arch = watch.fs.at(join(main.root, 'arch'))
    arch.fail(new Error('the watcher broke'))
    arch.fail(new Error('and said so twice'))
    setStatus(main, 'T-03-003', 'doing')
    watch.clock.runIntervals()
    await watch.client.notifications('task.changed')
    expect(watch.fs.opened.filter((entry) => entry.target === join(main.root, 'arch'))).toHaveLength(2)
    expect(watch.fs.open().filter((entry) => entry.target === join(main.root, 'arch'))).toHaveLength(1)
  })
})

describe('watch: source.stale', () => {
  test('a checkout that disappears is stale, naming it, once; the others keep working', async () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    const watch = await watching(main, [main, other])
    rmSync(other.root, { recursive: true, force: true })
    watch.clock.runIntervals()
    const [stale] = await watch.client.notifications('source.stale')
    expect(stale?.params.reason).toContain(other.root)
    expect(stale?.params.checkout).toBe(other.root)
    watch.clock.runIntervals()
    setStatus(main, 'T-03-003', 'doing')
    watch.touch(main)
    watch.settle()
    await watch.client.notifications('task.changed')
    expect(await inbox(watch)).toEqual([])
    expect(watch.fs.open().filter((entry) => entry.target.includes('feature'))).toEqual([])
  })

  test('a checkout that was not there when watch came is stale after the reply, and the others work', async () => {
    const main = makeRepo()
    const gone = join(main.root, '..', 'no-such-checkout')
    const clock = fakeClock()
    const fs = fakeFs()
    const client = await main.provider({ watch: { timers: clock.timers, watchFs: fs.watchFs } })
    open.push(client)
    const reply = await client.request('watch', { checkouts: [main.root, gone] })
    expect(reply.result).toEqual({})
    const [stale] = await client.notifications('source.stale')
    expect(stale?.params.reason).toContain('no-such-checkout')
    expect(stale?.params.checkout).toBe(gone)
    setStatus(main, 'T-03-003', 'doing')
    fs.at(join(main.root, 'arch')).emit('x')
    clock.runWaits()
    await client.notifications('task.changed')
  })

  test('comes back as changes when the checkout can be read again', async () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    const watch = await watching(main, [main, other])
    const moved = `${other.root}-away`
    const path = other.taskPath('T-03-003')
    renameSync(other.root, moved)
    watch.clock.runIntervals()
    await watch.client.notifications('source.stale')
    writeFileSync(join(moved, path), readFileSync(join(moved, path), 'utf8').replace(/^status: .*$/m, 'status: doing'))
    renameSync(moved, other.root)
    watch.clock.runIntervals()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params).toMatchObject({ key: 'plan:T-03-003', to: state('doing'), checkout: other.root })
    expect(watch.fs.open().filter((entry) => entry.target.includes('feature'))).toHaveLength(2)
  })
})

describe('watch: the set of checkouts', () => {
  test('a later watch replaces it: the checkouts left out are no longer read or watched', async () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    const watch = await watching(main, [main, other])
    expect(watch.fs.open()).toHaveLength(4)
    await watch.client.request('watch', { checkouts: [main.root] })
    expect(watch.fs.open().filter((entry) => entry.target.includes('feature'))).toEqual([])
    expect(watch.fs.open()).toHaveLength(2)
    const closed = watch.fs.opened.filter((entry) => entry.closed)
    expect(closed.every((entry) => entry.target.includes('feature'))).toBe(true)
    setStatus(other, 'T-03-003', 'doing')
    watch.clock.runIntervals()
    setStatus(main, 'T-03-002', 'in_review')
    watch.touch(main)
    watch.settle()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params.checkout).toBe(main.root)
    expect(await inbox(watch)).toEqual([])
  })

  test('a checkout kept by a later watch keeps its watchers, and one added is read without events', async () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    const watch = await watching(main, [main])
    const kept = watch.fs.open()
    await watch.client.request('watch', { checkouts: [main.root, other.root] })
    expect(watch.fs.open().slice(0, 2)).toEqual(kept)
    expect(watch.fs.open()).toHaveLength(4)
    expect(await inbox(watch)).toEqual([])
  })

  test('an empty watch drops them all and stops the recheck', async () => {
    const watch = await watching(makeRepo())
    await watch.client.request('watch', { checkouts: [] })
    expect(watch.fs.open()).toEqual([])
    expect(watch.clock.periods()).toEqual([])
  })

  test('every watcher and timer is closed when the provider ends', async () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    const watch = await watching(main, [main, other])
    watch.touch(main)
    expect(watch.fs.open()).toHaveLength(4)
    await watch.client.close()
    open.splice(open.indexOf(watch.client), 1)
    expect(watch.fs.open()).toEqual([])
    expect(watch.clock.periods()).toEqual([])
    expect(watch.clock.waits()).toEqual([])
  })
})

describe('watch: a real file watcher', () => {
  test('plan set in a worktree reaches the provider as task.changed for that worktree', async () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    const client = await main.provider()
    open.push(client)
    expect((await client.request('watch', { checkouts: [main.root, other.root] })).result).toEqual({})
    planOk(other, ['set', 'T-03-003', 'status=doing'])
    const [changed] = await client.notifications('task.changed', 1, DEADLINE_MS)
    expect(changed?.params).toMatchObject({
      key: 'plan:T-03-003',
      from: state('ready'),
      to: state('doing'),
      cause: 'worktree',
      checkout: other.root,
    })
  })

  test('git checkout of a branch with another status gives cause head', async () => {
    const main = makeRepo()
    main.git('checkout', '--quiet', '-b', 'other')
    setStatus(main, 'T-03-003', 'doing')
    main.git('commit', '--quiet', '-a', '-m', 'doing')
    main.git('checkout', '--quiet', 'main')
    const client = await main.provider()
    open.push(client)
    await client.request('watch', { checkouts: [main.root] })
    main.git('checkout', '--quiet', 'other')
    const [changed] = await client.notifications('task.changed', 1, DEADLINE_MS)
    expect(changed?.params).toMatchObject({ key: 'plan:T-03-003', to: state('doing'), cause: 'head' })
  })

  test('a tree that comes back after watch started is found, with a short recheck and a real watcher', async () => {
    const main = makeRepo()
    rmSync(join(main.root, 'arch', 'proposals', '05-lamp-works'), { recursive: true })
    const client = await main.provider({ watch: { recheckMs: 40 } })
    open.push(client)
    await client.request('watch', { checkouts: [main.root] })
    main.git('checkout', '--quiet', '--', 'arch')
    const events = await client.notifications('task.changed', 3, DEADLINE_MS)
    expect(events.map((event) => event.params.key).sort()).toEqual(['plan:T-05-001', 'plan:T-05-002', 'plan:T-05-003'])
  })
})
