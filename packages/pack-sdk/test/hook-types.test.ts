import { expect, test } from 'vitest'
import type { HookInputOutput, HookResultInput, ReportLineOutput } from '../src/index.ts'

// These checks run in `pnpm typecheck`: vitest strips the types, so the `@ts-expect-error` lines are the assertions.

test('a result needs only a status, and a pending result may carry retryAfter', () => {
  const pending: HookResultInput = { status: 'pending', retryAfter: 30 }
  // @ts-expect-error a status outside the six.
  const maybe: HookResultInput = { status: 'maybe' }
  // @ts-expect-error `retryAfter` is spelled in camel case.
  const snake: HookResultInput = { status: 'pending', retry_after: 30 }
  expect([pending.status, maybe, snake]).toHaveLength(3)
})

test('a hook input outside a task point has no change, and a task entry has no body', () => {
  const task: HookInputOutput['tasks'][number] = {
    key: 'plan:0027',
    shortId: 'ts-0027',
    kind: 'task',
    state: { status: 'active', category: 'active' },
    path: 'a.md',
    templateFields: {},
    fields: {},
    links: [
      {
        type: 'blocked-by',
        dir: 'out',
        key: 'plan:0009',
        kind: 'task',
        status: 'done',
        category: 'done',
        stale: false,
        missing: false,
      },
    ],
  }
  // @ts-expect-error a task entry never carries its body.
  const withBody: HookInputOutput['tasks'][number] = { ...task, body: 'text' }
  const change: HookInputOutput['change'] = undefined
  expect([task.key, withBody, change]).toHaveLength(3)
})

test('a report line is one of six kinds and always has v and t', () => {
  const metric: ReportLineOutput = { v: 1, t: '2026-11-13T09:25:03.010Z', kind: 'metric', name: 'tests.passed', value: 412 }
  // @ts-expect-error every line carries its timestamp.
  const noTime: ReportLineOutput = { v: 1, kind: 'progress', done: 1 }
  // @ts-expect-error a merged `cached` count is not a usage key.
  const merged: ReportLineOutput = { v: 1, t: 'x', kind: 'usage', model: 'sonnet', cached: 5 }
  expect([metric.kind, noTime, merged]).toHaveLength(3)
})
