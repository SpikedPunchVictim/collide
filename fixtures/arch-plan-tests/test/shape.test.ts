import { describe, expect, test } from 'vitest'
import {
  diffRuns,
  diffShapes,
  envelopeShape,
  mergeShapes,
  runShape,
  shapeOf,
} from '../../orchard-arch/contract/shape.ts'
import type { RunShape } from '../../orchard-arch/contract/shape.ts'

// The shape reader (fixtures/orchard-arch/contract/shape.ts) turns one run of a plan CLI into a shape that is kept in
// a public repository. These tests pin what it keeps and, above all, what it does not.

describe('a leaf', () => {
  test.each([
    ['null', null, 'null'],
    ['a boolean', true, 'boolean'],
    ['a number', 3.5, 'number'],
    ['free text', 'Rename the quay', 'string'],
    ['an empty string', '', 'string'],
    ['a path', 'proposals/03-harbor-index/tasks/T-03-001-survey-the-quay.md', 'string'],
  ])('%s', (_name, value, shape) => {
    expect(shapeOf(value)).toBe(shape)
  })

  test.each([
    ['a task id with its slug', 'T-03-001-survey-the-quay', 'task-id'],
    ['a short task id', 'T-03-001', 'task-short-id'],
    ['a proposal id', 'P03', 'proposal-id'],
    ['an epic id', 'E-07', 'epic-id'],
    ['an epic id with its slug', 'E-07-harbor-plan', 'epic-id'],
    ['a question id', 'Q-03-01-which-berth-first', 'id'],
    ['a short question id', 'Q-03-01', 'short-id'],
  ])('%s is named by its id pattern', (_name, value, shape) => {
    expect(shapeOf(value)).toBe(shape)
  })

  test('a status the pack declares is kept as itself', () => {
    expect(shapeOf('in_review')).toBe('=in_review')
  })

  test('a word that is no status is not kept', () => {
    expect(shapeOf('started')).toBe('string')
  })
})

describe('the envelope', () => {
  test('keeps its command and its schema version', () => {
    const shape = envelopeShape({ schema_version: 1, command: 'new-task', data: {} }) as Record<string, unknown>
    expect(shape.command).toBe('=new-task')
    expect(shape.schema_version).toBe('=1')
  })

  test('keeps no command that is more than one lowercase word with hyphens', () => {
    const shape = envelopeShape({ command: 'Rename /home/x to y' }) as Record<string, unknown>
    expect(shape.command).toBe('string')
  })

  test('keeps a command only at its top level', () => {
    const shape = envelopeShape({ command: 'set', data: { command: 'set' } }) as { data: Record<string, unknown> }
    expect(shape.data.command).toBe('string')
  })
})

describe('a list', () => {
  test('is empty when it has no element', () => {
    expect(shapeOf([])).toEqual([])
  })

  test('holds its elements merged into one shape', () => {
    const shape = shapeOf([{ id: 'T-03-001', note: 'a' }, { id: 'T-03-002' }])
    expect(shape).toEqual([{ id: 'task-short-id', 'note?': 'string' }])
  })

  test('holds the alternatives of a leaf, sorted', () => {
    expect(shapeOf(['done', 'doing', 'done', 'text'])).toEqual(['=doing|=done|string'])
  })

  test('holds a key that every element has without a mark', () => {
    expect(shapeOf([{ a: 1 }, { a: 2 }])).toEqual([{ a: 'number' }])
  })

  test('becomes mixed where a leaf meets an object', () => {
    expect(shapeOf([null, { a: 1 }])).toEqual(['mixed'])
  })

  test('keeps a mark of a key that an element lacks, once it is merged again', () => {
    const first = shapeOf([{ a: 1 }, {}])
    expect(mergeShapes(first, shapeOf([{ a: 1 }, { a: 2 }]))).toEqual([{ 'a?': 'number' }])
  })
})

