// A scripted world for the Planning tab (ts-0195-ju): the plan source with two epics, three proposals (one only on a
// branch), the tasks that implement their features, and the sections and near-duplicates the daemon would report.
// The sections are written by hand and agree with the features below; the bridge spec (`browser/planning.spec.ts`)
// checks the real ones against `collide sections --json`.
import type { TaskOutput } from '../../src/owner/generated.ts'
import { FieldWorld, type LinkType, PLAN, task } from './field-world.ts'

type Over = Partial<TaskOutput>

const EPIC_ONE = 'plan:ep-001-aa'
const EPIC_TWO = 'plan:ep-002-bb'

const feature = (id: string, section: string, title: string) => ({ id, section, title, summary: '' })

/** The plan pack's link types, with the name each gives the other end. */
export const PLAN_LINK_TYPES: LinkType[] = [
  { name: 'blocked-by', dir: 'out', blocking: true, inverse: 'blocks' },
  { name: 'part-of', dir: 'out', blocking: false, inverse: 'has part' },
  { name: 'implements', dir: 'out', blocking: false, inverse: 'implemented by', target: 'feature' },
  { name: 'includes', dir: 'out', blocking: false, inverse: 'included in' },
  { name: 'informs', dir: 'out', blocking: false, inverse: 'informed by' },
]

type Category = TaskOutput['state']['category']

/** A proposal's `draft` is active, its `accepted` done, and a spike's `planned` ready; the rest are categories. */
const CATEGORY_OF: Record<string, Category> = { draft: 'active', accepted: 'done', planned: 'ready' }

function item(key: string, title: string, status: string, over: Over = {}) {
  return task(key, title, status, CATEGORY_OF[status] ?? (status as Category), over)
}

/**
 * Board UI first: two features in Field, one in Drawer and one in Settings, on a proposal that is a draft. GitHub
 * source: an accepted proposal with a feature in Sources and one in `Setting`, which is near `Settings`. Loose ideas:
 * a draft only on a branch, with one feature that is unsorted. Tasks: `ts-0004-dd` is the next ready one of the first
 * epic, since `ts-0002-bb` waits on one that is under way.
 */
