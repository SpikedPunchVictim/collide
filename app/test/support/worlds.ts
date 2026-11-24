// The scripted worlds the frame's tests and the fake-owner page share (ts-0179-ke). A world is a `World` of
// `loop-world.ts`: a `FakeOwner` over plain data. The page names one in its address (`#world=two-boards`), and a vitest
// test calls the same function, so a test in Node and a spec in Chrome see the same daemon.
import { FieldWorld, eightStatuses, mixedSources, onePlanSource } from './field-world.ts'
import { World, board } from './loop-world.ts'

/** Two boards with different default lines, and a third folder that `board.open` can add. */
export function twoBoards(): World {
  // A board with no task yet: the Field has nothing to show, and says so.
  const world = new FieldWorld()
  world.boards = [board('b-alpha', '/work/alpha'), board('b-beta', '/work/beta', 'develop')]
  world.folders.set('/work/gamma', board('b-gamma', '/work/gamma'))
  return world
}

/** A daemon that knows no board yet. */
export function noBoards(): World {
  return new World()
}

export const WORLDS: Readonly<Record<string, () => World>> = {
  'two-boards': twoBoards,
  'no-boards': noBoards,
  'field-one-source': onePlanSource,
  'field-eight': eightStatuses,
  'field-mixed': mixedSources,
}
