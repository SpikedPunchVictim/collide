import { describe, expect, test } from 'vitest'
import { readFindings } from '../src/findings.ts'

/** What a made-up proposal has: two features, a decision and an assumption. */
const OWN = new Set(['F1', 'F2', 'D1', 'A1'])

const read = (body: string, firstLine = 1) => readFindings(body, firstLine, OWN)

/** A body of these lines, each ending in a line break. */
const text = (...lines: string[]): string => `${lines.join('\n')}\n`
/** A red-team finding, `R1`, with the lines under it. */
const red = (...lines: string[]): string =>
  text('## Findings', '### Red-team', '- **R1 high Two writers**: x', ...lines)
/** A problem of the part, on `line`, whose reason matches `reason`. */
const problem = (line: number, reason: RegExp | string) => ({
  part: 'Findings',
  line,
  reason: typeof reason === 'string' ? expect.stringContaining(reason) : expect.stringMatching(reason),
})

const FULL = [
  '## Findings',
  '',
  '### Red-team',
  '- **R1 high Two writers**: Two writers race.',
  '  The text goes on.',
  '  Target: D1',
  '  Dismissed: The lock covers it.',
  '',
  '### Pre-mortem',
  '- **R2 low Slow scan**: The scan is slow.',
  '  Target: F2',
  '  Accepted: Added a cache to the plan.',
  '- **R3 medium Loose end**: Nobody owns it.',
  '',
].join('\n')

describe('an entry of the Findings part', () => {
  test('carries its section, severity, title, text and each label, with its line', () => {
    const { findings, problems } = read(FULL)
    expect(problems).toEqual([])
    expect(findings).toEqual([
      {
        id: 'R1',
        section: 'Red-team',
        severity: 'high',
        title: 'Two writers',
        text: 'Two writers race. The text goes on.',
        target: 'D1',
        dismissed: 'The lock covers it.',
        open: false,
        line: 4,
      },
      {
        id: 'R2',
        section: 'Pre-mortem',
        severity: 'low',
        title: 'Slow scan',
        text: 'The scan is slow.',
        target: 'F2',
        accepted: 'Added a cache to the plan.',
        open: false,
        line: 10,
      },
      {
        id: 'R3',
        section: 'Pre-mortem',
        severity: 'medium',
        title: 'Loose end',
        text: 'Nobody owns it.',
        open: true,
        line: 13,
      },
    ])
  })

  test('each severity reads as written, with no problem', () => {
    for (const severity of ['high', 'medium', 'low']) {
      const { findings, problems } = read(`## Findings\n### Red-team\n- **R1 ${severity} T**: x\n`)
      expect(problems).toEqual([])
      expect(findings[0]).toMatchObject({ severity, title: 'T' })
    }
  })

  test('a label may continue on the lines below it, and a line of the file is a line of the file', () => {
    const body = '## Findings\n### Pre-mortem\n- **R1 low T**: x\n  Dismissed: Not now,\n    it is rare.\n'
    const { findings, problems } = read(body, 10)
    expect(problems).toEqual([])
    expect(findings[0]).toMatchObject({ dismissed: 'Not now, it is rare.', line: 12 })
  })

  test('the headings are matched in any case after trimming, and give the name as the part spells it', () => {
    const body = text('##  FINDINGS ', '###   RED-TEAM  ', '- **R1 low A**: x', '### pre-mortem', '- **R2 low B**: x')
    const { findings, problems } = read(body)
    expect(problems).toEqual([])
    expect(findings.map((finding) => finding.section)).toEqual(['Red-team', 'Pre-mortem'])
  })

  test('CRLF reads as LF does, with no carriage return in any value', () => {
    const lf = read(FULL)
    const crlf = read(FULL.replaceAll('\n', '\r\n'))
    expect(crlf).toEqual(lf)
    expect(JSON.stringify(crlf)).not.toContain('\\r')
  })
})

