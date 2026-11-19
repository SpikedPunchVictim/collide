import { describe, expect, test } from 'vitest'
import { readAssumptions } from '../src/assumptions.ts'
import type { ItemLookup } from '../src/decisions.ts'

/** The items of a made-up scan: a task, a concluded and a planned spike, and a proposal with two features. */
const KNOWN: Record<string, { status: string; features: string[]; kind: string }> = {
  'ts-0001-aa': { status: 'ready', features: [], kind: 'task' },
  'sp-0001-aa': { status: 'concluded', features: [], kind: 'spike' },
  'sp-0002-bb': { status: 'planned', features: [], kind: 'spike' },
  'pr-0003-km': { status: 'draft', features: ['F1', 'F2'], kind: 'proposal' },
}
const lookup: ItemLookup = (id) => KNOWN[id]

const read = (body: string, firstLine = 1) => readAssumptions(body, firstLine, lookup)

/** A body of these lines, each ending in a line break. */
const text = (...lines: string[]): string => `${lines.join('\n')}\n`
/** An assumption, `A1`, with the lines under it. It has a `State:`, so it has no problem of its own. */
const entry = (...under: string[]): string => text('## Assumptions', '- **A1 T**: x', '  State: verified', ...under)
/** A problem of the ledger, on `line`, whose reason matches `reason`. */
const problem = (line: number, reason: RegExp) => ({ part: 'Assumptions', line, reason: expect.stringMatching(reason) })

const FULL = [
  '## Assumptions',
  '',
  '- **A1 Files are enough**: The plan leans on plain files.',
  '  The text goes on.',
  '  State: verified',
  '  Evidence: sp-0001-aa, pr-0003-km#F2, core/src/item.rs',
  '  Spike: sp-0002-bb',
  '',
].join('\n')

describe('an entry of the Assumption ledger', () => {
  test('carries its title, its text and each label, with its line, and a spike with its status', () => {
    const { assumptions, problems } = read(FULL)
    expect(problems).toEqual([])
    expect(assumptions).toEqual([
      {
        id: 'A1',
        title: 'Files are enough',
        text: 'The plan leans on plain files. The text goes on.',
        state: 'verified',
        evidence: [
          { target: 'sp-0001-aa', kind: 'item', status: 'concluded' },
          { target: 'pr-0003-km#F2', kind: 'feature', proposal: 'pr-0003-km' },
          { target: 'core/src/item.rs', kind: 'path' },
        ],
        spike: 'sp-0002-bb',
        spikeStatus: 'planned',
        line: 3,
      },
    ])
  })

  test('each of the four states reads as written, with no problem', () => {
    for (const state of ['unverified', 'verified', 'contradicted', 'accepted risk']) {
      const { assumptions, problems } = read(`## Assumptions\n- **A1 T**: x\n  State: ${state}\n`)
      expect(problems).toEqual([])
      expect(assumptions[0]!.state).toBe(state)
    }
  })

  test('has no optional key it did not write', () => {
    const { assumptions, problems } = read('## Assumptions\n- **A1 T**:\n  State: unverified\n')
    expect(problems).toEqual([])
    expect(assumptions).toEqual([{ id: 'A1', title: 'T', text: '', state: 'unverified', evidence: [], line: 2 }])
  })

  test('a label may continue on the lines below it, and a line of the file is a line of the file', () => {
    const body = '## Assumptions\n- **A1 T**: x\n  Spike: sp-0001-aa\n  State: accepted\n    risk\n'
    const { assumptions, problems } = read(body, 10)
    expect(problems).toEqual([])
    expect(assumptions[0]).toMatchObject({ spike: 'sp-0001-aa', state: 'accepted risk', line: 11 })
  })

  test('the heading is matched in any case after trimming, and the entries of every Assumptions part count', () => {
    const body = text(
      '##   ASSUMPTIONS  ',
      '- **A1 A**: x',
      '',
      '## Context',
      '',
      '## assumptions',
      '- **A2 B**: x',
    )
    expect(read(body).assumptions.map((assumption) => assumption.id)).toEqual(['A1', 'A2'])
    expect(read('## Assumption\n- **A1 A**: x\n').assumptions).toEqual([])
  })

  test('CRLF reads as LF does, with no carriage return in any value', () => {
    const lf = read(FULL)
    const crlf = read(FULL.replaceAll('\n', '\r\n'))
    expect(crlf).toEqual(lf)
    expect(JSON.stringify(crlf)).not.toContain('\\r')
  })
})

