import { mkdirSync, readFileSync, realpathSync, renameSync, rmSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { afterAll, afterEach, beforeAll, describe, expect, test } from 'vitest'
import { DEBOUNCE_MS, RECHECK_MS } from '../src/watch.ts'
import { fakeClock, fakeFs } from './fakes.ts'
import { commitAll, EPIC_FILE, fileVersion, git, makeRepo, spawnProvider, tempDir, connect } from './helpers.ts'

// `watch` over real git repositories. A test about the debounce, the recheck or what is watched gives the provider a
// fake clock and a fake file watcher, so it waits for nothing; the two that need a real watcher event wait for it with
// a deadline, never for a set time. Git reads no user or system configuration.

const PLANNING = '.collide/planning'
const ALPHA = `${PLANNING}/epics/ep-001-aa-alpha`
const BETA = `${PLANNING}/epics/ep-002-bb-beta`
const ONE = `${ALPHA}/tasks/ts-0001-aa-one.md`
const TWO = `${ALPHA}/tasks/ts-0002-bb-two.md`
const SAFE_GIT = { GIT_CONFIG_GLOBAL: '/dev/null', GIT_CONFIG_NOSYSTEM: '1' }
const ENV = { ...process.env, ...SAFE_GIT }

const saved = { global: process.env.GIT_CONFIG_GLOBAL, system: process.env.GIT_CONFIG_NOSYSTEM }
beforeAll(() => Object.assign(process.env, SAFE_GIT))
afterAll(() => {
  for (const [name, value] of [['GIT_CONFIG_GLOBAL', saved.global], ['GIT_CONFIG_NOSYSTEM', saved.system]] as const) {
    if (value === undefined) delete process.env[name]
    else process.env[name] = value
  }
})

const item = (kind: string, id: string, status: string, title: string, body: string): string =>
  `---\nid: ${id}\nkind: ${kind}\nstatus: ${status}\ntitle: "${title}"\nowners: []\nlinks: []\n---\n\n${body}\n`
const task = (id: string, status = 'ready', title = `Item ${id}`): string =>
  item('task', id, status, title, '## Goal\n\nMade-up content.')
const question = (id: string, status = 'open'): string =>
  item('question', id, status, `Question ${id}`, '## Question\n\nWhy?')
const spike = (id: string, status = 'planned'): string =>
  item('spike', id, status, `Spike ${id}`, '## Question\n\nWhy?')
const PROPOSAL = item('proposal', 'pr-0001-aa', 'draft', 'Made-up proposal', '') +
  '## Context\n\nMade-up.\n\n## Features\n\n### Packs\n- **F1 Trust prompt**: asks before the first run.\n'

/** A committed repository with an epic, two tasks and a second epic. */
function repo(extra: Record<string, string> = {}): string {
  const main = makeRepo({
    'ep-001-aa-alpha/epic.md': EPIC_FILE,
    'ep-002-bb-beta/epic.md': EPIC_FILE.replace('ep-001-aa', 'ep-002-bb'),
    'ep-001-aa-alpha/tasks/ts-0001-aa-one.md': task('ts-0001-aa'),
    'ep-001-aa-alpha/tasks/ts-0002-bb-two.md': task('ts-0002-bb'),
    ...extra,
  })
  git(main, 'init', '--quiet')
  commitAll(main, 'fixture')
  return main
}

const write = (root: string, path: string, text: string): void => {
  mkdirSync(dirname(join(root, path)), { recursive: true })
  writeFileSync(join(root, path), text)
}
const read = (root: string, path: string): string => readFileSync(join(root, path), 'utf8')
const setStatus = (root: string, path: string, status: string): void =>
  write(root, path, read(root, path).replace(/^status: .*$/m, `status: ${status}`))
const state = (status: string) => ({ status, category: status })

type Client = ReturnType<typeof connect>
const open: Client[] = []
afterEach(async () => {
  await Promise.all(open.splice(0).map((client) => client.close()))
})

interface Watching {
  client: Client
  clock: ReturnType<typeof fakeClock>
  fs: ReturnType<typeof fakeFs>
  /** What the watcher would have seen: an event in the planning folder of `checkout`. */
  touch(checkout: string, name?: string | null): void
  /** The quiet spell ends: every marked checkout is read once. */
  settle(): void
}

interface WatchingOptions {
  checkouts?: string[]
  packData?: string | null
  layout?: unknown
}

/** A provider on a fake clock and a fake file watcher, told to watch `checkouts`. */
async function watching(main: string, options: WatchingOptions = {}): Promise<Watching> {
  const { checkouts = [main], packData = null, layout } = options
  const clock = fakeClock()
  const fs = fakeFs()
  const client = connect({ packData, watch: { timers: clock.timers, watchFs: fs.watchFs, env: ENV } })
  open.push(client)
  const config = layout === undefined ? {} : { layout }
  expect((await client.request('initialize', { config, root: main })).error).toBeUndefined()
  const reply = await client.request('watch', { checkouts })
  expect(reply.error).toBeUndefined()
  return {
    client,
    clock,
    fs,
    touch: (checkout, name = 'epics/somewhere.md') => fs.at(join(checkout, PLANNING)).emit(name),
    settle: () => clock.runWaits(),
  }
}

/** The notifications that are in, after one more round trip. */
async function inbox(watch: Watching) {
  await watch.client.request('get', { id: 'ts-9999-zz' })
  return watch.client.pending()
}

describe('watch: what it does when asked', () => {
  test('answers with an empty result, sends nothing for what is already there, and sets the recheck', async () => {
    const watch = await watching(repo())
    expect(await inbox(watch)).toEqual([])
    expect(watch.clock.waits()).toEqual([])
    expect(watch.clock.periods()).toEqual([RECHECK_MS])
    expect(RECHECK_MS).toBe(10_000)
  })

  test('says in initialize that it watches', async () => {
    const client = connect()
    open.push(client)
    const { result } = await client.request('initialize', { config: {}, root: repo() })
    expect(result?.capabilities.watch).toBe(true)
  })

  test('watches the planning folder of each checkout, below it too, and the folder of its HEAD file', async () => {
    const main = repo()
    const linked = join(tempDir(), 'linked')
    git(main, 'worktree', 'add', '--quiet', '-b', 'side', linked)
    const { fs } = await watching(main, { checkouts: [main, linked] })
    const opened = fs.open().map((entry) => [entry.target, entry.recursive])
    expect(opened).toHaveLength(4)
    expect(opened).toEqual(
      expect.arrayContaining([
        [join(main, PLANNING), true],
        [realpathSync(join(main, '.git')), false],
        [join(linked, PLANNING), true],
        // A worktree keeps its HEAD file in a folder of the main repository's git folder.
        [realpathSync(join(main, '.git', 'worktrees', 'linked')), false],
      ]),
    )
  })

  test('watches the private root of the main checkout, and not of a worktree', async () => {
    const main = repo()
    const linked = join(tempDir(), 'linked')
    git(main, 'worktree', 'add', '--quiet', '-b', 'side', linked)
    const packData = tempDir()
    write(packData, 'planning/epics/ep-009-zz-priv/epic.md', EPIC_FILE.replace('ep-001-aa', 'ep-009-zz'))
    const { fs } = await watching(main, { checkouts: [main, linked], packData })
    const targets = fs.open().map((entry) => entry.target)
    expect(targets.filter((target) => target === join(packData, 'planning'))).toHaveLength(1)
    expect(fs.at(join(packData, 'planning')).recursive).toBe(true)
  })

  test.each([
    ['a list of checkouts', { checkouts: 'nowhere' }],
    ['absolute paths', { checkouts: ['relative/path'] }],
  ])('needs %s', async (_name, params) => {
    const client = connect()
    open.push(client)
    await client.request('initialize', { config: {}, root: repo() })
    expect((await client.request('watch', params)).error?.message).toMatch(/absolute paths/)
  })

  test('waits for a quiet spell: each event moves the end of it, so ten events are one read', async () => {
    const main = repo()
    const watch = await watching(main)
    for (let n = 0; n < 10; n++) watch.touch(main)
    expect(watch.clock.waits()).toEqual([DEBOUNCE_MS])
    expect(DEBOUNCE_MS).toBe(300)
  })

  test('an event of the HEAD file marks the checkout, and one of another file in the git folder does not', async () => {
    const main = repo()
    const watch = await watching(main)
    const gitFolder = watch.fs.at(realpathSync(join(main, '.git')))
    gitFolder.emit('index')
    gitFolder.emit('ORIG_HEAD')
    expect(watch.clock.waits()).toEqual([])
    gitFolder.emit('HEAD')
    expect(watch.clock.waits()).toEqual([DEBOUNCE_MS])
  })
})

describe('watch: task.changed', () => {
  test('after a status write: the old and new state, the field that changed, the cause and the checkout', async () => {
    const main = repo()
    const watch = await watching(main)
    setStatus(main, ONE, 'active')
    watch.touch(main)
    watch.settle()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params).toEqual({
      key: 'plan:ts-0001-aa',
      version: fileVersion(join(main, ONE)),
      from: state('ready'),
      to: state('active'),
      diff: { status: { from: 'ready', to: 'active' } },
      cause: 'worktree',
      checkout: main,
    })
    expect(await inbox(watch)).toEqual([])
  })

  test('a read that finds nothing new sends nothing', async () => {
    const main = repo()
    const watch = await watching(main)
    watch.touch(main)
    watch.settle()
    setStatus(main, TWO, 'active')
    watch.touch(main)
    watch.settle()
    const changed = await watch.client.notifications('task.changed')
    expect(changed.map((one) => one.params.key)).toEqual(['plan:ts-0002-bb'])
    expect(await inbox(watch)).toEqual([])
  })

  test('a task moved to another epic names its links and its path, and is not removed', async () => {
    const main = repo()
    const watch = await watching(main)
    mkdirSync(join(main, BETA, 'tasks'), { recursive: true })
    renameSync(join(main, ONE), join(main, BETA, 'tasks/ts-0001-aa-one.md'))
    watch.touch(main, 'epics/ep-002-bb-beta/tasks/ts-0001-aa-one.md')
    watch.settle()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params.key).toBe('plan:ts-0001-aa')
    expect(changed?.params.diff).toEqual({
      links: { from: [{ type: 'part-of', to: 'ep-001-aa' }], to: [{ type: 'part-of', to: 'ep-002-bb' }] },
      path: { from: ONE, to: `${BETA}/tasks/ts-0001-aa-one.md` },
    })
    expect(await inbox(watch)).toEqual([])
  })

  test('a title that changed is named, with the status that did not change left out', async () => {
    const main = repo()
    const watch = await watching(main)
    write(main, ONE, read(main, ONE).replace('"Item ts-0001-aa"', '"A better title"'))
    watch.touch(main)
    watch.settle()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params.diff).toEqual({ title: { from: 'Item ts-0001-aa', to: 'A better title' } })
  })

  test('a feature added to a proposal names the features, so that core reads the proposal again', async () => {
    const main = repo({ 'ep-001-aa-alpha/proposals/pr-0001-aa-p.md': PROPOSAL })
    const watch = await watching(main)
    const file = `${ALPHA}/proposals/pr-0001-aa-p.md`
    write(main, file, `${read(main, file)}- **F2 Pack lock**: pins each pack.\n`)
    watch.touch(main)
    watch.settle()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params.key).toBe('plan:pr-0001-aa')
    expect(Object.keys(changed?.params.diff)).toContain('features')
  })

  test('a new task is a change from nothing', async () => {
    const main = repo()
    const watch = await watching(main)
    write(main, `${ALPHA}/tasks/ts-0003-cc-three.md`, task('ts-0003-cc'))
    watch.touch(main)
    watch.settle()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params).toMatchObject({ key: 'plan:ts-0003-cc', from: null, to: state('ready'), checkout: main })
  })

  test('a question is watched with the rest of its epic', async () => {
    const main = repo({ 'ep-001-aa-alpha/questions/qu-0001-aa-why.md': question('qu-0001-aa') })
    const watch = await watching(main)
    const file = `${ALPHA}/questions/qu-0001-aa-why.md`
    setStatus(main, file, 'dropped')
    watch.touch(main, 'epics/ep-001-aa-alpha/questions/qu-0001-aa-why.md')
    expect(watch.clock.waits()).toEqual([DEBOUNCE_MS])
    watch.settle()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params).toMatchObject({ key: 'plan:qu-0001-aa', to: state('dropped') })
  })

  test('a change in the private root is a change of the main checkout', async () => {
    const main = repo()
    const packData = tempDir()
    const file = 'planning/epics/ep-009-zz-priv/tasks/ts-0900-zz-secret.md'
    write(packData, 'planning/epics/ep-009-zz-priv/epic.md', EPIC_FILE.replace('ep-001-aa', 'ep-009-zz'))
    write(packData, file, task('ts-0900-zz'))
    const watch = await watching(main, { packData })
    setStatus(packData, file, 'active')
    watch.fs.at(join(packData, 'planning')).emit('epics/ep-009-zz-priv/tasks/ts-0900-zz-secret.md')
    watch.settle()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params).toMatchObject({ key: 'plan:ts-0900-zz', to: state('active'), checkout: main })
  })

  test('the recheck reads each checkout without an event', async () => {
    const main = repo()
    const watch = await watching(main)
    setStatus(main, ONE, 'review')
    watch.clock.runIntervals()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params).toMatchObject({ key: 'plan:ts-0001-aa', to: state('review') })
  })

  test('a change in a worktree is that worktree\'s, with its own checkout', async () => {
    const main = repo()
    const linked = join(tempDir(), 'linked')
    git(main, 'worktree', 'add', '--quiet', '-b', 'side', linked)
    const watch = await watching(main, { checkouts: [main, linked] })
    setStatus(linked, ONE, 'active')
    watch.touch(linked)
    watch.settle()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params).toMatchObject({ key: 'plan:ts-0001-aa', checkout: linked })
    expect(await inbox(watch)).toEqual([])
  })
})

