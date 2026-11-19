import { describe, expect, test } from 'vitest'
import { readPart } from '../src/parts.ts'
import type { PartSpec } from '../src/parts.ts'

/** A made-up part with two headings, as the Findings part has. A part with none reads headings as plain text. */
const SORTED: PartSpec = { part: 'Notes', letter: 'N', labels: ['Why'], sections: ['One', 'Two'] }
const UNSORTED: PartSpec = { part: 'Notes', letter: 'N', labels: ['Why'] }

const text = (...lines: string[]): string => `${lines.join('\n')}\n`

describe('a part that names its headings', () => {
  test('an entry carries the heading it is under, as the part spells it, and a heading is matched in any case', () => {
    const body = text('## Notes', '### one', '- **N1 A**: x', '###  TWO ', '- **N2 B**: y')
    const { entries, problems } = readPart(body, 1, SORTED)
    expect(problems).toEqual([])
    expect(entries.map((entry) => [entry.id, entry.section])).toEqual([
      ['N1', 'One'],
      ['N2', 'Two'],
    ])
  })

  test('a heading that is not one of them is a problem naming the line and both headings, and ends the section', () => {
    const body = text('## Notes', '### One', '- **N1 A**: x', '### Three', '- **N2 B**: y')
    const { entries, problems } = readPart(body, 5, SORTED)
    expect(problems).toEqual([{ part: 'Notes', line: 8, reason: expect.stringMatching(/"Three".*One.*Two/) }])
    expect(entries.map((entry) => entry.section)).toEqual(['One', undefined])
  })

  test('an entry before any heading has no section, and a new part starts with none', () => {
    const body = text('## Notes', '- **N1 A**: x', '### One', '## Other', '## Notes', '- **N2 B**: y')
    const { entries, problems } = readPart(body, 1, SORTED)
    expect(problems).toEqual([])
    expect(entries.map((entry) => entry.section)).toEqual([undefined, undefined])
  })

  test('only a line of three # and a space is a heading, and not inside a fence or an entry', () => {
    const body = text('## Notes', '### One', '```', '### Three', '```', '#### Four', '- **N1 A**: x', '  ### Five')
    const { entries, problems } = readPart(body, 1, SORTED)
    expect(problems).toEqual([])
    expect(entries[0]).toMatchObject({ section: 'One', text: 'x ### Five' })
  })

  test('a heading outside the part is not read', () => {
    const body = text('## Context', '### Three', '## Notes', '### One', '- **N1 A**: x')
    expect(readPart(body, 1, SORTED).problems).toEqual([])
  })
})

describe('a part that names none', () => {
  test('a ### heading is plain text: no problem, and an entry has no section key', () => {
    const body = text('## Notes', '### Anything', '- **N1 A**: x')
    const { entries, problems } = readPart(body, 1, UNSORTED)
    expect(problems).toEqual([])
    expect(entries[0]).not.toHaveProperty('section')
  })
})
