import { describe, expect, test } from 'vitest'
import { parseFeatures } from '../src/features.ts'

/** 0.10's example, as a body that starts at line 1. */
const EXAMPLE = `## Features

### Packs
- **F1 Trust prompt for repo packs**: a repo pack asks before its first run.
  A continuation line is indented by two spaces.
- **F2 Pack lock**: \`packs.lock\` pins each pack to a tree.

### Unsorted
- **F3 Menu bar badge**: the Queue count in the menu bar.
`

/** The problems of `body` as `line: reason`, so a test can match the line and part of the reason. */
const problemsOf = (body: string, firstLine = 1): string[] => parseFeatures(body, firstLine).problems.map((problem) => `${problem.line}: ${problem.reason}`)

describe("0.10's example", () => {
  test('lists F1 and F2 in section Packs and F3 with no section, and joins the continuation line into the summary', () => {
    const { features, problems } = parseFeatures(EXAMPLE)
    expect(problems).toEqual([])
    expect(features).toEqual([
      { id: 'F1', title: 'Trust prompt for repo packs', section: 'Packs', summary: 'a repo pack asks before its first run. A continuation line is indented by two spaces.' },
      { id: 'F2', title: 'Pack lock', section: 'Packs', summary: '`packs.lock` pins each pack to a tree.' },
      { id: 'F3', title: 'Menu bar badge', section: '', summary: 'the Queue count in the menu bar.' },
    ])
  })

  test('reads the part from the middle of a body, and stops at the next ## heading', () => {
    const body = `## Context\n\nSome text. - **F9 Not a feature**: outside the part.\n\n${EXAMPLE}\n## Decision\n\n- **F8 Also outside**: after the part.\n`
    const { features, problems } = parseFeatures(body)
    expect(features.map((feature) => feature.id)).toEqual(['F1', 'F2', 'F3'])
    expect(problems).toEqual([])
  })
})

describe('sections', () => {
  test('an entry above the first heading has no section, and so has one under ### unsorted in any case', () => {
    const body = '## Features\n- **F1 Above**: before any heading.\n### unsorted\n- **F2 Lower**: x\n### UNSORTED\n- **F3 Upper**: y\n### Mixed\n- **F4 Named**: z\n'
    const { features, problems } = parseFeatures(body)
    expect(problems).toEqual([])
    expect(features.map((feature) => [feature.id, feature.section])).toEqual([['F1', ''], ['F2', ''], ['F3', ''], ['F4', 'Mixed']])
  })

  test('a heading is trimmed, and 40 characters is allowed', () => {
    const name = 'x'.repeat(40)
    const { features, problems } = parseFeatures(`## Features\n###   ${name}   \n- **F1 A**: b\n`)
    expect(problems).toEqual([])
    expect(features[0]?.section).toBe(name)
  })

  test('a section keeps the case it is written in, and two headings of one name give one section name', () => {
    const { features } = parseFeatures('## Features\n### Packs\n- **F1 A**: b\n### Other\n- **F2 C**: d\n### Packs\n- **F3 E**: f\n')
    expect(features.map((feature) => feature.section)).toEqual(['Packs', 'Other', 'Packs'])
  })
})

describe('a proposal with no usable part', () => {
  test('has no features and no problems when there is no Features heading', () => {
    expect(parseFeatures('## Context\n\nText only.\n')).toEqual({ features: [], problems: [] })
    expect(parseFeatures('')).toEqual({ features: [], problems: [] })
  })

  test('has none when the part is empty', () => {
    expect(parseFeatures('## Features\n\n## Decision\n')).toEqual({ features: [], problems: [] })
  })

  test('a ## Features line inside a fenced block elsewhere is not the part', () => {
    const body = '## Context\n\n```markdown\n## Features\n- **F1 In a fence**: not read.\nnot an entry\n```\n\n~~~\n## Features\n~~~\n'
    expect(parseFeatures(body)).toEqual({ features: [], problems: [] })
  })

  test('a line that only starts like the heading is not the part', () => {
    expect(parseFeatures('## Features and more\n- **F1 A**: b\n### Features\n- **F2 C**: d\n')).toEqual({ features: [], problems: [] })
  })

  test('the heading may carry trailing spaces and a CRLF ending', () => {
    const { features, problems } = parseFeatures('## Features  \r\n\r\n### Packs\r\n- **F1 A**: b\r\n  more\r\n')
    expect(problems).toEqual([])
    expect(features).toEqual([{ id: 'F1', title: 'A', section: 'Packs', summary: 'b more' }])
  })
})