describe('an object keyed by data', () => {
  test('is a map: its keys are named by what they are and its values are merged', () => {
    const shape = shapeOf({
      'T-03-001-survey-the-quay': { status: 'done' },
      'T-03-002': { status: 'ready', note: 'x' },
      'harbor-index': { status: 'doing' },
      'quay-index': { status: 'doing', note: 'y' },
    })
    expect(shape).toEqual({
      '<task-id>': { status: '=done' },
      '<task-short-id>': { note: 'string', status: '=ready' },
      '<string>': { 'note?': 'string', status: '=doing' },
    })
  })

  test('keeps a status as a key of a map', () => {
    expect(shapeOf({ ready: 2, 'harbor-index': 1 })).toEqual({ ready: 'number', '<string>': 'number' })
  })

  test('is an object with its keys when they are all of the schema', () => {
    expect(shapeOf({ blockedTasks: [], schema_version: 1, depends_on: [] })).toEqual({
      blockedTasks: [],
      depends_on: [],
      schema_version: 'number',
    })
  })
})

describe('what the reader keeps from a run', () => {
  const FORBIDDEN = [
    'Users',
    'jane',
    'harbor',
    'quay',
    'ledger',
    'secret',
    'feature/',
    'example',
    '2026',
    'a3f9c0e1d2b4',
    'T-03-001',
    '.md',
    'proposals',
  ]

  const envelope = {
    schema_version: 1,
    command: 'status',
    corpus: '/Users/jane/work/orchard/arch',
    data: {
      path: '/Users/jane/work/orchard/arch/proposals/03-harbor-index/tasks/T-03-001-secret-slug.md',
      title: 'Rename the secret ledger',
      branch: 'feature/jane/secret-ledger',
      date: '2026-03-04',
      sha: 'a3f9c0e1d2b4a5968778695a4b3c2d1e0f9a8b7c',
      author: 'jane@example.invalid',
      files: ['proposals/03-harbor-index/tasks/T-03-001-secret-slug.md', 'work/epics/E-07-harbor-plan.md'],
      byTask: {
        'T-03-001-secret-slug': { title: 'Secret title', when: '2026-03-04' },
        'secret-slug': { title: 'Another secret title' },
        '2026-03-04': { title: 'A third' },
        'proposals/03-harbor-index': { title: 'A fourth' },
      },
      rows: [{ note: 'a secret note', command: 'secret command with spaces' }],
    },
  }

  test('is no title, path, slug, branch, date, sha or name', () => {
    const kept = JSON.stringify(runShape({ code: 0, stdout: JSON.stringify(envelope), stderr: '' }))
    for (const word of FORBIDDEN) expect(kept, word).not.toContain(word)
  })

  test('is, of the ids and the keys that were data, their names', () => {
    const shape = envelopeShape(envelope) as { data: { byTask: Record<string, unknown> } }
    expect(Object.keys(shape.data.byTask).sort()).toEqual(['<string>', '<task-id>'])
  })

  test('is no text of the standard error stream, only its line count', () => {
    const run = runShape({ code: 2, stdout: '', stderr: 'jane: /Users/jane/x is not a step\nsecond line\n' })
    expect(JSON.stringify(run)).not.toContain('jane')
    expect(run.stderr_lines).toBe(2)
  })
})

describe('the shape of a run', () => {
  test('says how it ended and which streams it used', () => {
    expect(runShape({ code: 2, stdout: '', stderr: 'one line\n' })).toEqual({
      exit: 2,
      stdout_empty: true,
      stdout_json: false,
      stderr_empty: false,
      stderr_lines: 1,
      envelope: null,
    })
  })

  test('counts the lines of the standard error stream, a blank one too, and not the last newline', () => {
    expect(runShape({ code: 0, stdout: '{}', stderr: '\nwrote: x\n' }).stderr_lines).toBe(2)
  })

  test('has no envelope for output that is not JSON', () => {
    const run = runShape({ code: 0, stdout: 'plain words\n', stderr: '' })
    expect(run.stdout_json).toBe(false)
    expect(run.envelope).toBeNull()
  })

  test('has no exit code for a run that a signal ended', () => {
    expect(runShape({ code: null, stdout: '', stderr: '' }).exit).toBeNull()
  })
})