describe('what is not the Assumption ledger', () => {
  test('plain text in the section is read as nothing, with no problem', () => {
    const body = text('## Assumptions', '', 'We assume files.', '', '- **Assumed**: files', '  and more.', '', '- one')
    expect(read(body)).toEqual({ assumptions: [], problems: [] })
  })

  test('a body with no Assumptions section has none', () => {
    expect(read('## Context\n\nText.\n')).toEqual({ assumptions: [], problems: [] })
  })

  test('an entry under another heading is not read, and the part ends at the next ## heading', () => {
    const body = '## Context\n- **A1 A**: x\n\n## Assumptions\n- **A2 B**: x\n\n## Notes\n- **A3 C**: x\n'
    expect(read(body).assumptions.map((assumption) => assumption.id)).toEqual(['A2'])
    expect(read(body).problems).toEqual([])
  })

  test('a fenced block is not read, and an Assumptions heading inside one is not a part', () => {
    const body = text(
      '## Assumptions',
      '```',
      '- **A1 A**: x',
      '```',
      '- **A2 B**: x',
      '',
      '## Context',
      '~~~',
      '## Assumptions',
      '- **A3 C**: x',
      '~~~',
    )
    expect(read(body).assumptions.map((assumption) => assumption.id)).toEqual(['A2'])
    expect(read(body).problems).toEqual([])
  })
})

describe('the State of an assumption', () => {
  test('a missing State reads as unverified, and is not a problem', () => {
    const { assumptions, problems } = read('## Assumptions\n- **A1 T**: x\n  Evidence: ts-0001-aa\n')
    expect(assumptions[0]!.state).toBe('unverified')
    expect(problems).toEqual([])
  })

  test('a State that is none of the four is a problem on its line, and reads as unverified', () => {
    for (const value of ['maybe', 'Verified', 'accepted-risk', 'verified, mostly', '']) {
      const { assumptions, problems } = read(`## Assumptions\n- **A1 T**: x\n  State: ${value}\n`)
      expect(assumptions[0]!.state, value).toBe('unverified')
      expect(problems, value).toEqual([
        problem(3, /^A1 .*unverified, verified, contradicted/),
      ])
    }
  })

  test('a State given twice is a problem, and the first stays', () => {
    const { assumptions, problems } = read(entry('  State: contradicted'))
    expect(assumptions[0]!.state).toBe('verified')
    expect(problems).toEqual([problem(4, /^A1 .*State:.*more than once/)])
  })
})

describe('the Spike of an assumption', () => {
  const body = (spike: string): string => entry(`  Spike: ${spike}`)

  test('a spike carries its status', () => {
    expect(read(body('sp-0001-aa')).assumptions[0]).toMatchObject({ spike: 'sp-0001-aa', spikeStatus: 'concluded' })
    expect(read(body('sp-0001-aa')).problems).toEqual([])
  })

  test('an id that names no item is a problem, and is kept as written with no status', () => {
    const { assumptions, problems } = read(body('sp-9999-zz'))
    expect(assumptions[0]).toMatchObject({ spike: 'sp-9999-zz' })
    expect(assumptions[0]).not.toHaveProperty('spikeStatus')
    expect(problems).toEqual([problem(4, /^A1 .*sp-9999-zz.*no item/)])
  })

  test('an item that is no spike is a problem, and carries no status', () => {
    for (const id of ['ts-0001-aa', 'pr-0003-km']) {
      const { assumptions, problems } = read(body(id))
      expect(assumptions[0]).toMatchObject({ spike: id })
      expect(assumptions[0], id).not.toHaveProperty('spikeStatus')
      expect(problems, id).toEqual([problem(4, /^A1 .*not a spike/)])
    }
  })

  test('a lookup that does not say the kind of an item names no spike', () => {
    const blind: ItemLookup = (id) => (KNOWN[id] === undefined ? undefined : { status: 'concluded', features: [] })
    const { problems } = readAssumptions(body('sp-0001-aa'), 1, blind)
    expect(problems).toEqual([expect.objectContaining({ reason: expect.stringMatching(/not a spike/) })])
  })

  test('a Spike that says nothing is no spike and no problem, and a second Spike is a problem', () => {
    expect(read(body(''))).toMatchObject({ assumptions: [{ id: 'A1' }], problems: [] })
    expect(read(body('')).assumptions[0]).not.toHaveProperty('spike')
    const twice = read(entry('  Spike: sp-0001-aa', '  Spike: sp-0002-bb'))
    expect(twice.assumptions[0]).toMatchObject({ spike: 'sp-0001-aa' })
    expect(twice.problems).toEqual([problem(5, /^A1 .*Spike:/)])
  })
})