describe('each problem is reported with its line, all in one pass', () => {
  test('an entry that is not in the form - **F<n> <title>**: <summary>', () => {
    const body = [
      '## Features',
      '- plain bullet',
      '- **F1 No colon** missing the colon',
      '- **F2**: no title',
      '- **F3 Empty summary**:',
      '- **Fx Not a number**: x',
      '- **F04 Leading zero**: x',
      '* **F5 Star bullet**: x',
      '',
    ].join('\n')
    const problems = problemsOf(body)
    expect(problems).toHaveLength(7)
    expect(problems.map((problem) => problem.split(':')[0])).toEqual(['2', '3', '4', '5', '6', '7', '8'])
    expect(problems[0]).toContain('- **F<n> <title>**: <summary>')
  })

  test('a repeated F<n>: the second is reported, names the line of the first, and the first stays', () => {
    const { features, problems } = parseFeatures('## Features\n- **F1 First**: a\n- **F2 Second**: b\n- **F1 Again**: c\n  more of it\n')
    expect(features.map((feature) => feature.title)).toEqual(['First', 'Second'])
    expect(problems).toEqual([{ line: 4, reason: expect.stringContaining('F1 is already used on line 2') }])
  })

  test('F0, because numbers count from 1', () => {
    const { features, problems } = parseFeatures('## Features\n- **F0 Zero**: a\n')
    expect(features).toEqual([])
    expect(problems).toEqual([{ line: 2, reason: expect.stringContaining('count from 1') }])
  })

  test('an empty section heading, and one over 40 characters, and one with a #', () => {
    const body = `## Features\n###\n###    \n### ${'x'.repeat(41)}\n### Pack #1\n`
    const problems = problemsOf(body)
    expect(problems.map((problem) => problem.split(':')[0])).toEqual(['2', '3', '4', '5'])
    expect(problems[0]).toContain('empty')
    expect(problems[2]).toContain('40 characters')
    expect(problems[3]).toContain('#')
  })

  test('a #### heading', () => {
    const problems = problemsOf('## Features\n#### Too deep\n- **F1 A**: b\n')
    expect(problems).toEqual(['2: only ### section headings are allowed here, not ####'])
  })

  test('a line that is neither an entry nor a continuation', () => {
    const problems = problemsOf('## Features\nJust some text.\n- **F1 A**: b\n\nMore text after a blank line.\n###Heading with no space\n')
    expect(problems.map((problem) => problem.split(':')[0])).toEqual(['2', '5', '6'])
    expect(problems[0]).toContain('neither an entry nor a continuation')
  })

  test('a continuation indented by one space', () => {
    const problems = problemsOf('## Features\n- **F1 A**: b\n c one space\n  d two spaces\n')
    expect(problems).toEqual(['3: a continuation line is indented by at least two spaces, not one'])
  })

  test('a continuation with no entry before it, and one after a heading', () => {
    const problems = problemsOf('## Features\n  nothing to continue\n- **F1 A**: b\n### Sec\n  its entry was closed by the heading\n')
    expect(problems.map((problem) => problem.split(':')[0])).toEqual(['2', '5'])
  })

  test('a continuation after a blank line still continues the entry', () => {
    const { features, problems } = parseFeatures('## Features\n- **F1 A**: b\n\n  after a blank line\n')
    expect(problems).toEqual([])
    expect(features[0]?.summary).toBe('b after a blank line')
  })

  test('the continuation lines of an entry that was refused are not reported again', () => {
    const problems = problemsOf('## Features\n- **F0 Zero**: a\n  more of zero\n  and more\n- **F1 A**: b\n- **F1 Dup**: c\n  dup more\n')
    expect(problems.map((problem) => problem.split(':')[0])).toEqual(['2', '6'])
  })

  test('a second ## Features part, with its content left unread', () => {
    const body = '## Features\n- **F1 A**: b\n\n## Context\n\n## Features\n- **F2 Ignored**: c\nnot an entry\n\n## Decision\n'
    const { features, problems } = parseFeatures(body)
    expect(features.map((feature) => feature.id)).toEqual(['F1'])
    expect(problems).toEqual([{ line: 6, reason: expect.stringContaining('second ## Features part') }])
  })

  test('a fenced block inside the part, whose content is not reported line by line', () => {
    const body = '## Features\n- **F1 A**: b\n```\nsome code\n- not an entry\n```\n- **F2 C**: d\n'
    const { features, problems } = parseFeatures(body)
    expect(features.map((feature) => feature.id)).toEqual(['F1', 'F2'])
    expect(problems).toEqual([{ line: 3, reason: expect.stringContaining('fenced') }])
  })

  test('a tab is not an indent', () => {
    expect(problemsOf('## Features\n- **F1 A**: b\n\tby a tab\n')).toHaveLength(1)
  })

  test('every problem of a part comes back in one pass, in line order', () => {
    const body = '## Features\nstray\n### \n- **F1 A**: b\n- **F1 B**: c\n- **F0 Z**: d\n c\n#### deep\n'
    expect(parseFeatures(body).problems.map((problem) => problem.line)).toEqual([2, 3, 5, 6, 7, 8])
  })
})

describe('line numbers', () => {
  test('count from the line the body starts on, so they are lines of the file', () => {
    expect(problemsOf('## Features\nstray\n', 9)).toEqual([expect.stringMatching(/^10: /)])
    expect(problemsOf('\n\n## Features\n\nstray\n', 9)).toEqual([expect.stringMatching(/^13: /)])
  })
})
