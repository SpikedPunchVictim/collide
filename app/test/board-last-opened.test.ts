import { afterEach, describe, expect, test } from 'vitest'
import { type Loop, createLoop } from '../src/loop/index.ts'
import { type LoopState, initialLoopState, loopReducer } from '../src/loop/state.ts'
import { World, board, missingBoard } from './support/loop-world.ts'

// The board a window opens on (ts-0218-sc; plan, Stage 6a, Line selector): the one the daemon says was opened or picked
// last, not the first in the list. The reducer's rule is tested on plain data; the loop's calls over a scripted owner.

function synced(state: LoopState, boards: ReturnType<typeof board>[]): LoopState {
  const sync = { items: [], followed: null, run: null, diff: null, diffProblem: null }
  return loopReducer(state, { type: 'synced', boards, ...sync })
}

describe('the board a window opens on', () => {
  test('is the one with the latest time, whatever its place in the list', () => {
    const state = synced(initialLoopState(), [
      board('b-1', '/work/one', 'main', 100),
      board('b-2', '/work/two', 'main', 300),
      board('b-3', '/work/three', 'main', 200),
    ])
    expect(state.boardId).toBe('b-2')
  })

  test('is the first in the list when no board has a time', () => {
    const state = synced(initialLoopState(), [board('b-1', '/work/one'), board('b-2', '/work/two')])
    expect(state.boardId).toBe('b-1')
  })

  test('is a board with a time over boards from before that have none', () => {
    const state = synced(initialLoopState(), [board('b-1', '/work/one'), board('b-2', '/work/two', 'main', 5)])
    expect(state.boardId).toBe('b-2')
  })

  test('is the board whose folder is gone when it was the one picked last', () => {
    const listed = [board('b-1', '/work/one', 'main', 1), missingBoard('b-2', '/work/old', 2)]
    const state = synced(initialLoopState(), listed)
    expect(state.boardId).toBe('b-2')
  })

  test('is no board when there are none', () => {
    expect(synced(initialLoopState(), []).boardId).toBeNull()
  })

  test('is left alone by a reload while the window’s own board is still listed', () => {
    const listed = [board('b-1', '/work/one', 'main', 1), board('b-2', '/work/two', 'main', 2)]
    const first = synced(initialLoopState(), listed)
    const chosen = loopReducer(first, { type: 'board_chosen', id: 'b-1' })
    const again = synced(chosen, [board('b-1', '/work/one', 'main', 1), board('b-2', '/work/two', 'main', 9)])
    expect(again.boardId).toBe('b-1')
  })

  test('is the latest again when the window’s own board is no longer listed', () => {
    const listed = [board('b-1', '/work/one', 'main', 1), board('b-2', '/work/two', 'main', 2)]
    const first = synced(initialLoopState(), listed)
    const chosen = loopReducer(first, { type: 'board_chosen', id: 'b-1' })
    const again = synced(chosen, [board('b-2', '/work/two', 'main', 2), board('b-3', '/work/three', 'main', 3)])
    expect(again.boardId).toBe('b-3')
  })
})

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

function twoBoards(): World {
  const world = new World()
  world.boards = [board('b-1', '/work/one', 'main', 100), board('b-2', '/work/two', 'main', 200)]
  world.folders.set('/work/three', board('b-3', '/work/three'))
  return world
}

describe('picking a board in the menu', () => {
  test('tells the daemon which board, and the window switches at once', async () => {
    const world = twoBoards()
    const loop = await pageOf(world)
    expect(loop.getState().boardId).toBe('b-2')
    loop.chooseBoard('b-1')
    expect(loop.getState().boardId).toBe('b-1')
    expect(world.callsTo('board.pick')).toEqual([{ board: 'b-1' }])
  })

  test('is what a window that starts or reloads afterwards opens on', async () => {
    const world = twoBoards()
    const loop = await pageOf(world)
    loop.chooseBoard('b-1')
    await expect.poll(() => world.boards.find((b) => b.id === 'b-1')?.last_opened_ms).toBeGreaterThan(200)
    const reloaded = await pageOf(world)
    expect(reloaded.getState().boardId).toBe('b-1')
  })

  test('still switches the window when the daemon refuses it', async () => {
    const world = twoBoards()
    world.refusePick = 'No.'
    const loop = await pageOf(world)
    loop.chooseBoard('b-1')
    await expect.poll(() => world.callsTo('board.pick').length).toBe(1)
    expect(loop.getState().boardId).toBe('b-1')
  })

  test('is not sent to the daemon from a window started with defaults, which saves nothing', async () => {
    const world = twoBoards()
    const loop = await pageOf(world, { defaults: true })
    loop.chooseBoard('b-1')
    expect(loop.getState().boardId).toBe('b-1')
    expect(world.callsTo('board.pick')).toEqual([])
  })

})

describe('opening a folder in the window', () => {
  test('is a pick of the board it opens, and what a window that starts afterwards opens on', async () => {
    const world = twoBoards()
    const loop = await pageOf(world)
    await loop.openFolder('/work/three')
    expect(loop.getState().boardId).toBe('b-3')
    await expect.poll(() => world.callsTo('board.pick')).toEqual([{ board: 'b-3' }])
    await expect.poll(() => world.boards.find((b) => b.id === 'b-3')?.last_opened_ms).toBeGreaterThan(200)
    expect((await pageOf(world)).getState().boardId).toBe('b-3')
  })

  test('is not a pick from a window started with defaults, so a window after it opens as it did', async () => {
    const world = twoBoards()
    const loop = await pageOf(world, { defaults: true })
    await loop.openFolder('/work/three')
    expect(loop.getState().boardId).toBe('b-3')
    expect(world.callsTo('board.pick')).toEqual([])
    expect((await pageOf(world)).getState().boardId).toBe('b-2')
  })

  test('is not a pick when the folder did not open', async () => {
    const world = twoBoards()
    const loop = await pageOf(world)
    await loop.openFolder('/work/nowhere')
    expect(world.callsTo('board.pick')).toEqual([])
  })
})