describe('whether a finding is open', () => {
  test('a finding with neither Accepted nor Dismissed is open, whatever its severity, and is no problem', () => {
    for (const severity of ['high', 'medium', 'low']) {
      const { findings, problems } = read(`## Findings\n### Red-team\n- **R1 ${severity} T**: x\n  Target: F1\n`)
      expect(findings[0]!.open).toBe(true)
      expect(problems).toEqual([])
    }
  })

  test('a finding with Accepted is not open, and nor is one with Dismissed', () => {
    for (const label of ['Accepted: Changed the plan.', 'Dismissed: Not a risk.']) {
      const { findings, problems } = read(red(`  ${label}`))
      expect(findings[0]!.open, label).toBe(false)
      expect(problems, label).toEqual([])
    }
  })

  test('a finding with both is a problem on the line of the second, and has both as written; it is not open', () => {
    const { findings, problems } = read(red('  Accepted: Changed the plan.', '  Dismissed: Not a risk.'))
    expect(findings[0]).toMatchObject({ accepted: 'Changed the plan.', dismissed: 'Not a risk.', open: false })
    expect(problems).toEqual([problem(5, /^R1 .*Accepted.*Dismissed/)])
  })

  test('an Accepted or a Dismissed that says nothing is a problem, and the finding stays open', () => {
    for (const label of ['Accepted', 'Dismissed']) {
      const { findings, problems } = read(red(`  ${label}:`))
      expect(findings[0]!.open, label).toBe(true)
      expect(findings[0], label).not.toHaveProperty(label.toLowerCase())
      expect(problems, label).toEqual([problem(4, `R1 has an empty ${label}:`)])
    }
  })

  test('a label given twice that takes one value is a problem, and the first value stays', () => {
    const twice = red('  Dismissed: first', '  Dismissed: second', '  Target: F1', '  Target: F2')
    const { findings, problems } = read(twice)
    expect(findings[0]).toMatchObject({ dismissed: 'first', target: 'F1' })
    expect(problems.map((problem) => problem.line)).toEqual([5, 7])
  })
})

describe('the severity of a finding', () => {
  test('a severity that is none of the three is a problem on the entry, and is kept as written', () => {
    for (const severity of ['urgent', 'High', 'LOW']) {
      const { findings, problems } = read(`## Findings\n### Red-team\n- **R1 ${severity} T**: x\n`)
      expect(findings[0], severity).toMatchObject({ severity, title: 'T' })
      expect(problems, severity).toEqual([
        { part: 'Findings', line: 3, reason: expect.stringMatching(/^R1 .*high, medium or low.*"[A-Za-z]+"/) },
      ])
    }
  })

  test('a title with no severity before it names its first word as the severity, which is the problem', () => {
    const { findings, problems } = read('## Findings\n### Red-team\n- **R1 Two writers race**: x\n')
    expect(findings[0]).toMatchObject({ severity: 'Two', title: 'writers race' })
    expect(problems).toHaveLength(1)
  })

  test('a severity and no title is a problem, and the title is empty', () => {
    const { findings, problems } = read('## Findings\n### Red-team\n- **R1 high**: x\n')
    expect(findings[0]).toMatchObject({ severity: 'high', title: '' })
    expect(problems).toEqual([{ part: 'Findings', line: 3, reason: expect.stringMatching(/^R1 .*title/) }])
  })
})

describe('the Target of a finding', () => {
  const target = (value: string): string => red(`  Target: ${value}`)

  test('a feature, a decision and an assumption of the proposal are each a target', () => {
    for (const id of ['F1', 'F2', 'D1', 'A1']) {
      expect(read(target(id)), id).toMatchObject({ findings: [{ target: id }], problems: [] })
    }
  })

  test('a target that the proposal has not is a problem on its line, and is kept as written', () => {
    for (const id of ['D2', 'F3', 'A9']) {
      const { findings, problems } = read(target(id))
      expect(findings[0], id).toMatchObject({ target: id })
      expect(problems, id).toEqual([
        { part: 'Findings', line: 4, reason: expect.stringMatching(new RegExp(`^R1 .*${id}.*does not have`)) },
      ])
    }
  })

  test('a target that is none of F<n>, D<n> or A<n> is a problem, whatever it names elsewhere', () => {
    for (const value of ['ts-0001-aa', 'R1', 'F0', 'f1', 'F1, D1', '']) {
      const { problems } = read(target(value))
      expect(problems, value).toEqual([
        { part: 'Findings', line: 4, reason: expect.stringMatching(/^R1 .*F<n>, D<n> or A<n>/) },
      ])
    }
  })

  test('a finding with no Target is no problem', () => {
    expect(read(red())).toMatchObject({ findings: [{ id: 'R1' }], problems: [] })
    expect(read(red()).findings[0]).not.toHaveProperty('target')
  })

  test('a target is checked against what the caller says the proposal has', () => {
    expect(readFindings(target('D2'), 1, new Set(['D2'])).problems).toEqual([])
    expect(readFindings(target('D1'), 1, new Set()).problems).toHaveLength(1)
  })
})