describe('watch: spike folders', () => {
  const SPIKE_FILE = `${PLANNING}/spikes/sp-0001-aa-trial/record.md`
  const withSpike = () => repo({ '../spikes/sp-0001-aa-trial/record.md': spike('sp-0001-aa') })

  test('only a spike\'s own record marks the checkout: what is inside the folder does not', async () => {
    const main = withSpike()
    const watch = await watching(main)
    for (const name of [
      'spikes/sp-0001-aa-trial/out/result.txt',
      'spikes/sp-0001-aa-trial/work/record.md',
      'spikes/sp-0001-aa-trial/node_modules/a/index.js',
      'spikes/sp-0001-aa-trial/notes.md',
    ]) {
      watch.touch(main, name)
    }
    expect(watch.clock.waits()).toEqual([])
    watch.touch(main, 'spikes/sp-0001-aa-trial/record.md')
    expect(watch.clock.waits()).toEqual([DEBOUNCE_MS])
  })

  test('a spike folder that comes or goes marks the checkout, and so does the epics folder beside it', async () => {
    const main = withSpike()
    const watch = await watching(main)
    watch.touch(main, 'spikes/sp-0002-bb-new')
    expect(watch.clock.waits()).toEqual([DEBOUNCE_MS])
  })

  test('a change of a spike\'s record is a change of the spike', async () => {
    const main = withSpike()
    const watch = await watching(main)
    setStatus(main, SPIKE_FILE, 'running')
    watch.touch(main, 'spikes/sp-0001-aa-trial/record.md')
    watch.settle()
    const [changed] = await watch.client.notifications('task.changed')
    expect(changed?.params).toMatchObject({ key: 'plan:sp-0001-aa', to: { status: 'running' } })
  })

  test('a folder named by paths.spike is watched, and its spike folders follow the same rule', async () => {
    const main = repo({ '../../../work/spikes/sp-0001-aa-trial/record.md': spike('sp-0001-aa') })
    const watch = await watching(main, { layout: { paths: { spike: 'work/spikes' } } })
    const folder = watch.fs.at(join(main, 'work/spikes'))
    expect(folder.recursive).toBe(true)
    folder.emit('sp-0001-aa-trial/out/result.txt')
    expect(watch.clock.waits()).toEqual([])
    folder.emit('sp-0001-aa-trial/record.md')
    expect(watch.clock.waits()).toEqual([DEBOUNCE_MS])
  })
})

