import { readFileSync, readdirSync } from 'node:fs'
import { join } from 'node:path'
import { afterEach, describe, expect, test } from 'vitest'
import { type Loop, createLoop } from '../src/loop/index.ts'
import { OwnerError } from '../src/owner/index.ts'
import { World, board, claude, diff, permissionItem, reviewItem, run } from './support/loop-world.ts'

// The thin-loop page's state and its calls, over a scripted `FakeOwner` (ts-0081-wm; 0.15: a reducer for the page's
// state, so its tests need no browser). A test says what the daemon holds, pushes the event that tells the page, and
// reads the state the page would draw.

const open: Loop[] = []

async function pageOf(world: World): Promise<Loop> {
  const loop = createLoop(world.owner)
  open.push(loop)
  await loop.open()
  await loop.idle()
  return loop
}

afterEach(() => {
  for (const loop of open.splice(0)) loop.close()
})

/** A world with one board, where the page is ready for a prompt. */
function oneBoard(): World {
  const world = new World()
  world.boards = [board('b-1', '/work/repo')]
  return world
}

describe('the repository', () => {
  test('the boards come from board.list and the first is chosen', async () => {
    const world = new World()
    world.boards = [board('b-1', '/work/one'), board('b-2', '/work/two')]
    const loop = await pageOf(world)
    expect(loop.getState().boards.map((b) => b.id)).toEqual(['b-1', 'b-2'])
    expect(loop.getState().boardId).toBe('b-1')
  })

  test('a person can choose another board', async () => {
    const world = new World()
    world.boards = [board('b-1', '/work/one'), board('b-2', '/work/two')]
    const loop = await pageOf(world)
    loop.chooseBoard('b-2')
    expect(loop.getState().boardId).toBe('b-2')
  })

  test('a typed folder opens through board.open and is chosen', async () => {
    const world = oneBoard()
    world.folders.set('/work/new', board('b-9', '/work/new'))
    const loop = await pageOf(world)
    await loop.openFolder('/work/new')
    expect(world.callsTo('board.open')).toEqual([{ root: '/work/new' }])
    expect(loop.getState().boardId).toBe('b-9')
    expect(loop.getState().boards.map((b) => b.id)).toEqual(['b-1', 'b-9'])
  })

  test('a method handed to a component as a callback still works, as the page hands them', async () => {
    const world = oneBoard()
    world.folders.set('/work/new', board('b-9', '/work/new'))
    const loop = await pageOf(world)
    const { openFolder, chooseBoard } = loop
    await openFolder('/work/new')
    expect(loop.getState().boardId).toBe('b-9')
    chooseBoard('b-1')
    expect(loop.getState().boardId).toBe('b-1')
  })

  test('a folder that is not a checkout says what is wrong and chooses nothing', async () => {
    const world = oneBoard()
    const loop = await pageOf(world)
    await loop.openFolder('/work/plain')
    expect(loop.getState().folderProblems).toEqual(['This folder is not a git repository.'])
    expect(loop.getState().boardId).toBe('b-1')
  })

  test('a blank folder makes no call', async () => {
    const world = oneBoard()
    const loop = await pageOf(world)
    await loop.openFolder('   ')
    expect(world.callsTo('board.open')).toEqual([])
  })
})

describe('the two version chips', () => {
  test('the page reads daemon.agent_versions and keeps the Claude entry', async () => {
    const world = oneBoard()
    world.versions = { agents: [{ ...claude(), agent: 'other' }, claude()] }
    const loop = await pageOf(world)
    expect(loop.getState().versions?.in_use).toBe('2.1.325')
  })

  test('a daemon that cannot read the versions leaves the page working, with no chips', async () => {
    const world = oneBoard()
    world.versions = 'fails'
    const loop = await pageOf(world)
    expect(loop.getState().versions).toBeNull()
    expect(loop.getState().boardId).toBe('b-1')
  })
})

