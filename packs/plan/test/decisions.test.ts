import { describe, expect, test } from 'vitest'
import { readDecisions } from '../src/decisions.ts'
import type { ItemLookup } from '../src/decisions.ts'

/** The items of a made-up scan: a task, a concluded spike and a proposal with two features. */
const KNOWN: Record<string, { status: string; features: string[] }> = {
  'ts-0001-aa': { status: 'ready', features: [] },
  'sp-0001-aa': { status: 'concluded', features: [] },
  'pr-0003-km': { status: 'draft', features: ['F1', 'F2'] },
}
const lookup: ItemLookup = (id) => KNOWN[id]

const read = (body: string, firstLine = 1) => readDecisions(body, firstLine, lookup)

/** A body of these lines, each ending in a line break. */
const text = (...lines: string[]): string => `${lines.join('\n')}\n`
/** A decision, `D1`, with the lines under it. It has an alternative, so it has no problem of its own. */
const entry = (...under: string[]): string => text('## Decision', '- **D1 T**: x', '  Passed on: y', ...under)
const problemsOf = (body: string, firstLine = 1): string[] =>
  read(body, firstLine).problems.map((problem) => `${problem.line}: ${problem.reason}`)

const FULL = [
  '## Decision',
  '',
  '- **D1 Use files**: Plain files in the repo.',
  '  The text goes on.',
  '  Passed on: A database, because it needs a server.',
  '  Passed on: A wiki, since nobody reviews it.',
  '  Evidence: sp-0001-aa, pr-0003-km#F2, core/src/item.rs',
  '  Revisit if: the files pass 10 MB.',
  '  Confidence: medium',
  '',
].join('\n')

describe('an entry of the Decision log', () => {
  test('carries its title, its text and each label, with its line', () => {
    const { decisions, problems } = read(FULL)
    expect(problems).toEqual([])
    expect(decisions).toEqual([
      {
        id: 'D1',
        title: 'Use files',
        text: 'Plain files in the repo. The text goes on.',
        passedOn: ['A database, because it needs a server.', 'A wiki, since nobody reviews it.'],
        evidence: [
          { target: 'sp-0001-aa', kind: 'item', status: 'concluded' },
          { target: 'pr-0003-km#F2', kind: 'feature', proposal: 'pr-0003-km' },
          { target: 'core/src/item.rs', kind: 'path' },
        ],
        revisitIf: 'the files pass 10 MB.',
        confidence: 'medium',
        line: 3,
      },
    ])
  })

  test('has no optional key it did not write, and no label of its own is needed for the text', () => {
    const { decisions, problems } = read('## Decision\n- **D1 T**:\n  Passed on: x, since y\n')
    expect(problems).toEqual([])
    expect(decisions).toEqual([{ id: 'D1', title: 'T', text: '', passedOn: ['x, since y'], evidence: [], line: 2 }])
  })

  test('a label may continue on the lines below it, and a line of the file is a line of the file', () => {
    const body = '## Decision\n- **D1 T**: x\n  Passed on: A db\n    because it needs a server\n  Confidence: high\n'
    const { decisions, problems } = read(body, 10)
    expect(problems).toEqual([])
    expect(decisions[0]).toMatchObject({ passedOn: ['A db because it needs a server'], confidence: 'high', line: 11 })
  })

  test('the heading is matched in any case after trimming, and the entries of every Decision part count', () => {
    const body = text(
      '##   DECISION  ',
      '- **D1 A**: x',
      '  Passed on: y',
      '',
      '## Context',
      '',
      '## decision',
      '- **D2 B**: x',
      '  Passed on: y',
    )
    expect(read(body).decisions.map((decision) => decision.id)).toEqual(['D1', 'D2'])
    expect(read('## Decisions\n- **D1 A**: x\n').decisions).toEqual([])
  })

  test('CRLF reads as LF does, with no carriage return in any value', () => {
    const lf = read(FULL)
    const crlf = read(FULL.replaceAll('\n', '\r\n'))
    expect(crlf).toEqual(lf)
    expect(JSON.stringify(crlf)).not.toContain('\\r')
  })
})