describe('watch: task.removed, task.renamed, task.invalid and source.stale', () => {
  test('a deleted file is task.removed with the cause deleted', async () => {
    const main = repo()
    const watch = await watching(main)
    rmSync(join(main, TWO))
    watch.touch(main)
    watch.settle()
    const [removed] = await watch.client.notifications('task.removed')
    expect(removed?.params).toEqual({ key: 'plan:ts-0002-bb', cause: 'deleted', checkout: main })
    expect(await inbox(watch)).toEqual([])
  })

  test('a file that stops parsing is task.invalid, and the task is not also removed', async () => {
    const main = repo()
    const watch = await watching(main)
    write(main, TWO, 'no frontmatter at all\n')
    watch.touch(main)
    watch.settle()
    const [invalid] = await watch.client.notifications('task.invalid')
    expect(invalid?.params).toMatchObject({ path: TWO, checkout: main })
    expect(await inbox(watch)).toEqual([])
  })

  test('an id that went while another came is a rename when git renamed the file between the two commits', async () => {
    const main = repo()
    const watch = await watching(main)
    const to = `${ALPHA}/tasks/ts-0007-gg-seven.md`
    git(main, 'mv', TWO, to)
    write(main, to, task('ts-0007-gg', 'ready', 'Item ts-0002-bb'))
    commitAll(main, 'reid')
    watch.fs.at(realpathSync(join(main, '.git'))).emit('HEAD')
    watch.settle()
    const [renamed] = await watch.client.notifications('task.renamed')
    expect(renamed?.params).toEqual({ from: 'plan:ts-0002-bb', to: 'plan:ts-0007-gg', checkout: main })
    // Nothing but the path differs, and a rename says that.
    expect(await inbox(watch)).toEqual([])
  })

  test('with no rename git can name, an id that went and a new one are a removal and a new task', async () => {
    const main = repo()
    const watch = await watching(main)
    rmSync(join(main, TWO))
    write(main, `${ALPHA}/tasks/ts-0007-gg-seven.md`, task('ts-0007-gg'))
    watch.touch(main)
    watch.settle()
    await watch.client.notifications('task.removed')
    const [added] = await watch.client.notifications('task.changed')
    expect(added?.params).toMatchObject({ key: 'plan:ts-0007-gg', from: null })
    expect(await inbox(watch)).toEqual([])
  })

  test('a checkout that cannot be read is source.stale, once, and the others go on', async () => {
    const main = repo()
    const linked = join(tempDir(), 'linked')
    git(main, 'worktree', 'add', '--quiet', '-b', 'side', linked)
    const watch = await watching(main, { checkouts: [main, linked] })
    rmSync(linked, { recursive: true })
    watch.clock.runIntervals()
    const [stale] = await watch.client.notifications('source.stale')
    expect(stale?.params).toMatchObject({ checkout: linked })
    expect(stale?.params.reason).toContain('could not be read')

    // The next recheck finds it still gone and says nothing more; a change in the main checkout is still heard.
    watch.clock.runIntervals()
    setStatus(main, ONE, 'active')
    watch.clock.runIntervals()
    await watch.client.notifications('task.changed')
    expect(await inbox(watch)).toEqual([])
  })

  test('a checkout that cannot be read when watch is asked is source.stale after the reply', async () => {
    const main = repo()
    const gone = join(tempDir(), 'gone')
    const watch = await watching(main, { checkouts: [main, gone] })
    const [stale] = await watch.client.notifications('source.stale')
    expect(stale?.params).toMatchObject({ checkout: gone })
  })
})