describe('a prompt', () => {
  test('creates a scratch task and starts collide/quick on it, with the right board and key', async () => {
    const world = oneBoard()
    const loop = await pageOf(world)
    await loop.startRun('Fix the typo in the readme')
    expect(world.callsTo('task.apply')).toEqual([
      { board: 'b-1', source: 'scratch', apply: { op: 'create', args: { prompt: 'Fix the typo in the readme' } } },
    ])
    expect(world.callsTo('run.start')).toEqual([{ board: 'b-1', tasks: ['scratch:1'], workflow: 'collide/quick' }])
    const order = world.owner.calls.map((c) => c.method).filter((m) => m === 'task.apply' || m === 'run.start')
    expect(order).toEqual(['task.apply', 'run.start'])
  })

  test('is followed: the run, its task and its words are in the state', async () => {
    const world = oneBoard()
    const loop = await pageOf(world)
    await loop.startRun('Fix the typo in the readme')
    await loop.idle()
    expect(loop.getState().followed).toEqual({
      board: 'b-1',
      task: 'scratch:1',
      prompt: 'Fix the typo in the readme',
      run: 'r-1',
    })
    expect(loop.getState().run?.state).toEqual({ state: 'running' })
  })

  test('with only blanks makes no call', async () => {
    const world = oneBoard()
    const loop = await pageOf(world)
    await loop.startRun('  \n ')
    expect(world.callsTo('task.apply')).toEqual([])
  })

  test('with no board makes no call and says to choose one', async () => {
    const world = new World()
    const loop = await pageOf(world)
    await loop.startRun('Fix it')
    expect(world.callsTo('task.apply')).toEqual([])
    expect(loop.getState().refusal).toBe('Choose a repository first.')
  })

  test('refused at the start shows its reason, and a second press makes no second task', async () => {
    const world = oneBoard()
    world.refuseStart = 'The checkout is on a detached HEAD.'
    const loop = await pageOf(world)
    await loop.startRun('Fix it')
    expect(loop.getState().refusal).toBe('The checkout is on a detached HEAD.')
    expect(loop.getState().followed).toBeNull()
    world.refuseStart = null
    await loop.startRun('Fix it')
    expect(world.callsTo('task.apply')).toHaveLength(1)
    expect(world.callsTo('run.start')).toHaveLength(2)
    expect(loop.getState().followed?.run).toBe('r-1')
    expect(loop.getState().refusal).toBeNull()
  })

  test('a different prompt after a refusal makes a new task', async () => {
    const world = oneBoard()
    world.refuseStart = 'No.'
    const loop = await pageOf(world)
    await loop.startRun('First')
    world.refuseStart = null
    await loop.startRun('Second')
    expect(world.callsTo('task.apply')).toHaveLength(2)
  })

  test('refused at the create shows its reason and starts no run', async () => {
    const world = oneBoard()
    world.refuseCreate = 'The prompt is too long.'
    const loop = await pageOf(world)
    await loop.startRun('Fix it')
    expect(loop.getState().refusal).toBe('The prompt is too long.')
    expect(world.callsTo('run.start')).toEqual([])
  })
})

describe('the run and its events', () => {
  test('a run event leads the page from working to the review item, with the diff', async () => {
    const world = oneBoard()
    const loop = await pageOf(world)
    await loop.startRun('Fix it')
    await loop.idle()
    expect(loop.getState().items).toEqual([])

    world.finish('r-1')
    world.announce('r-1')
    await loop.idle()

    const state = loop.getState()
    expect(state.run?.state).toEqual({ state: 'ended', outcome: 'done' })
    expect(state.items.map((i) => i.kind)).toEqual(['review'])
    expect(state.diff?.files.map((f) => f.path)).toEqual(['src/a.ts'])
    expect(world.callsTo('run.diff')).toEqual([{ run: 'r-1' }])
  })

  test('a queue.changed notice reloads the Queue', async () => {
    const world = oneBoard()
    const loop = await pageOf(world)
    await loop.startRun('Fix it')
    await loop.idle()
    world.items = [permissionItem('q-1', 'r-1', 'scratch:1')]
    world.owner.push({ method: 'queue.changed', params: { count: 1, added: ['q-1'], removed: [] } })
    await loop.idle()
    expect(loop.getState().items.map((i) => i.id)).toEqual(['q-1'])
  })

  test('an event that says nothing about runs or the Queue reloads nothing', async () => {
    const world = oneBoard()
    const loop = await pageOf(world)
    await loop.startRun('Fix it')
    await loop.idle()
    const before = world.owner.calls.length
    world.owner.push({
      method: 'daemon.event',
      params: {
        seq: 9,
        wall_ms: 1,
        actor: { kind: 'workflow' },
        experiment: null,
        body: { event: 'metric.reported', run: 'r-1', phase: 'agent', name: 'x', value: 1 } as never,
      },
    })
    await loop.idle()
    expect(world.owner.calls.length).toBe(before)
  })

  test('events that come together are met with one reload', async () => {
    const world = oneBoard()
    const loop = await pageOf(world)
    await loop.startRun('Fix it')
    await loop.idle()
    const before = world.callsTo('run.get').length
    for (let i = 0; i < 5; i++) world.announce('r-1')
    await loop.idle()
    expect(world.callsTo('run.get').length - before).toBeLessThanOrEqual(2)
  })

  test('after close the page ignores the channel', async () => {
    const world = oneBoard()
    const loop = await pageOf(world)
    loop.close()
    const before = world.owner.calls.length
    world.announce('r-1')
    await loop.idle()
    expect(world.owner.calls.length).toBe(before)
  })
})

