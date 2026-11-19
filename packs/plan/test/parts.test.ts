import { describe, expect, test } from 'vitest'
import { readPart } from '../src/parts.ts'
import type { PartSpec } from '../src/parts.ts'

/** A made-up part: heading `## Assumptions`, entries `A<n>`, two labels. Shows the reader is not the Decision log's. */
const ASSUMPTIONS: PartSpec = { part: 'Assumptions', letter: 'A', labels: ['State', 'Spike'] }

const read = (body: string, firstLine = 1) => readPart(body, firstLine, ASSUMPTIONS)

/** A body of these lines, each ending in a line break. */
const text = (...lines: string[]): string => `${lines.join('\n')}\n`

describe('the entry form, shared by every part', () => {
  test('reads an entry, its continuation lines and its labelled lines, with each line of the file', () => {
    const { entries, problems } = read(
      [
        '## Assumptions',
        '',
        '- **A1 Files are enough**: the plan leans on it',
        '  and it goes on.',
        '  State: verified',
        '  Spike: sp-0001-aa',
        '',
      ].join('\n'),
    )
    expect(problems).toEqual([])
    expect(entries).toEqual([
      {
        id: 'A1',
        title: 'Files are enough',
        text: 'the plan leans on it and it goes on.',
        line: 3,
        labels: [
          { name: 'State', value: 'verified', line: 5 },
          { name: 'Spike', value: 'sp-0001-aa', line: 6 },
        ],
      },
    ])
  })

  test('a line that starts with a label is a label, and the lines after it continue its value', () => {
    const { entries } = read('## Assumptions\n- **A1 T**: text\n  State: verified\n    by a spike\n    and a review\n')
    expect(entries[0]).toMatchObject({
      text: 'text',
      labels: [{ name: 'State', value: 'verified by a spike and a review', line: 3 }],
    })
  })

  test('the heading is matched in any case after trimming, and a different heading is not the part', () => {
    expect(read('##   ASSUMPTIONS  \n- **A1 T**: x\n').entries).toHaveLength(1)
    expect(read('## Assumptions and more\n- **A1 T**: x\n').entries).toEqual([])
  })

  test('only the letter of the part makes an entry', () => {
    const { entries, problems } = read('## Assumptions\n- **D1 T**: a decision, not an assumption\n- **A2 U**: yes\n')
    expect(entries.map((entry) => entry.id)).toEqual(['A2'])
    expect(problems).toEqual([])
  })

  test('a repeated number is a problem that names the first line, and the first entry stays', () => {
    const { entries, problems } = read(
      text('## Assumptions', '- **A1 First**: a', '- **A2 B**: b', '- **A1 Again**: c', '  State: verified'),
    )
    expect(entries.map((entry) => entry.title)).toEqual(['First', 'B'])
    expect(problems).toEqual([
      { part: 'Assumptions', line: 4, reason: expect.stringContaining('A1 is already used on line 2') },
    ])
  })

  test('A0 is a problem, because numbers count from 1', () => {
    const { entries, problems } = read('## Assumptions\n- **A0 Zero**: a\n')
    expect(entries).toEqual([])
    expect(problems).toEqual([{ part: 'Assumptions', line: 2, reason: expect.stringContaining('count from 1') }])
  })

  test('an entry that is almost the form is a problem, and its continuation lines are not reported again', () => {
    const { entries, problems } = read(
      text('## Assumptions', '- **A1 No colon** text', '  State: verified', '  Bogus: x', '- **A02 Zero**: x'),
    )
    expect(entries).toEqual([])
    expect(problems.map((problem) => problem.line)).toEqual([2, 5])
  })

  test('an unknown label is a problem on its line, and the entry stays', () => {
    const { entries, problems } = read('## Assumptions\n- **A1 T**: x\n  Because: it is so\n  State: verified\n')
    expect(entries).toHaveLength(1)
    expect(entries[0]!.labels).toEqual([{ name: 'State', value: 'verified', line: 4 }])
    expect(problems).toEqual([{ part: 'Assumptions', line: 3, reason: expect.stringContaining('Because:') }])
  })

  test('a lowercase word and a colon in a continuation line is text, not a label', () => {
    const { entries, problems } = read('## Assumptions\n- **A1 T**: x\n  note: this is prose\n')
    expect(problems).toEqual([])
    expect(entries[0]!.text).toBe('x note: this is prose')
  })

  test('text that is not an entry is read as nothing, with no problem', () => {
    const { entries, problems } = read('## Assumptions\n\nWe assume a lot.\n- **Chosen**: this\n  more\n')
    expect(entries).toEqual([])
    expect(problems).toEqual([])
  })
})
