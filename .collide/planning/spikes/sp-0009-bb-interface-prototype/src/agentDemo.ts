// Demo data for the in-app agent: a dozen ready tasks that describe behaviour changes in the invented Larder repo
// (fixtures/repo/), and a few finished runs with different build times. Used by the page and by tests; the
// scenarios do not use it. All names are invented. Task texts never name a file path: the agent has to look.
import { FULL_WORKFLOW, SHORT_WORKFLOW, type PhaseSpec } from './gen'
import { addRun, initialState } from './world'
import { advanceAll } from './sim'
import type { Epic, State, Task } from './types'

export const PANTRY_EPIC: Epic = { id: 'E-09', title: 'Larder: kitchen stock tracker' }

const t = (n: number, title: string, body: string, labels: string[]): Task => ({
  id: `T-09-${String(n).padStart(3, '0')}`, title, epic: 'E-09', status: 'ready', labels, source: 'tasks/', dependsOn: [], body,
})

export const PANTRY_TASKS: Task[] = [
  t(1, 'Accept fractions in quantities',
    'Adding "1/2 cup flour" or "1 1/2 kg rice" fails with a quantity error today. Fractions and mixed numbers should be read as the matching decimal amount.', ['api']),
  t(2, 'Treat unit spellings as one unit',
    '"tbsp", "tablespoon" and "tablespoons" count as three different units, so one ingredient shows up several times in lists and reports. Spelling variants and plurals should count as one unit.', ['api']),
  t(3, 'Say what was wrong with a quantity',
    'When a quantity cannot be read, the tool prints "Invalid input". It should quote the text it could not read and give one example of an accepted form, for instance "2 cups".', ['ui']),
  t(4, 'Warn earlier about expiry',
    'Items are flagged as expiring only one day ahead. Make it three days, and let the user set the number of days.', ['cache']),
  t(5, 'Quiet hours for reminders',
    'Reminders go out at any hour. A reminder that would fall between 22:00 and 08:00 should be held until 08:00.', ['infra']),
  t(6, 'No reminders for items without an expiry date',
    'An item with no expiry date makes the whole reminder run stop with an error. Such items should be skipped and never produce a reminder.', ['api']),
  t(7, 'Show days left in the list',
    'The list command should show how many days are left before each item expires, for example "3 days", "today" or "expired".', ['ui']),
  t(8, 'Low-stock report shows the shelf',
    'The low-stock report lists the item and the amount left but not where it is kept. Add the shelf name to each line.', ['docs']),
  t(9, 'Quote commas in the CSV export',
    'An item name with a comma splits into two columns in the exported spreadsheet. Such values should be quoted, and quote marks inside them doubled.', ['api']),
  t(10, 'Round report totals',
    'Totals in reports sometimes show 0.30000000000000004. Show amounts rounded to two decimals everywhere in reports.', ['ui']),
  t(11, 'Keep a backup before upgrading the data file',
    'When the stored format changes between versions, the old file is overwritten in place. Keep a copy of the previous file next to it before the upgrade runs.', ['infra']),
  t(12, 'Do not corrupt the data file on a crash',
    'If the process stops while saving, the data file can be left half written. Saving should either finish completely or leave the previous file intact.', ['infra']),
]

/**
 * The grouping the fixture was designed to have. `ambiguous` tasks touch two areas on purpose; placing either one in
 * either of its areas, or leaving it out, is acceptable.
 */
export const PANTRY_GROUPS: Record<string, string[]> = {
  quantities: ['T-09-001', 'T-09-002'],
  expiry: ['T-09-004', 'T-09-005', 'T-09-006'],
  reports: ['T-09-008', 'T-09-009', 'T-09-010'],
  datafile: ['T-09-011', 'T-09-012'],
}
export const PANTRY_AMBIGUOUS: Record<string, string[]> = {
  'T-09-003': ['quantities', 'messages'],
  'T-09-007': ['expiry', 'list command'],
}

const withBuild = (base: PhaseSpec[], build: number): PhaseSpec[] => base.map((p) => (p.key === 'build' ? { ...p, dur: build } : p))

/** Finished runs with distinct build times, so "longest in build" has one right order. */
export const DEMO_RUNS: { id: string; task: string; build: number; quick?: boolean; endedAgo: number }[] = [
  { id: 'R-101', task: 'T-02-006', build: 420, endedAgo: 3000 },
  { id: 'R-102', task: 'T-02-009', build: 1380, endedAgo: 2800 },
  { id: 'R-103', task: 'T-01-005', build: 640, endedAgo: 2300 },
  { id: 'R-104', task: 'T-01-008', build: 960, endedAgo: 1900 },
  { id: 'R-105', task: 'T-06-005', build: 280, quick: true, endedAgo: 1500 },
  { id: 'R-106', task: 'T-06-009', build: 1180, endedAgo: 1100 },
  { id: 'R-107', task: 'T-05-004', build: 780, endedAgo: 700 },
  { id: 'R-108', task: 'T-08-003', build: 1500, endedAgo: 400 },
  { id: 'R-109', task: 'T-08-006', build: 340, quick: true, endedAgo: 150 },
]
export const BUILD_ORDER_DESC = [...DEMO_RUNS].sort((a, b) => b.build - a.build).map((r) => r.id)

export function applyAgentDemo(s: State): State {
  s.epics.push(PANTRY_EPIC)
  s.tasks.push(...PANTRY_TASKS.map((x) => ({ ...x, body: x.body })))
  for (const [i, d] of DEMO_RUNS.entries()) {
    const task = s.tasks.find((x) => x.id === d.task)
    const phases = withBuild(d.quick ? SHORT_WORKFLOW : FULL_WORKFLOW, d.build)
    const run = addRun(s, {
      id: d.id, taskId: d.task, title: task?.title ?? d.task, startWall: 0, review: 'accepted',
      workflow: d.quick ? 'quick-fix' : 'build-review', gen: { seed: 500 + i, phases },
    })
    const total = run.script[run.script.length - 1].at
    run.startWall = s.simNow - d.endedAgo - total
  }
  // one run still going, so Tracks has a live row
  const live = addRun(s, { id: 'R-110', taskId: 'T-03-004', title: 'Import log loses the first line', startWall: s.simNow - 500, gen: { seed: 510 } })
  live.review = 'pending'
  advanceAll(s, s.simNow)
  s.counter = 110
  return s
}

export function agentDemoState(facilitator = false): State {
  return applyAgentDemo(initialState('orchard', facilitator))
}