describe('what is not the Decision log', () => {
  test('plain text in the section is read as nothing, with no problem', () => {
    const body = text(
      '## Decision',
      '',
      'We chose files.',
      '',
      '- **Chosen**: files',
      '  because they diff.',
      '',
      '### Why',
      '',
      '- one',
      '- two',
    )
    expect(read(body)).toEqual({ decisions: [], problems: [] })
  })

  test('a body with no Decision section has none', () => {
    expect(read('## Context\n\nText.\n')).toEqual({ decisions: [], problems: [] })
  })

  test('an entry under another heading is not read, and the part ends at the next ## heading', () => {
    const body = '## Context\n- **D1 A**: x\n\n## Decision\n- **D2 B**: x\n  Passed on: y\n\n## Notes\n- **D3 C**: x\n'
    expect(read(body).decisions.map((decision) => decision.id)).toEqual(['D2'])
    expect(read(body).problems).toEqual([])
  })

  test('a fenced block is not read, and a Decision heading inside one is not a part', () => {
    const body = text(
      '## Decision',
      '```',
      '- **D1 A**: x',
      '```',
      '- **D2 B**: x',
      '  Passed on: y',
      '',
      '## Context',
      '~~~',
      '## Decision',
      '- **D3 C**: x',
      '~~~',
    )
    expect(read(body).decisions.map((decision) => decision.id)).toEqual(['D2'])
    expect(read(body).problems).toEqual([])
  })
})