/** The page with a run that has asked for a permission. */
async function asked(input?: unknown): Promise<{ world: World; loop: Loop }> {
  const world = oneBoard()
  const loop = await pageOf(world)
  await loop.startRun('Run the tests')
  await loop.idle()
  world.setRun('r-1', { state: { state: 'paused', cause: 'ask_human' } })
  world.items = [permissionItem('q-1', 'r-1', 'scratch:1', input)]
  world.announce('r-1')
  await loop.idle()
  return { world, loop }
}

describe('a permission request', () => {
  test('is in the Queue with what it asks', async () => {
    const { loop } = await asked({ command: 'cargo test' })
    const item = loop.getState().items[0]!
    expect(item.kind).toBe('permission')
    expect(item.permission?.input).toEqual({ command: 'cargo test' })
  })

  test('allow once answers with allow, and the item goes when the daemon takes the answer', async () => {
    const { world, loop } = await asked({ command: 'cargo test' })
    world.onAnswer = () => {
      world.items = []
      world.setRun('r-1', { state: { state: 'running' } })
    }
    await loop.allowOnce('q-1')
    expect(world.callsTo('queue.answer')).toEqual([
      { item: 'q-1', answer: { type: 'permission', reply: { type: 'allow' } } },
    ])
    expect(loop.getState().items).toEqual([])
    expect(loop.getState().run?.state).toEqual({ state: 'running' })
  })

  test('deny answers with deny', async () => {
    const { world, loop } = await asked()
    await loop.deny('q-1')
    expect(world.callsTo('queue.answer')).toEqual([
      { item: 'q-1', answer: { type: 'permission', reply: { type: 'deny' } } },
    ])
  })

  test('no code of the page can answer allow always', () => {
    // The answer type has no such reply (0.3), and the agent's own `allow_always` option is never read. Reading the
    // page's sources keeps a later edit from adding one.
    const folder = join(import.meta.dirname, '..', 'src', 'loop')
    for (const file of readdirSync(folder)) {
      const text = readFileSync(join(folder, file), 'utf8')
      expect(text, file).not.toMatch(/allow_always|always allow|allowAlways/i)
    }
  })

  test('an answer sent twice at once is sent once', async () => {
    const { world, loop } = await asked()
    await Promise.all([loop.allowOnce('q-1'), loop.allowOnce('q-1')])
    expect(world.callsTo('queue.answer')).toHaveLength(1)
  })
})

/** The page with a run that finished and waits for review. */
async function inReview(): Promise<{ world: World; loop: Loop }> {
  const world = oneBoard()
  const loop = await pageOf(world)
  await loop.startRun('Fix the typo')
  await loop.idle()
  world.finish('r-1')
  world.announce('r-1')
  await loop.idle()
  return { world, loop }
}