describe('the difference from a recorded shape', () => {
  const recorded = shapeOf({ data: { id: 'T-03-001-x', written: true, files: ['a'], note: 'x' } })

  test('is none for the same shape', () => {
    const actual = shapeOf({ data: { id: 'T-03-002-y', written: false, files: ['b'], note: 'y' } })
    expect(diffShapes(recorded, actual)).toEqual([])
  })

  test('names the path of a leaf whose type differs', () => {
    const actual = shapeOf({ data: { id: { name: 'T-03-001-x' }, written: true, files: ['a'], note: 'x' } })
    expect(diffShapes(recorded, actual)).toEqual(['envelope.data.id: recorded task-id, got object'])
  })

  test('names the path of a leaf with another type', () => {
    const actual = shapeOf({ data: { id: 'T-03-001-x', written: 'yes', files: ['a'], note: 'x' } })
    expect(diffShapes(recorded, actual)).toEqual(['envelope.data.written: recorded boolean, got string'])
  })

  test('names a key that the recording has and the run lacks', () => {
    const actual = shapeOf({ data: { id: 'T-03-001-x', written: true, files: ['a'] } })
    expect(diffShapes(recorded, actual)).toEqual(['envelope.data.note: recorded, missing'])
  })

  test('names a key that the recording does not have', () => {
    const actual = shapeOf({ data: { id: 'T-03-001-x', written: true, files: ['a'], note: 'x', extra: 1 } })
    expect(diffShapes(recorded, actual)).toEqual(['envelope.data.extra: not recorded'])
  })

  test('names the path inside a list', () => {
    const actual = shapeOf({ data: { id: 'T-03-001-x', written: true, files: [3], note: 'x' } })
    expect(diffShapes(recorded, actual)).toEqual(['envelope.data.files[]: recorded string, got number'])
  })

  test('lets a key that the recording marks as sometimes absent be absent', () => {
    const wanted = shapeOf([{ a: 1, b: 2 }, { a: 1 }])
    expect(diffShapes(wanted, shapeOf([{ a: 5 }]))).toEqual([])
  })

  test('refuses a key that the recording has in every element and the run lacks in one', () => {
    const wanted = shapeOf([{ a: 1 }])
    expect(diffShapes(wanted, shapeOf([{ a: 1 }, {}]))).toEqual(['envelope[].a: recorded in all, missing in some'])
  })

  test('lets an empty list or an empty object match any', () => {
    const wanted = shapeOf({ queue: [{ a: 1 }], velocity: {} })
    expect(diffShapes(wanted, shapeOf({ queue: [], velocity: { w: 1 } }))).toEqual([])
  })

  test('takes a leaf with fewer alternatives than the recording, and refuses one with more', () => {
    expect(diffShapes('=doing|=done', '=done')).toEqual([])
    expect(diffShapes('=done', '=doing|=done')).toEqual(['envelope: recorded =done, got =doing|=done'])
  })
})

describe('the difference between two runs', () => {
  const base: RunShape = {
    exit: 0,
    stdout_empty: false,
    stdout_json: true,
    stderr_empty: true,
    stderr_lines: 0,
    envelope: { a: 'number' },
  }

  test.each([
    ['the exit code', { exit: 1 }, 'exit: recorded 0, got 1'],
    ['the line count of standard error', { stderr_lines: 2 }, 'stderr_lines: recorded 0, got 2'],
    ['whether standard output is empty', { stdout_empty: true }, 'stdout_empty: recorded false, got true'],
  ])('names %s', (_name, change, message) => {
    expect(diffRuns(base, { ...base, ...change })).toContain(message)
  })

  test('names a path in the envelope', () => {
    expect(diffRuns(base, { ...base, envelope: { a: 'string' } })).toEqual(['envelope.a: recorded number, got string'])
  })
})