describe('each problem has its line and starts with the id of the entry', () => {
  test('a decision with no alternative is listed, with the problem on its first line', () => {
    const { decisions, problems } = read('## Decision\n- **D1 T**: x\n  Evidence: ts-0001-aa\n')
    expect(decisions.map((decision) => decision.id)).toEqual(['D1'])
    expect(problems).toEqual([{ part: 'Decision', line: 2, reason: expect.stringMatching(/^D1 .*Passed on:/) }])
  })

  test('an empty Passed on is no alternative', () => {
    const problems = problemsOf(text('## Decision', '- **D1 T**: x', '  Passed on:'))
    expect(problems).toEqual([expect.stringMatching(/^2: D1 .*Passed on:/)])
  })

  test('an evidence id that names no item is listed with no status, with the problem on its line', () => {
    const { decisions, problems } = read(entry('  Evidence: ts-0001-aa, ts-9999-zz'))
    expect(decisions[0]!.evidence).toEqual([
      { target: 'ts-0001-aa', kind: 'item', status: 'ready' },
      { target: 'ts-9999-zz', kind: 'item' },
    ])
    expect(problems).toEqual([
      { part: 'Decision', line: 4, reason: expect.stringMatching(/^D1 .*ts-9999-zz.*no item/) },
    ])
  })

  test('a feature the proposal has not, a proposal that is not there, and a fragment that is no feature', () => {
    const body = (target: string): string => `## Decision\n- **D1 T**: x\n  Passed on: y\n  Evidence: ${target}\n`
    expect(problemsOf(body('pr-0003-km#F9'))).toEqual([expect.stringMatching(/^4: D1 .*pr-0003-km has no feature F9/)])
    expect(problemsOf(body('pr-0099-zz#F1'))).toEqual([expect.stringMatching(/^4: D1 .*pr-0099-zz.*no item/)])
    expect(problemsOf(body('ts-0001-aa#F1'))).toEqual([expect.stringMatching(/^4: D1 .*ts-0001-aa has no feature F1/)])
    expect(problemsOf(body('pr-0003-km#F0'))).toEqual([expect.stringMatching(/^4: D1 .*F2/)])
    expect(problemsOf(body('pr-0003-km#F1'))).toEqual([])
  })

  test('a repo path is checked for its shape only: relative, with no .. in it', () => {
    const body = (target: string): string => `## Decision\n- **D1 T**: x\n  Passed on: y\n  Evidence: ${target}\n`
    expect(problemsOf(body('core/src/not-here.rs, Cargo.toml, docs/a..b.md'))).toEqual([])
    expect(problemsOf(body('/etc/passwd'))).toEqual([expect.stringMatching(/^4: D1 .*\/etc\/passwd.*relative/)])
    expect(problemsOf(body('../outside.md'))).toEqual([expect.stringMatching(/^4: D1 .*\.\.\/outside\.md/)])
    expect(problemsOf(body('a/../../b'))).toEqual([expect.stringMatching(/^4: D1 .*a\/\.\.\/\.\.\/b/)])
  })

  test('empty pieces of an Evidence list are ignored, and an empty Evidence is no evidence', () => {
    const { decisions, problems } = read(entry('  Evidence: ts-0001-aa, , ,', '  Evidence:'))
    expect(problems).toEqual([])
    expect(decisions[0]!.evidence).toEqual([{ target: 'ts-0001-aa', kind: 'item', status: 'ready' }])
  })

  test('a repeated Evidence label adds to the list', () => {
    const { decisions } = read(entry('  Evidence: ts-0001-aa', '  Evidence: sp-0001-aa'))
    expect(decisions[0]!.evidence.map((entry) => entry.target)).toEqual(['ts-0001-aa', 'sp-0001-aa'])
  })

  test('a repeated D<n> is a problem that names the first line, and the first stays', () => {
    const body = '## Decision\n- **D1 First**: a\n  Passed on: y\n- **D1 Again**: b\n  Passed on: z\n'
    const { decisions, problems } = read(body)
    expect(decisions.map((decision) => decision.title)).toEqual(['First'])
    expect(problems).toEqual([
      { part: 'Decision', line: 4, reason: expect.stringContaining('D1 is already used on line 2') },
    ])
  })

  test('a number is not reused across two Decision parts either', () => {
    const body = '## Decision\n- **D1 A**: a\n  Passed on: y\n## Context\n## Decision\n- **D1 B**: b\n  Passed on: y\n'
    expect(problemsOf(body)).toEqual([expect.stringContaining('6: D1 is already used on line 2')])
  })

  test('a Confidence that is none of the three is a problem and is not kept; the three are kept as written', () => {
    const body = (value: string): string => `## Decision\n- **D1 T**: x\n  Passed on: y\n  Confidence: ${value}\n`
    for (const value of ['low', 'medium', 'high']) {
      expect(read(body(value))).toMatchObject({ decisions: [{ confidence: value }], problems: [] })
    }
    for (const value of ['sure', 'High', '']) {
      const { decisions, problems } = read(body(value))
      expect(decisions[0]).not.toHaveProperty('confidence')
      expect(problems).toEqual([
        { part: 'Decision', line: 4, reason: expect.stringMatching(/^D1 .*low, medium or high/) },
      ])
    }
  })

  test('a label given twice that takes one value is a problem, and the first value stays', () => {
    const body = entry('  Revisit if: a', '  Revisit if: b', '  Confidence: low', '  Confidence: high')
    const { decisions, problems } = read(body)
    expect(decisions[0]).toMatchObject({ revisitIf: 'a', confidence: 'low' })
    expect(problems.map((problem) => problem.line)).toEqual([5, 7])
  })

  test('an unknown label is a problem on its line, and the entry stays', () => {
    const { decisions, problems } = read('## Decision\n- **D1 T**: x\n  Passed on: y\n  Owner: me\n')
    expect(decisions).toHaveLength(1)
    expect(problems).toEqual([{ part: 'Decision', line: 4, reason: expect.stringMatching(/^D1 .*Owner:/) }])
  })

  test('every problem is reported in one pass, in line order', () => {
    const body = [
      '## Decision',
      '- **D1 A**: x',
      '  Evidence: ts-9999-zz',
      '  Owner: me',
      '- **D2 B**: x',
      '  Passed on: y',
      '  Confidence: sure',
      '',
    ].join('\n')
    expect(read(body).problems.map((problem) => problem.line)).toEqual([2, 3, 4, 7])
  })

  test('a heading line is not a problem, and an entry after it is read', () => {
    const body = '## Decision\n\n### Storage\n- **D1 T**: x\n  Passed on: y\n'
    expect(read(body)).toMatchObject({ decisions: [{ id: 'D1' }], problems: [] })
  })
})

describe('an entry that is very long', () => {
  test('reads in full: thousands of continuation lines and one line of 200 000 characters', () => {
    const lines = Array.from({ length: 5000 }, (_, index) => `  line ${index}`)
    const long = `  ${'x'.repeat(200_000)}`
    const body = text('## Decision', '- **D1 T**: start', ...lines, long, '  Passed on: y')
    const { decisions, problems } = read(body)
    expect(problems).toEqual([])
    expect(decisions[0]!.text.length).toBeGreaterThan(200_000)
    expect(decisions[0]!.text.startsWith('start line 0 line 1')).toBe(true)
    expect(decisions[0]!.passedOn).toEqual(['y'])
  }, 30_000)

  test('a title of 100 000 asterisks does not hang the reader', () => {
    const body = `## Decision\n- **D1 ${'*'.repeat(100_000)}\n- **D2 ${'**'.repeat(50_000)}: x\n`
    const { problems } = read(body)
    expect(problems.length).toBeGreaterThan(0)
  }, 30_000)
})