describe('the review item', () => {
  test('picking a file loads its patch, and picking nothing clears it', async () => {
    const { world, loop } = await inReview()
    await loop.pickFile('src/a.ts')
    expect(world.callsTo('run.diff')).toContainEqual({ run: 'r-1', path: 'src/a.ts' })
    expect(loop.getState().picked).toBe('src/a.ts')
    expect(loop.getState().patch?.text).toContain('+new')
    await loop.pickFile(null)
    expect(loop.getState().picked).toBeNull()
    expect(loop.getState().patch).toBeNull()
  })

  test('accept sends accept, and a merged answer is told in words', async () => {
    const { world, loop } = await inReview()
    world.answerResult = () => ({ outcome: 'merged', commit: 'c0ffee' })
    world.onAnswer = () => {
      world.items = []
      world.setRun('r-1', { state: { state: 'ended', outcome: 'accepted' } })
    }
    await loop.accept('q-review')
    expect(world.callsTo('queue.answer')).toEqual([{ item: 'q-review', answer: { type: 'accept' } }])
    expect(loop.getState().items).toEqual([])
    expect(loop.getState().notice).toBe('Accepted. The work is merged into your checkout.')
    expect(loop.getState().refusal).toBeNull()
  })

  test('a refused accept shows the reason and the item stays', async () => {
    const { world, loop } = await inReview()
    world.answerResult = () => ({
      outcome: 'refused',
      message: 'Your checkout is on `dev`, not `main`. Switch to `main`, then accept again.',
    })
    await loop.accept('q-review')
    expect(loop.getState().refusal).toBe(
      'Your checkout is on `dev`, not `main`. Switch to `main`, then accept again.',
    )
    expect(loop.getState().items.map((i) => i.id)).toEqual(['q-review'])
    expect(loop.getState().notice).toBeNull()
  })

  test('a refusal goes when the person tries something else that works', async () => {
    const { world, loop } = await inReview()
    world.answerResult = () => ({ outcome: 'refused', message: 'No.' })
    await loop.accept('q-review')
    world.answerResult = () => ({ outcome: 'done' })
    await loop.accept('q-review')
    expect(loop.getState().refusal).toBeNull()
  })

  test('an accept that was queued says it needs another step', async () => {
    const { world, loop } = await inReview()
    world.answerResult = () => ({ outcome: 'queued', item: 'q-2' })
    await loop.accept('q-review')
    expect(loop.getState().notice).toBe('Collide could not finish this and put another item in the Queue.')
  })

  test('an answer that fails shows the failure and the item stays', async () => {
    const { world, loop } = await inReview()
    world.owner.handle('queue.answer', () => {
      throw new OwnerError(-32000, 'the connection dropped; the answer may have taken effect')
    })
    await loop.accept('q-review')
    expect(loop.getState().refusal).toBe('the connection dropped; the answer may have taken effect')
    expect(loop.getState().items.map((i) => i.id)).toEqual(['q-review'])
  })

  test('send back sends the words as given and follows the new run', async () => {
    const { world, loop } = await inReview()
    world.onAnswer = () => {
      world.items = []
      world.addRun('b-1', run('r-2', 'scratch:1', { started_ms: 6_000 }))
    }
    await loop.reprompt('q-review', '  Also fix the title.\n')
    expect(world.callsTo('queue.answer')).toEqual([
      { item: 'q-review', answer: { type: 'reprompt', prompt: '  Also fix the title.\n' } },
    ])
    expect(loop.getState().followed?.run).toBe('r-2')
    expect(loop.getState().run?.id).toBe('r-2')
    expect(loop.getState().diff).toBeNull()
    expect(loop.getState().picked).toBeNull()
  })

  test('send back with blank words makes no call', async () => {
    const { world, loop } = await inReview()
    await loop.reprompt('q-review', '  \n')
    expect(world.callsTo('queue.answer')).toEqual([])
  })

  test('discard needs a second press, and the first press sends nothing', async () => {
    const { world, loop } = await inReview()
    await loop.discard('q-review')
    expect(world.callsTo('queue.answer')).toEqual([])
    expect(loop.getState().confirmingDiscard).toBe(true)
    await loop.discard('q-review')
    expect(world.callsTo('queue.answer')).toEqual([{ item: 'q-review', answer: { type: 'discard' } }])
    expect(loop.getState().confirmingDiscard).toBe(false)
  })

  test('keeping the work after the first press sends nothing and the next press asks again', async () => {
    const { world, loop } = await inReview()
    await loop.discard('q-review')
    loop.keep()
    expect(loop.getState().confirmingDiscard).toBe(false)
    await loop.discard('q-review')
    expect(world.callsTo('queue.answer')).toEqual([])
    expect(loop.getState().confirmingDiscard).toBe(true)
  })

  test('another answer ends a question about discarding', async () => {
    const { loop } = await inReview()
    await loop.discard('q-review')
    await loop.reprompt('q-review', 'More')
    expect(loop.getState().confirmingDiscard).toBe(false)
  })
})