describe('the Evidence of an assumption', () => {
  test('an id that names no item is listed with no status, with the problem on its line', () => {
    const { assumptions, problems } = read(entry('  Evidence: ts-0001-aa, ts-9999-zz'))
    expect(assumptions[0]!.evidence).toEqual([
      { target: 'ts-0001-aa', kind: 'item', status: 'ready' },
      { target: 'ts-9999-zz', kind: 'item' },
    ])
    expect(problems).toEqual([problem(4, /^A1 .*ts-9999-zz.*no item/)])
  })

  test('a feature the proposal has not, and a path with .. in it, are problems; a repeated label adds', () => {
    const { assumptions, problems } = read(entry('  Evidence: pr-0003-km#F9', '  Evidence: ../outside.md, sp-0001-aa'))
    expect(assumptions[0]!.evidence.map((one) => one.target)).toEqual(['pr-0003-km#F9', '../outside.md', 'sp-0001-aa'])
    expect(problems.map((problem) => problem.line)).toEqual([4, 5])
  })

  test('an empty Evidence is no evidence and no problem', () => {
    expect(read(entry('  Evidence:', '  Evidence: ts-0001-aa, ,'))).toMatchObject({
      assumptions: [{ evidence: [{ target: 'ts-0001-aa' }] }],
      problems: [],
    })
  })
})

describe('the other problems of the ledger', () => {
  test('a repeated A<n> is a problem that names the first line, also across two parts, and the first stays', () => {
    const body = text(
      '## Assumptions',
      '- **A1 First**: a',
      '  State: verified',
      '- **A1 Again**: b',
      '## Context',
      '## Assumptions',
      '- **A1 Third**: c',
    )
    const { assumptions, problems } = read(body)
    expect(assumptions.map((assumption) => assumption.title)).toEqual(['First'])
    expect(problems.map((problem) => `${problem.line}: ${problem.reason}`)).toEqual([
      expect.stringContaining('4: A1 is already used on line 2'),
      expect.stringContaining('7: A1 is already used on line 2'),
    ])
  })

  test('an unknown label is a problem on its line, and the entry stays', () => {
    const { assumptions, problems } = read(entry('  Owner: me'))
    expect(assumptions).toHaveLength(1)
    expect(problems).toEqual([{ part: 'Assumptions', line: 4, reason: expect.stringMatching(/^A1 .*Owner:/) }])
  })

  test('every problem is reported in one pass, in line order', () => {
    const body = text(
      '## Assumptions',
      '- **A1 A**: x',
      '  Spike: ts-0001-aa',
      '  State: maybe',
      '- **A2 B**: x',
      '  Evidence: ts-9999-zz',
      '  Owner: me',
    )
    expect(read(body).problems.map((problem) => problem.line)).toEqual([3, 4, 6, 7])
  })

  test('a heading line is not a problem, and an entry after it is read', () => {
    const body = '## Assumptions\n\n### Storage\n- **A1 T**: x\n'
    expect(read(body)).toMatchObject({ assumptions: [{ id: 'A1' }], problems: [] })
  })
})

describe('an entry that is very long', () => {
  test('reads in full: thousands of continuation lines and one line of 200 000 characters', () => {
    const lines = Array.from({ length: 5000 }, (_, index) => `  line ${index}`)
    const body = text('## Assumptions', '- **A1 T**: start', ...lines, `  ${'x'.repeat(200_000)}`, '  State: verified')
    const { assumptions, problems } = read(body)
    expect(problems).toEqual([])
    expect(assumptions[0]!.text.length).toBeGreaterThan(200_000)
    expect(assumptions[0]!.state).toBe('verified')
  }, 30_000)

  test('a title of 100 000 asterisks does not hang the reader', () => {
    const body = `## Assumptions\n- **A1 ${'*'.repeat(100_000)}\n- **A2 ${'**'.repeat(50_000)}: x\n`
    expect(read(body).problems.length).toBeGreaterThan(0)
  }, 30_000)
})