describe('watch: the end', () => {
  test('closing the input closes every watcher and clears the clocks', async () => {
    const main = repo()
    const watch = await watching(main)
    expect(watch.fs.open().length).toBeGreaterThan(0)
    await watch.client.close()
    open.splice(open.indexOf(watch.client), 1)
    expect(watch.fs.open()).toEqual([])
    expect(watch.clock.periods()).toEqual([])
    expect(watch.clock.waits()).toEqual([])
  })

  test('a change after the end sends nothing', async () => {
    const main = repo()
    const watch = await watching(main)
    watch.touch(main)
    await watch.client.close()
    open.splice(open.indexOf(watch.client), 1)
    setStatus(main, ONE, 'active')
    watch.settle()
    expect(watch.client.pending()).toEqual([])
  })
})

describe('watch: a change between list and watch', () => {
  /** A provider on fakes that has listed `checkout`, as the daemon does before it asks to watch. */
  async function listed(main: string, checkout = main) {
    const clock = fakeClock()
    const fs = fakeFs()
    const client = connect({ watch: { timers: clock.timers, watchFs: fs.watchFs, env: ENV } })
    open.push(client)
    expect((await client.request('initialize', { config: {}, root: main })).error).toBeUndefined()
    expect((await client.request('list', { checkout })).error).toBeUndefined()
    return { client, clock, fs }
  }

  test('a status written after the list is sent at once, with no event and no recheck', async () => {
    const main = repo()
    const { client, clock } = await listed(main)
    setStatus(main, ONE, 'active')
    expect((await client.request('watch', { checkouts: [main] })).error).toBeUndefined()
    const [changed] = await client.notifications('task.changed')
    expect(changed?.params).toMatchObject({
      key: 'plan:ts-0001-aa',
      from: state('ready'),
      to: state('active'),
      diff: { status: { from: 'ready', to: 'active' } },
      checkout: main,
    })
    expect(clock.waits(), 'no wait for the quiet spell').toEqual([])
  })

  test('a file deleted after the list is task.removed, and one added is a new task', async () => {
    const main = repo()
    const { client } = await listed(main)
    rmSync(join(main, TWO))
    write(main, `${ALPHA}/tasks/ts-0003-cc-three.md`, task('ts-0003-cc'))
    await client.request('watch', { checkouts: [main] })
    const [removed] = await client.notifications('task.removed')
    const [added] = await client.notifications('task.changed')
    expect(removed?.params).toMatchObject({ key: 'plan:ts-0002-bb', checkout: main })
    expect(added?.params).toMatchObject({ key: 'plan:ts-0003-cc', from: null, checkout: main })
  })

  test('a commit made after the list is a change with the cause head', async () => {
    const main = repo()
    const { client } = await listed(main)
    setStatus(main, ONE, 'review')
    commitAll(main, 'after the list')
    await client.request('watch', { checkouts: [main] })
    const [changed] = await client.notifications('task.changed')
    expect(changed?.params).toMatchObject({ key: 'plan:ts-0001-aa', to: state('review'), cause: 'head' })
  })

  test('nothing is sent when nothing changed after the list, and the next change is the only one', async () => {
    const main = repo()
    const { client, clock, fs } = await listed(main)
    await client.request('watch', { checkouts: [main] })
    setStatus(main, TWO, 'active')
    fs.at(join(main, PLANNING)).emit('epics/somewhere.md')
    clock.runWaits()
    const changed = await client.notifications('task.changed')
    expect(changed.map((each) => each.params.key)).toEqual(['plan:ts-0002-bb'])
    expect(await client.request('get', { id: 'ts-9999-zz' })).toMatchObject({ result: {} })
    expect(client.pending()).toEqual([])
  })

  test('a list made for another checkout is not the baseline of this one', async () => {
    const main = repo()
    const linked = join(tempDir(), 'linked')
    git(main, 'worktree', 'add', '--quiet', '-b', 'side', linked)
    const { client } = await listed(main, linked)
    setStatus(main, ONE, 'active')
    await client.request('watch', { checkouts: [main] })
    // The main checkout was read for its baseline, as before: what is there is not news.
    expect(await client.request('get', { id: 'ts-9999-zz' })).toMatchObject({ result: {} })
    expect(client.pending()).toEqual([])
  })

  test('a list is the baseline of one watch, and a later watch reads a baseline of its own', async () => {
    const main = repo()
    const { client } = await listed(main)
    await client.request('watch', { checkouts: [main] })
    setStatus(main, ONE, 'active')
    await client.request('watch', { checkouts: [] })
    await client.request('watch', { checkouts: [main] })
    expect(await client.request('get', { id: 'ts-9999-zz' })).toMatchObject({ result: {} })
    expect(client.pending()).toEqual([])
  })
})

