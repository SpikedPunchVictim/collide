import { expect, test } from 'vitest'
import type {
  ApplyResultOutput,
  InitializeResultInput,
  ObserverAckOutput,
  ObserverEventOutput,
  PreviewResultOutput,
  TaskInvalidOutput,
} from '../src/index.ts'

// These checks run in `pnpm typecheck`: vitest strips the types, so the `@ts-expect-error` lines are the assertions.

test('a provider with one kind and no links needs neither `kinds` nor `links`', () => {
  const result: InitializeResultInput = {
    scope: 'ref',
    idPattern: '^a$',
    shortIdPattern: '^b$',
    statuses: [{ name: 'ready', category: 'ready' }],
    capabilities: { watch: true },
  }
  // @ts-expect-error `scope` is required.
  const noScope: InitializeResultInput = { idPattern: '^a$', shortIdPattern: '^b$', statuses: [], capabilities: {} }
  expect([result.scope, noScope]).toHaveLength(2)
})

test('an apply result is ok, conflict or refused, each with its own payload', () => {
  const refused: ApplyResultOutput = { outcome: 'refused', message: 'illegal transition' }
  // @ts-expect-error a refusal carries a message.
  const noMessage: ApplyResultOutput = { outcome: 'refused' }
  // @ts-expect-error an outcome outside the three.
  const later: ApplyResultOutput = { outcome: 'later', message: 'm' }
  expect([refused.outcome, noMessage, later]).toHaveLength(3)
})

test('task.invalid and the observer lines carry their keys', () => {
  const invalid: TaskInvalidOutput = { path: 'a.md', checkout: null, reason: 'no title' }
  const event: ObserverEventOutput = { seq: 1, event: 'run.started', payload: { run: 'r-1' } }
  const ack: ObserverAckOutput = { seq: 1 }
  // @ts-expect-error an acknowledgement names a sequence number.
  const bare: ObserverAckOutput = {}
  expect([invalid.reason, event.seq, ack.seq, bare]).toHaveLength(4)
})

test('a preview answers ok, conflict or refused, like apply', () => {
  const ok: PreviewResultOutput = { outcome: 'ok', items: [], files: [] }
  // @ts-expect-error `ok` carries items and files.
  const bare: PreviewResultOutput = { outcome: 'ok' }
  expect([ok.outcome, bare]).toHaveLength(2)
})