describe('the two headings', () => {
  test('a heading that is neither Red-team nor Pre-mortem is a problem naming its line', () => {
    const body = text('## Findings', '### Red-team', '### Notes', '### Red team', '#### Red-team')
    expect(read(body).problems).toEqual([
      { part: 'Findings', line: 3, reason: expect.stringContaining('"Notes"') },
      { part: 'Findings', line: 4, reason: expect.stringContaining('"Red team"') },
    ])
  })

  test('an entry under such a heading is a problem, is listed, and has no section', () => {
    const body = text('## Findings', '### Notes', '- **R1 low T**: x', '', '### Pre-mortem', '- **R2 low U**: x')
    const { findings, problems } = read(body)
    expect(findings.map((finding) => finding.id)).toEqual(['R1', 'R2'])
    expect(findings[0]).not.toHaveProperty('section')
    expect(findings[1]).toMatchObject({ section: 'Pre-mortem' })
    expect(problems.map((problem) => problem.line)).toEqual([2, 3])
    expect(problems[1]!.reason).toMatch(/^R1 .*outside.*Red-team.*Pre-mortem/)
  })

  test('an entry before any heading is a problem, is listed, and has no section', () => {
    const { findings, problems } = read('## Findings\n- **R1 low T**: x\n')
    expect(findings).toHaveLength(1)
    expect(findings[0]).not.toHaveProperty('section')
    expect(problems).toEqual([{ part: 'Findings', line: 2, reason: expect.stringMatching(/^R1 .*outside/) }])
  })

  test('the headings of one part do not carry into the next, and a heading outside the part is not read', () => {
    const body = text(
      '## Context',
      '### Whatever',
      '## Findings',
      '### Red-team',
      '- **R1 low A**: x',
      '## Notes',
      '- **R2 low B**: x',
      '## Findings',
      '- **R3 low C**: x',
    )
    const { findings, problems } = read(body)
    expect(findings.map((finding) => finding.id)).toEqual(['R1', 'R3'])
    expect(findings[1]).not.toHaveProperty('section')
    expect(problems.map((problem) => problem.line)).toEqual([9])
  })

  test('a number is shared by the two headings', () => {
    const body = text('## Findings', '### Red-team', '- **R1 low A**: x', '### Pre-mortem', '- **R1 low B**: x')
    const { findings, problems } = read(body)
    expect(findings.map((finding) => finding.title)).toEqual(['A'])
    expect(problems).toEqual([problem(5, 'R1 is already used on line 3')])
  })

  test('a heading is a heading only at the start of a line, and not inside a fence', () => {
    const body = text('## Findings', '### Red-team', '```', '### Notes', '```', '- **R1 low A**: x', '  ### Notes')
    expect(read(body)).toMatchObject({ findings: [{ id: 'R1', section: 'Red-team' }], problems: [] })
  })
})

describe('what is not the Findings part', () => {
  test('plain text in the part, under a heading or not, is read as nothing, with no problem', () => {
    const body = text('## Findings', '', 'No review.', '', '### Red-team', '', 'Nothing found.', '', '- one')
    expect(read(body)).toEqual({ findings: [], problems: [] })
  })

  test('a body with no Findings section has none, and a Finding heading is not the part', () => {
    expect(read('## Context\n\nText.\n')).toEqual({ findings: [], problems: [] })
    expect(read('## Finding\n### Red-team\n- **R1 low A**: x\n')).toEqual({ findings: [], problems: [] })
  })

  test('an unknown label is a problem on its line, and the entry stays', () => {
    const { findings, problems } = read(red('  Owner: me'))
    expect(findings).toHaveLength(1)
    expect(problems).toEqual([{ part: 'Findings', line: 4, reason: expect.stringMatching(/^R1 .*Owner:/) }])
  })

  test('every problem is reported in one pass, in line order', () => {
    const body = text(
      '## Findings',
      '- **R1 low A**: x',
      '### Red-team',
      '- **R2 odd B**: x',
      '  Target: D9',
      '  Owner: me',
      '### Notes',
    )
    expect(read(body).problems.map((problem) => problem.line)).toEqual([2, 4, 5, 6, 7])
  })
})

describe('an entry that is very long', () => {
  test('reads in full: thousands of continuation lines and one line of 200 000 characters', () => {
    const lines = Array.from({ length: 5000 }, (_, index) => `  line ${index}`)
    const body = text('## Findings', '### Red-team', '- **R1 high T**: start', ...lines, `  ${'x'.repeat(200_000)}`)
    const { findings, problems } = read(body)
    expect(problems).toEqual([])
    expect(findings[0]!.text.length).toBeGreaterThan(200_000)
    expect(findings[0]!.open).toBe(true)
  }, 30_000)

  test('a title of 100 000 asterisks does not hang the reader', () => {
    const body = `## Findings\n### Red-team\n- **R1 high ${'*'.repeat(100_000)}\n- **R2 low ${'**'.repeat(50_000)}: x\n`
    expect(read(body).problems.length).toBeGreaterThan(0)
  }, 30_000)
})
