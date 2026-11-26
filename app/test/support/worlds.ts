// The scripted worlds the frame's tests and the fake-owner page share (ts-0179-ke). A world is a `World` of
// `loop-world.ts`: a `FakeOwner` over plain data. The page names one in its address (`#world=two-boards`), and a vitest
// test calls the same function, so a test in Node and a spec in Chrome see the same daemon.
import { changesWorld, peekWorld } from './changes-world.ts'
import {
  FieldWorld,
  drawerWorld,
  eightStatuses,
  epicsStripWorld,
  filtersWorld,
  manyEpicsWorld,
  mixedSources,
  onePlanSource,
  walkWorld,
} from './field-world.ts'
import {
  emptyBoardWorld,
  firstRunWorld,
  shareRefusedWorld,
  sharedBoardWorld,
  trustBoardWorld,
} from './first-run-world.ts'
import { HOSTILE_BODY } from './hostile.ts'
import { World, board, missingBoard, permissionItem, run } from './loop-world.ts'
import {
  settingsProblemsWorld,
  settingsRefusedWorld,
  settingsSecretRefusedWorld,
  settingsWorld,
} from './settings-world.ts'

/** Two boards with different default lines, and a third folder that `board.open` can add. */
export function twoBoards(): World {
  // A board with no task yet: the Field has nothing to show, and says so.
  const world = new FieldWorld()
  world.boards = [board('b-alpha', '/work/alpha'), board('b-beta', '/work/beta', 'develop')]
  world.folders.set('/work/gamma', board('b-gamma', '/work/gamma'))
  return world
}

/**
 * One board that is there and one whose folder is gone (ts-0191-nv), and three folders to locate it at: `/work/new` is
 * accepted, `/work/unrecorded` is accepted with a note that the check could not run, and `/work/other` is refused as
 * another repository. `/work/gamma` can be opened as a new board.
 */
export function missingBoardWorld(): World {
  const world = new FieldWorld()
  world.boards = [board('b-alpha', '/work/alpha'), missingBoard('b-gone', '/work/gone')]
  world.folders.set('/work/gamma', board('b-gamma', '/work/gamma'))
  world.locateRefusals.set(
    '/work/other',
    '`/work/other` is a different repository: its first commit is aaaaaaaaaaaa, and the board\'s is bbbbbbbbbbbb.',
  )
  world.locateNotes.set('/work/unrecorded', 'Collide had no record of the old repository, so it could not check.')
  return world
}

/**
 * A task whose body is hostile, with a run, and an agent's permission request whose reason is the same text
 * (ts-0183-ee). The loop page shows both, which are the two places that show task or agent text before the drawer.
 */
export function hostileText(): World {
  const world = new FieldWorld()
  world.seed('b-alpha', 'scratch:1', HOSTILE_BODY, run('r-1', 'scratch:1'))
  const item = permissionItem('q-1', 'r-1', 'scratch:1')
  item.permission!.reason = HOSTILE_BODY
  world.items = [item]
  return world
}

/** A daemon that knows no board yet. */
export function noBoards(): World {
  return new World()
}

export const WORLDS: Readonly<Record<string, () => World>> = {
  'two-boards': twoBoards,
  'no-boards': noBoards,
  'first-run': firstRunWorld,
  'empty-board': emptyBoardWorld,
  'shared-board': sharedBoardWorld,
  'share-refused': shareRefusedWorld,
  'trust-board': trustBoardWorld,
  'missing-board': missingBoardWorld,
  'hostile-text': hostileText,
  'field-one-source': onePlanSource,
  'field-eight': eightStatuses,
  'field-mixed': mixedSources,
  'field-drawer': drawerWorld,
  'field-walk': walkWorld,
  'field-filters': filtersWorld,
  'field-epics-50': manyEpicsWorld,
  'field-epics-strip': epicsStripWorld,
  'field-changes': changesWorld,
  'field-peek': peekWorld,
  settings: settingsWorld,
  'settings-problems': settingsProblemsWorld,
  'settings-refused': settingsRefusedWorld,
  'settings-secret-refused': settingsSecretRefusedWorld,
}