describe('the notice after a re-prompt', () => {
  const sentBack = 'Sent back. The agent is working on it again.'

  /** A re-prompt on the review item of r-1; the daemon starts r-2 on the same task. */
  async function reprompted(): Promise<{ world: World; loop: Loop }> {
    const { world, loop } = await inReview()
    world.onAnswer = () => {
      world.items = []
      world.addRun('b-1', run('r-2', 'scratch:1', { started_ms: 6_000 }))
    }
    await loop.reprompt('q-review', 'Also fix the title.')
    return { world, loop }
  }

  test('a re-prompt followed by the first events of the new run keeps the notice', async () => {
    const { world, loop } = await reprompted()
    world.announce('r-2')
    await loop.idle()
    expect(loop.getState().followed?.run).toBe('r-2')
    expect(loop.getState().notice).toBe(sentBack)
  })

  test('the review item of the new run clears the notice', async () => {
    const { world, loop } = await reprompted()
    world.finish('r-2', 'q-2')
    world.announce('r-2')
    await loop.idle()
    expect(loop.getState().run?.id).toBe('r-2')
    expect(loop.getState().items.map((i) => i.id)).toEqual(['q-2'])
    expect(loop.getState().notice).toBeNull()
  })

  test('the old run, still ended and listed before the new one starts, does not clear the notice', async () => {
    const { world, loop } = await inReview()
    world.onAnswer = () => {
      world.items = []
    }
    await loop.reprompt('q-review', 'Also fix the title.')
    world.announce('r-1')
    await loop.idle()
    expect(loop.getState().run?.id).toBe('r-1')
    expect(loop.getState().notice).toBe(sentBack)
  })

  test('a new prompt clears the notice', async () => {
    const { loop } = await reprompted()
    await loop.startRun('Something else')
    await loop.idle()
    expect(loop.getState().notice).toBeNull()
  })
})

describe('closing the window and opening it again', () => {
  test('a run that is live shows the same run, with its words', async () => {
    const world = oneBoard()
    world.seed('b-1', 'scratch:7', 'Rename the module', run('r-7', 'scratch:7'))
    const loop = await pageOf(world)
    const state = loop.getState()
    expect(state.followed).toEqual({ board: 'b-1', task: 'scratch:7', prompt: 'Rename the module', run: 'r-7' })
    expect(state.run?.id).toBe('r-7')
    expect(state.run?.state).toEqual({ state: 'running' })
    expect(state.boardId).toBe('b-1')
    expect(world.callsTo('run.list').length).toBeGreaterThan(0)
    expect(world.callsTo('queue.list').length).toBeGreaterThan(0)
  })

  test('a review item that waits shows the same item, its run and its diff', async () => {
    const world = oneBoard()
    world.seed('b-1', 'scratch:7', 'Rename the module', run('r-7', 'scratch:7'))
    world.finish('r-7')
    const loop = await pageOf(world)
    const state = loop.getState()
    expect(state.items.map((i) => i.id)).toEqual(['q-review'])
    expect(state.followed?.run).toBe('r-7')
    expect(state.run?.state).toEqual({ state: 'ended', outcome: 'done' })
    expect(state.diff?.files).toHaveLength(1)
  })

  test('the run is found on the board that holds it when there are two', async () => {
    const world = new World()
    world.boards = [board('b-1', '/work/one'), board('b-2', '/work/two')]
    world.seed('b-2', 'scratch:1', 'On the second board', run('r-1', 'scratch:1'))
    const loop = await pageOf(world)
    expect(loop.getState().followed).toEqual({
      board: 'b-2',
      task: 'scratch:1',
      prompt: 'On the second board',
      run: 'r-1',
    })
    expect(loop.getState().boardId).toBe('b-2')
  })

  test('a waiting item is followed before a newer run that nothing waits on', async () => {
    const world = oneBoard()
    world.seed('b-1', 'scratch:1', 'Old', run('r-1', 'scratch:1', { started_ms: 1_000 }))
    world.seed('b-1', 'scratch:2', 'New', run('r-2', 'scratch:2', { started_ms: 9_000 }))
    world.items = [permissionItem('q-1', 'r-1', 'scratch:1')]
    const loop = await pageOf(world)
    expect(loop.getState().followed?.run).toBe('r-1')
  })

  test('with no run and no item the page shows an empty loop', async () => {
    const world = oneBoard()
    const loop = await pageOf(world)
    expect(loop.getState().followed).toBeNull()
    expect(loop.getState().run).toBeNull()
    expect(loop.getState().loading).toBe(false)
  })

  test('another item of the Queue can be opened, and the page then follows its run', async () => {
    const world = oneBoard()
    world.seed('b-1', 'scratch:1', 'First', run('r-1', 'scratch:1'))
    world.seed('b-1', 'scratch:2', 'Second', run('r-2', 'scratch:2', { started_ms: 3_000 }))
    world.items = [permissionItem('q-1', 'r-1', 'scratch:1'), reviewItem('q-2', 'r-2', 'scratch:2')]
    world.diffs.set('r-2', diff())
    const loop = await pageOf(world)
    expect(loop.getState().followed?.run).toBe('r-1')
    await loop.openItem('q-2')
    expect(loop.getState().followed).toMatchObject({ run: 'r-2', prompt: 'Second' })
  })

  test('a load that fails says so in words and the page can load again', async () => {
    const world = oneBoard()
    world.owner.handle('board.list', () => {
      throw new OwnerError(-32603, 'the daemon is not running')
    })
    const loop = await pageOf(world)
    expect(loop.getState().problem).toBe('Could not reach Collide: the daemon is not running')
    expect(loop.getState().loading).toBe(false)
  })
})

