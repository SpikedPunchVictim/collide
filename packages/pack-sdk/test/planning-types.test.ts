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
    apiVersion: 'collide/v0',
    name: 'ferry',
    points: { 'spike.before_conclude': { on: 'task.before_status', match: ['done'] } },
    checklists: { spike: { version: 1, gates: 'ready', items: [{ id: 'design', text: 'Designed' }] } },
  }
  expect([kind.name, Object.keys(manifest.points ?? {})]).toEqual(['spike', ['spike.before_conclude']])
})

test('a manifest needs apiVersion and name, and holds hooks, a binding, a role and an observer', () => {
  const manifest: PackManifestInput = {
    apiVersion: 'collide/v0',
    name: 'ferry',
    workspaces: { worktree: { create: { script: 'hooks/ws-create.ts', timeout: '120m', outputs: { mapCommit: {} } } } },
    gates: { evidence: { script: 'gates/evidence.ts' } },
    bindings: [{ point: 'task.before_status', match: ['done'], gate: 'evidence', timeout: '30s' }],
    roles: { builder: { agent: 'claude', definition: 'repo:.claude/agents/builder.md', readOnly: true } },
    observers: { notify: { script: 'observers/notify.ts', events: ['run.*'] } },
    protectedPaths: { paths: ['.agents/**'], at: 'phase-end' },
  }
  // @ts-expect-error apiVersion and name are required.
  const unnamed: PackManifestInput = { points: {} }
  // @ts-expect-error a key the manifest does not have.
  const typo: PackManifestInput = { apiVersion: 'collide/v0', name: 'ferry', viewz: {} }
  // @ts-expect-error `at` has one value.
  const at: PackManifestInput['protectedPaths'] = { paths: [], at: 'commit' }
  expect([manifest.name, unnamed, typo, at]).toHaveLength(4)
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
