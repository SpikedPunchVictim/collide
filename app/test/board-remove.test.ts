import { afterEach, describe, expect, test } from 'vitest'
import { type Loop, createLoop } from '../src/loop/index.ts'
import { World, board, missingBoard } from './support/loop-world.ts'

// Removing a board from Collide (ts-0222-gx; plan, Stage 6a, Line selector). The loop sends `board.remove`, and a
// window whose board was removed, by itself or by another window, moves to the board it picked last that is still
// listed, or to the first-run page when none is. The daemon's own rules (a live run, a defaults connection) are in
// `host/tests/board_remove.rs`; here the daemon's refusal is only shown in its words.

const open: Loop[] = []

async function pageOf(world: World, options?: { defaults: boolean }): Promise<Loop> {
  const loop = createLoop(world.owner, options)
  open.push(loop)
  await loop.open()
  await loop.idle()
  return loop
}

afterEach(() => {
  for (const loop of open.splice(0)) loop.close()
})

function threeBoards(): World {
  const world = new World()
  world.boards = [
    board('b-1', '/work/one', 'main', 100),
    board('b-2', '/work/two', 'main', 300),
    board('b-3', '/work/three', 'main', 200),
  ]
  return world
}

describe('removing the board the window is on', () => {
  test('asks the daemon to remove it, and the board leaves the list', async () => {
    const world = threeBoards()
    const loop = await pageOf(world)

    const problem = await loop.removeBoard('b-2')

    expect(problem).toBeNull()
    expect(world.callsTo('board.remove')).toEqual([{ board: 'b-2' }])
    expect(loop.getState().boards.map((b) => b.id)).toEqual(['b-1', 'b-3'])
  })

  test('moves the window to the board picked last that is still listed', async () => {
    const world = threeBoards()
    const loop = await pageOf(world)
    expect(loop.getState().boardId).toBe('b-2')

    await loop.removeBoard('b-2')

    expect(loop.getState().boardId).toBe('b-3')
  })

  test('moves the window to the first-run page when no board is left', async () => {
    const world = new World()
    world.boards = [board('b-1', '/work/one', 'main', 100)]
    const loop = await pageOf(world)

    await loop.removeBoard('b-1')

    expect(loop.getState().boards).toEqual([])
    expect(loop.getState().boardId).toBeNull()
  })

  test('works on a board whose folder is gone', async () => {
    const world = new World()
    world.boards = [board('b-1', '/work/one', 'main', 100), missingBoard('b-2', '/work/old', 200)]
    const loop = await pageOf(world)
    expect(loop.getState().boardId).toBe('b-2')

    await loop.removeBoard('b-2')

    expect(loop.getState().boardId).toBe('b-1')
  })

  test('leaves a board that is not the one in use, and the window where it is', async () => {
    const world = threeBoards()
    const loop = await pageOf(world)

    await loop.removeBoard('b-1')

    expect(loop.getState().boardId).toBe('b-2')
    expect(loop.getState().boards.map((b) => b.id)).toEqual(['b-2', 'b-3'])
  })
})

describe('a refused removal', () => {
  test('is the daemon’s words, and the board stays listed and in use', async () => {
    const world = threeBoards()
    world.removeRefusals.set('b-2', 'run r-7 is in progress on the board b-2, so it was not removed.')
    const loop = await pageOf(world)

    const problem = await loop.removeBoard('b-2')

    expect(problem).toBe('run r-7 is in progress on the board b-2, so it was not removed.')
    expect(loop.getState().boards.map((b) => b.id)).toEqual(['b-1', 'b-2', 'b-3'])
    expect(loop.getState().boardId).toBe('b-2')
  })
})

describe('another window removed a board', () => {
  test('moves this window off that board when the daemon says so', async () => {
    const world = threeBoards()
    const loop = await pageOf(world)
    expect(loop.getState().boardId).toBe('b-2')

    world.removedElsewhere('b-2')

    await expect.poll(() => loop.getState().boardId).toBe('b-3')
    expect(loop.getState().boards.map((b) => b.id)).toEqual(['b-1', 'b-3'])
  })

  test('leaves this window where it is when the board was another', async () => {
    const world = threeBoards()
    const loop = await pageOf(world)

    world.removedElsewhere('b-1')

    await expect.poll(() => loop.getState().boards.map((b) => b.id)).toEqual(['b-2', 'b-3'])
    expect(loop.getState().boardId).toBe('b-2')
  })

  test('shows the first-run page when it was the last board', async () => {
    const world = new World()
    world.boards = [board('b-1', '/work/one', 'main', 100)]
    const loop = await pageOf(world)

    world.removedElsewhere('b-1')

    await expect.poll(() => loop.getState().boards).toEqual([])
    expect(loop.getState().boardId).toBeNull()
  })
})

describe('opening the folder of a removed board', () => {
  test('lists the board again, as the same board', async () => {
    const world = threeBoards()
    const loop = await pageOf(world)
    await loop.removeBoard('b-2')

    const outcome = await loop.openChecked('/work/two')

    expect(outcome.opened).toBe(true)
    expect(loop.getState().boardId).toBe('b-2')
    expect(loop.getState().boards.map((b) => b.id)).toContain('b-2')
  })
})