describe('the connection', () => {
  test('a notice that the stream ended shows, and one that it came back clears', async () => {
    const world = oneBoard()
    const loop = await pageOf(world)
    world.owner.push({ method: 'shell.connection', params: { state: 'lost' } })
    expect(loop.getState().connection).toBe('lost')
    world.owner.push({ method: 'shell.connection', params: { state: 'back' } })
    await loop.idle()
    expect(loop.getState().connection).toBe('live')
  })

  test('on its return the page reloads with run.list, run.get and queue.list', async () => {
    const world = oneBoard()
    world.seed('b-1', 'scratch:1', 'Fix it', run('r-1', 'scratch:1'))
    const loop = await pageOf(world)
    world.owner.push({ method: 'shell.connection', params: { state: 'lost' } })
    const before = world.owner.calls.length
    world.owner.push({ method: 'shell.connection', params: { state: 'back' } })
    await loop.idle()
    const reloaded = world.owner.calls.slice(before).map((c) => c.method)
    expect(reloaded).toContain('run.list')
    expect(reloaded).toContain('run.get')
    expect(reloaded).toContain('queue.list')
  })

  test('what changed in the gap is shown after the return', async () => {
    const world = oneBoard()
    world.seed('b-1', 'scratch:1', 'Fix it', run('r-1', 'scratch:1'))
    const loop = await pageOf(world)
    expect(loop.getState().run?.state).toEqual({ state: 'running' })
    world.owner.push({ method: 'shell.connection', params: { state: 'lost' } })
    // The daemon's log no longer holds this: the page never hears of it as an event.
    world.finish('r-1')
    world.owner.push({ method: 'shell.connection', params: { state: 'back' } })
    await loop.idle()
    expect(loop.getState().run?.state).toEqual({ state: 'ended', outcome: 'done' })
    expect(loop.getState().items.map((i) => i.kind)).toEqual(['review'])
    expect(loop.getState().diff?.files).toHaveLength(1)
  })

  test('a run that began in the gap is found by the reload', async () => {
    const world = oneBoard()
    const loop = await pageOf(world)
    expect(loop.getState().followed).toBeNull()
    world.owner.push({ method: 'shell.connection', params: { state: 'lost' } })
    world.seed('b-1', 'scratch:3', 'Started elsewhere', run('r-3', 'scratch:3'))
    world.owner.push({ method: 'shell.connection', params: { state: 'back' } })
    await loop.idle()
    expect(loop.getState().followed?.run).toBe('r-3')
  })

  test('a reload that fails says so and keeps what the page had', async () => {
    const world = oneBoard()
    world.seed('b-1', 'scratch:1', 'Fix it', run('r-1', 'scratch:1'))
    const loop = await pageOf(world)
    world.owner.handle('queue.list', () => {
      throw new OwnerError(-32603, 'the daemon is not running')
    })
    world.owner.push({ method: 'shell.connection', params: { state: 'back' } })
    await loop.idle()
    expect(loop.getState().problem).toBe('Could not reach Collide: the daemon is not running')
    expect(loop.getState().run?.id).toBe('r-1')
  })
})