describe('watch: real file watchers', () => {
  test('a change on disk is heard, with no clock of the test', async () => {
    const main = repo()
    const client = connect({ watch: { env: ENV } })
    open.push(client)
    await client.request('initialize', { config: {}, root: main })
    expect((await client.request('watch', { checkouts: [main] })).error).toBeUndefined()
    setStatus(main, ONE, 'active')
    const [changed] = await client.notifications('task.changed')
    expect(changed?.params).toMatchObject({ key: 'plan:ts-0001-aa', to: state('active'), checkout: main })
  })

  test('over real stdio, a change made between list and watch is sent without another change', async () => {
    const main = repo()
    const child = await spawnProvider(main, SAFE_GIT)
    try {
      await child.request('list', { checkout: main })
      setStatus(main, ONE, 'active')
      expect(await child.request('watch', { checkouts: [main] })).toEqual({})
      const until = Date.now() + 20_000
      while (!child.notified.some((message) => message.method === 'task.changed')) {
        if (Date.now() > until) throw new Error(`no task.changed; the provider sent ${JSON.stringify(child.notified)}`)
        await new Promise((resolve) => setTimeout(resolve, 50))
      }
      expect(child.notified.find((message) => message.method === 'task.changed')?.params).toMatchObject({
        key: 'plan:ts-0001-aa',
        to: state('active'),
        checkout: main,
      })
    } finally {
      expect(await child.close()).toEqual({ code: 0, signal: null })
    }
  })

  test('over real stdio, the provider as a process hears a change and ends when its input ends', async () => {
    const main = repo()
    const child = await spawnProvider(main, SAFE_GIT)
    try {
      expect(await child.request('watch', { checkouts: [main] })).toEqual({})
      setStatus(main, ONE, 'active')
      const until = Date.now() + 20_000
      while (!child.notified.some((message) => message.method === 'task.changed')) {
        if (Date.now() > until) throw new Error(`no task.changed; the provider sent ${JSON.stringify(child.notified)}`)
        await new Promise((resolve) => setTimeout(resolve, 50))
      }
      expect(child.notified.find((message) => message.method === 'task.changed')?.params).toMatchObject({
        key: 'plan:ts-0001-aa',
        checkout: main,
      })
    } finally {
      expect(await child.close()).toEqual({ code: 0, signal: null })
    }
  })
})
