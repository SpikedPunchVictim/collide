import { expect, test } from 'vitest'
import type { SampleInput, SampleOutput, StepInput } from './fixtures/generated/index.ts'

// These checks run in `pnpm typecheck`: vitest strips the types, so the `@ts-expect-error` lines are the assertions.

test('generated types accept the shapes the schema allows', () => {
  const wait: StepInput = { kind: 'wait' }
  const agent: StepInput = { kind: 'agent', prompt: 'go' }
  const sample: SampleInput = {
    name: 'demo',
    retries: 3,
    budgetBytes: 1_000_000,
    steps: [agent, wait],
    level1: { level2: { level3: { value: 'deep' } } },
  }
  expect(sample.steps).toHaveLength(2)
})

test('generated types reject unknown keys in object literals', () => {
  // @ts-expect-error Wait {} has no fields besides kind.
  const wait: StepInput = { kind: 'wait', extra: 1 }
  const sample: SampleInput = {
    name: 'demo',
    retries: 3,
    budgetBytes: 1,
    steps: [],
    // @ts-expect-error unknown key at depth three.
    level1: { level2: { level3: { value: 'x', extra: 1 } } },
  }
  expect([wait, sample]).toHaveLength(2)
})

test('an output type is complete where the input type has defaults', () => {
  // @ts-expect-error labels is required on output.
  const output: SampleOutput = {
    name: 'demo',
    retries: 3,
    budgetBytes: 1,
    steps: [],
    level1: { level2: { level3: { value: 'x' } } },
  }
  expect(output.name).toBe('demo')
})