export function planningWorld(): FieldWorld {
  const world = new FieldWorld()
  world.sources = [PLAN]
  world.collections = [
    { id: EPIC_ONE, title: 'Board UI first', groups: [], path: 'epics/ep-001-aa/epic.md' },
    { id: EPIC_TWO, title: 'GitHub source', groups: [], path: 'epics/ep-002-bb/epic.md' },
  ]
  const one = { collections: [EPIC_ONE] }
  const two = { collections: [EPIC_TWO] }
  world.listing = [
    item('plan:ep-001-aa', 'Board UI first', 'active', { kind: 'epic' }),
    item('plan:ep-002-bb', 'GitHub source', 'active', { kind: 'epic' }),
    item('plan:pr-0001-aa', 'Board UI before GitHub', 'draft', {
      ...one,
      kind: 'proposal',
      links: [
        { type: 'part-of', to: 'ep-001-aa' },
        { type: 'includes', to: 'ts-0004-dd' },
      ],
      features: [
        feature('F1', 'Field', 'Columns from each source'),
        feature('F2', 'Field', 'Saved views'),
        feature('F3', 'Drawer', 'Each copy'),
        feature('F4', 'Settings', 'Settings page'),
      ],
      coverage: { featuresWithoutTask: ['F2'], tasksWithoutFeature: ['ts-0004-dd'] },
      decisions: [{ id: 'D1', title: 'Order', text: '', passedOn: ['Ship together'], evidence: [], line: 1 }],
      assumptions: [
        { id: 'A1', title: 'Small', text: '', state: 'unverified', evidence: [], line: 1 },
        { id: 'A2', title: 'Fast', text: '', state: 'verified', evidence: [], line: 2 },
      ],
    }),
    item('plan:pr-0002-bb', 'GitHub source', 'accepted', {
      ...two,
      kind: 'proposal',
      links: [{ type: 'part-of', to: 'ep-002-bb' }],
      features: [feature('F1', 'Sources', 'Issues as tasks'), feature('F2', 'Setting', 'Settings view')],
      coverage: { featuresWithoutTask: ['F2'], tasksWithoutFeature: [] },
    }),
    item('plan:pr-0003-cc', 'Loose ideas', 'draft', {
      kind: 'proposal',
      features: [feature('F1', '', 'Order by hand')],
      featureProblems: [{ line: 9, reason: 'a feature has no title' }],
    }),
    item('plan:ts-0001-aa', 'Draw the columns', 'done', {
      ...one,
      kind: 'task',
      links: [{ type: 'implements', to: 'pr-0001-aa#F1' }],
    }),
    item('plan:ts-0002-bb', 'Save a view', 'ready', {
      ...one,
      kind: 'task',
      links: [
        { type: 'implements', to: 'pr-0001-aa#F3' },
        { type: 'blocked-by', to: 'ts-0003-cc' },
      ],
    }),
    item('plan:ts-0003-cc', 'Build the drawer', 'active', {
      ...one,
      kind: 'task',
      links: [{ type: 'implements', to: 'pr-0001-aa#F3' }],
      checklist: {
        template: 'ready-to-build@1',
        items: [
          { id: 'tests', state: 'done', by: 'ana', date: '2026-11-20', evidence: 'ts-0001-aa' },
          { id: 'review', state: 'suggested', reason: 'the diff touches the wire' },
          { id: 'docs', state: 'n/a', reason: 'no user text' },
          { id: 'owner', state: 'open' },
        ],
      },
    }),
    item('plan:sp-0001-aa', 'Does the strip scale', 'planned', {
      kind: 'spike',
      links: [{ type: 'informs', to: 'pr-0001-aa' }],
      claims: [
        { id: 'C1', label: 'measured', loadBearing: true, verifiable: true },
        { id: 'C2', label: 'inferred', loadBearing: false },
      ],
    }),
    item('plan:ts-0004-dd', 'Show each copy', 'ready', { ...one, kind: 'task' }),
    item('plan:ts-0005-ee', 'Read issues', 'done', {
      ...two,
      kind: 'task',
      links: [{ type: 'implements', to: 'pr-0002-bb#F1' }],
    }),
    item('plan:ts-0007-gg', 'Read more issues', 'ready', {
      ...two,
      kind: 'task',
      links: [{ type: 'implements', to: 'pr-0002-bb#F1' }],
    }),
    item('plan:ts-0006-ff', 'Dropped work', 'dropped', {
      ...two,
      kind: 'task',
      links: [{ type: 'implements', to: 'pr-0002-bb#F1' }],
    }),
  ]
  world.linkTypes.set('plan', PLAN_LINK_TYPES)
  world.branchOnly = [{ key: 'plan:pr-0003-cc', checkout: '/work/alpha-plan', branch: 'plan/ui-first' }]
  world.sections = {
    sections: [
      { name: 'Field', featureCount: 2, proposalCount: 1, proposals: ['pr-0001-aa'] },
      { name: 'Drawer', featureCount: 1, proposalCount: 1, proposals: ['pr-0001-aa'] },
      { name: 'Setting', featureCount: 1, proposalCount: 1, proposals: ['pr-0002-bb'] },
      { name: 'Settings', featureCount: 1, proposalCount: 1, proposals: ['pr-0001-aa'] },
      { name: 'Sources', featureCount: 1, proposalCount: 1, proposals: ['pr-0002-bb'] },
      { name: '', featureCount: 1, proposalCount: 1, proposals: ['pr-0003-cc'] },
    ],
    nearDuplicates: [{ names: ['Setting', 'Settings'] }],
  }
  return world
}
