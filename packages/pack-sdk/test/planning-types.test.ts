import { expect, test } from 'vitest'
import type { ChangeOutput, KindDeclInput, PackManifestInput } from '../src/index.ts'

// These checks run in `pnpm typecheck`: vitest strips the types, so the `@ts-expect-error` lines are the assertions.

test('a spike kind, a derived point and a checklist template are accepted', () => {
  const kind: KindDeclInput = {
    name: 'spike',
    statuses: [{ name: 'planned', category: 'ready' }],
    ops: [{ name: 'conclude', movesTo: 'done' }],
  }
  const manifest: PackManifestInput = {
    points: { 'spike.before_conclude': { on: 'task.before_status', match: ['done'] } },
    checklists: { spike: { version: 1, gates: 'ready', items: [{ id: 'design', text: 'Designed' }] } },
  }
  expect([kind.name, Object.keys(manifest.points ?? {})]).toEqual(['spike', ['spike.before_conclude']])
})

test('a run-made change has an agent actor, and run and phase are optional', () => {
  const change: ChangeOutput = {
    op: null,
    args: {},
    from: null,
    to: { status: 'review', category: 'review' },
    diff: { status: { from: null, to: 'review' }, labels: { added: ['x'], removed: [] } },
    categoryMoved: true,
    via: 'run',
    actor: { kind: 'agent', run: 'r-77', phase: 'build' },
    batch: null,
    effects: [],
    effectOf: null,
    renamed: [],
    checkout: null,
    files: [],
  }
  const bare: ChangeOutput['actor'] = { kind: 'agent' }
  // @ts-expect-error an actor kind outside the three.
  const robot: ChangeOutput['actor'] = { kind: 'robot' }
  // @ts-expect-error a status category outside the six.
  const bad: KindDeclInput['statuses'] = [{ name: 'x', category: 'soon' }]
  expect([change.via, bare, robot, bad]).toHaveLength(4)
})
