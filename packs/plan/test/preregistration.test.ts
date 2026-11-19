import { createHash } from 'node:crypto'
import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { readPreregistration } from '../src/preregistration.ts'
import { call, EPIC_FILE, tempDir } from './helpers.ts'

type Row = Record<string, any>

const lines = (...rows: string[]): string => rows.join('\n')

/** The hash the plan describes: 16 lowercase hex characters of the SHA-256 of the original text. */
const hashOf = (text: string): string => createHash('sha256').update(text).digest('hex').slice(0, 16)

const ORIGINAL = lines(
  '## Pre-registration',
  '',
  'Hypothesis: A fresh session from a note is cheaper than a cold resume.',
  'Cheapest test: Run both once on the same transcript.',
  'Kill criterion: The fresh session costs more than the resume.',
  'Budget: $0.10',
  '',
  'Q1 predicts a gap of at least 20 percent.',
)

const AMENDMENT = lines('### Amendment 2026-11-20', '', 'The budget is now $0.20.')

const body = (...parts: string[]): string => `# Spike\n\n${parts.join('\n\n')}\n\n## Summary\n\nNothing yet.\n`

const read = (text: string, preregistered?: unknown, firstLine = 1) =>
  readPreregistration(text, firstLine, preregistered)

describe('the Pre-registration part gives its four texts', () => {
  test('a complete part: the four texts, no amendments, and no hashMatches without a frontmatter hash', () => {
    const { preregistration, problems } = read(body(ORIGINAL))
    expect(problems).toEqual([])
    expect(preregistration).toEqual({
      hypothesis: 'A fresh session from a note is cheaper than a cold resume.',
      cheapestTest: 'Run both once on the same transcript.',
      killCriterion: 'The fresh session costs more than the resume.',
      budget: '$0.10',
      amendments: 0,
    })
    expect(preregistration).not.toHaveProperty('hashMatches')
  })

  test('a label may be in any case, and the heading too, with trailing blanks', () => {
    const text = lines('## pre-registration  ', 'HYPOTHESIS: h', 'cheapest TEST: t', 'Kill Criterion: k', 'budget: b')
    const expected = { hypothesis: 'h', cheapestTest: 't', killCriterion: 'k', budget: 'b' }
    expect(read(text).preregistration).toMatchObject(expected)
    expect(read(text).problems).toEqual([])
  })

  test('a value runs over the lines that follow it, up to a blank line or the next label', () => {
    const text = lines(
      '## Pre-registration',
      'Hypothesis: first line',
      'second line',
      '',
      'Cheapest test: t',
      'Kill criterion:',
      '- stop if X',
      '- stop if Y',
      '',
      'Budget: b',
    )
    expect(read(text).preregistration).toMatchObject({
      hypothesis: 'first line second line',
      cheapestTest: 't',
      killCriterion: '- stop if X - stop if Y',
      budget: 'b',
    })
  })

  test('another word and a colon in prose is not a label and is not a problem', () => {
    const text = lines(
      '## Pre-registration',
      'Hypothesis: h',
      'Cheapest test: t',
      'Kill criterion: k',
      'Budget: b',
      '',
      'Stop rule: after two runs.',
      'Q1: the prediction.',
    )
    expect(read(text).problems).toEqual([])
  })

  test('a label inside a fenced block is not read, and a ## line inside one does not end the part', () => {
    const text = lines(
      '## Pre-registration',
      'Hypothesis: h',
      '',
      '```',
      'Kill criterion: not this one',
      '## Summary',
      '```',
      'Cheapest test: t',
      'Kill criterion: k',
      'Budget: b',
    )
    expect(read(text).preregistration).toMatchObject({ cheapestTest: 't', killCriterion: 'k' })
  })

  test('the labels of an amendment are not the original', () => {
    const text = lines(ORIGINAL, '', '### Amendment 2026-11-20', 'Budget: $9.99', 'Kill criterion: changed')
    expect(read(text).preregistration).toMatchObject({
      killCriterion: 'The fresh session costs more than the resume.',
      budget: '$0.10',
    })
  })
})

describe('problems', () => {
  test('a missing label is a problem on the heading, and its text is left out', () => {
    const text = lines('# S', '', '## Pre-registration', 'Hypothesis: h', 'Budget: b')
    const { preregistration, problems } = read(text)
    expect(problems).toEqual([
      { part: 'Pre-registration', line: 3, reason: expect.stringContaining('Cheapest test:') },
      { part: 'Pre-registration', line: 3, reason: expect.stringContaining('Kill criterion:') },
    ])
    expect(preregistration).toEqual({ hypothesis: 'h', budget: 'b', amendments: 0 })
  })

  test('a part with no label at all has four problems and still a preregistration', () => {
    const { preregistration, problems } = read('## Pre-registration\n\nJust prose.\n')
    expect(problems).toHaveLength(4)
    expect(preregistration).toEqual({ amendments: 0 })
  })

  test('an empty kill criterion is a problem on its line; an empty hypothesis is not', () => {
    const text = lines('## Pre-registration', 'Hypothesis:', 'Cheapest test: t', 'Kill criterion:', '', 'Budget: b')
    const { preregistration, problems } = read(text, undefined, 20)
    expect(problems).toEqual([
      { part: 'Pre-registration', line: 23, reason: expect.stringContaining('Kill criterion') },
    ])
    expect(preregistration).toMatchObject({ hypothesis: '', killCriterion: '' })
  })

  test('the lines are lines of the file', () => {
    const text = lines('x', '## Pre-registration', 'Hypothesis: h')
    expect(read(text, undefined, 10).problems.map((problem) => problem.line)).toEqual([11, 11, 11])
  })

  test('a second Pre-registration part is a problem on its heading and is not read', () => {
    const text = lines(ORIGINAL, '', '## Pre-registration', 'Budget: other')
    const { preregistration, problems } = read(text)
    expect(problems).toEqual([{ part: 'Pre-registration', line: 10, reason: expect.stringContaining('second') }])
    expect(preregistration?.budget).toBe('$0.10')
  })

  test('a record with no Pre-registration part has none and no problem, even with a frontmatter hash', () => {
    for (const hash of [undefined, 'abc', true]) {
      expect(read('# Spike\n\n## Summary\n\nText.\n', hash)).toEqual({ preregistration: null, problems: [] })
    }
    const fenced = lines('````', '## Pre-registration', 'Budget: b', '````')
    expect(read(fenced)).toEqual({ preregistration: null, problems: [] })
  })
})

describe('the hash covers the original text, and an amendment keeps it true', () => {
  /** The text the hash is over: the heading to the line before the first amendment, trailing blank lines dropped. */
  const original = ORIGINAL

  test('hashMatches is true when the frontmatter holds the hash of the original', () => {
    expect(read(body(ORIGINAL), hashOf(original)).preregistration?.hashMatches).toBe(true)
  })

  test('one amendment below keeps it true and is counted; two are counted', () => {
    const one = read(body(ORIGINAL, AMENDMENT), hashOf(original)).preregistration
    expect(one).toMatchObject({ amendments: 1, hashMatches: true })
    const two = read(body(ORIGINAL, AMENDMENT, lines('### Amendment 2026-11-22', '', 'Again.')), hashOf(original))
    expect(two.preregistration).toMatchObject({ amendments: 2, hashMatches: true })
    expect(two.problems).toEqual([])
  })

  test('a change to the original makes it false, and so does a change to its heading', () => {
    const edited = ORIGINAL.replace('$0.10', '$0.50')
    expect(read(body(edited, AMENDMENT), hashOf(original)).preregistration?.hashMatches).toBe(false)
    const renamed = ORIGINAL.replace('## Pre-registration', '## Pre-registration (draft)')
    expect(read(body(renamed), hashOf(original)).preregistration).toBeNull()
  })

  test('a hash that does not match is false, and so is one that is not text', () => {
    expect(read(body(ORIGINAL), '0123456789abcdef').preregistration?.hashMatches).toBe(false)
    expect(read(body(ORIGINAL), true).preregistration?.hashMatches).toBe(false)
    expect(read(body(ORIGINAL), ['a']).preregistration?.hashMatches).toBe(false)
    expect(read(body(ORIGINAL), hashOf(original).toUpperCase()).preregistration?.hashMatches).toBe(false)
  })

  test('the hash is 16 lowercase hex characters of the original text, in the first spec, so the test pins it', () => {
    expect(hashOf(original)).toMatch(/^[0-9a-f]{16}$/)
    expect(read(`${original}\n`, hashOf(original)).preregistration?.hashMatches).toBe(true)
  })

  test('blank lines after the original, before the next part or the amendment, are not part of it', () => {
    const text = `${ORIGINAL}\n\n\n\n## Summary\n`
    expect(read(text, hashOf(original)).preregistration?.hashMatches).toBe(true)
    const withBlanks = `${ORIGINAL}\n\n\n### Amendment 2026-11-20\nText.\n`
    expect(read(withBlanks, hashOf(original)).preregistration?.hashMatches).toBe(true)
  })

  test('what follows the next ## heading is not part of it', () => {
    const text = lines(ORIGINAL, '', '## Summary', '', 'Changed freely.')
    expect(read(text, hashOf(original)).preregistration?.hashMatches).toBe(true)
  })

  test('a ### heading before the first amendment is in the original; one after it is in the amendment', () => {
    const withSection = lines(ORIGINAL, '', '### Q1', '', 'More original text.')
    const text = lines(withSection, '', AMENDMENT, '', '### Q2', '', 'In the amendment.')
    expect(read(text, hashOf(withSection)).preregistration).toMatchObject({ hashMatches: true, amendments: 1 })
    expect(read(text, hashOf(original)).preregistration?.hashMatches).toBe(false)
  })

  test('an amendment heading in a fenced block is no amendment, and is part of the original', () => {
    const withFence = lines(ORIGINAL, '', '```', '### Amendment 2026-01-01', '```')
    const read1 = read(withFence, hashOf(withFence)).preregistration
    expect(read1).toMatchObject({ amendments: 0, hashMatches: true })
  })

  test('CRLF gives the same texts and the same hash as LF', () => {
    const text = body(ORIGINAL, AMENDMENT)
    const crlf = read(text.replaceAll('\n', '\r\n'), hashOf(original)).preregistration
    expect(crlf).toEqual(read(text, hashOf(original)).preregistration)
    expect(crlf?.hashMatches).toBe(true)
  })

  test('text before the part is not part of it', () => {
    const text = `Prose that changes.\n\n${ORIGINAL}\n`
    expect(read(text, hashOf(original)).preregistration?.hashMatches).toBe(true)
  })
})

// ---- through the provider -------------------------------------------------------------------------------------------

const ROOT = '.collide/planning'

const spike = (id: string, status: string, text: string, extra = ''): string =>
  `---\nid: ${id}\nkind: spike\nstatus: ${status}\ntitle: "Made-up"\nowners: []\nlinks: []\n${extra}---\n\n${text}`

function repoOf(files: Record<string, string>): string {
  const dir = tempDir()
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(dirname(join(dir, path)), { recursive: true })
    writeFileSync(join(dir, path), text)
  }
  return dir
}

const spikeAt = (id: string, text: string): Record<string, string> => ({
  [`${ROOT}/spikes/${id}-demo/record.md`]: text,
})

const tasksOf = async (root: string): Promise<Row[]> => ((await call(root, 'list')).result?.tasks ?? []) as Row[]

describe('a spike record over the provider', () => {
  test('list and get give the four texts, and an amendment keeps hashMatches true', async () => {
    const text = spike('sp-0001-aa', 'planned', body(ORIGINAL, AMENDMENT), `preregistered: "${hashOf(ORIGINAL)}"\n`)
    const root = repoOf(spikeAt('sp-0001-aa', text))
    const [listed] = await tasksOf(root)
    const got = (await call(root, 'get', { id: 'sp-0001-aa' })).result?.task
    expect(listed?.preregistration).toEqual({
      hypothesis: 'A fresh session from a note is cheaper than a cold resume.',
      cheapestTest: 'Run both once on the same transcript.',
      killCriterion: 'The fresh session costs more than the resume.',
      budget: '$0.10',
      amendments: 1,
      hashMatches: true,
    })
    expect(got?.preregistration).toEqual(listed?.preregistration)
    expect(listed).not.toHaveProperty('partProblems')
  })

  test('a change to the original after the hash makes hashMatches false, and it is no problem', async () => {
    const edited = body(ORIGINAL.replace('$0.10', '$5'))
    const text = spike('sp-0001-aa', 'planned', edited, `preregistered: "${hashOf(ORIGINAL)}"\n`)
    const [task] = await tasksOf(repoOf(spikeAt('sp-0001-aa', text)))
    expect(task?.preregistration.hashMatches).toBe(false)
    expect(task).not.toHaveProperty('partProblems')
  })

  test('a missing label is in partProblems under Pre-registration, on the heading line of the file', async () => {
    const text = spike('sp-0001-aa', 'planned', lines('# S', '', '## Pre-registration', 'Hypothesis: h'))
    const [task] = await tasksOf(repoOf(spikeAt('sp-0001-aa', text)))
    expect(task?.partProblems).toEqual([
      { part: 'Pre-registration', line: 12, reason: expect.stringContaining('Cheapest test:') },
      { part: 'Pre-registration', line: 12, reason: expect.stringContaining('Kill criterion:') },
      { part: 'Pre-registration', line: 12, reason: expect.stringContaining('Budget:') },
    ])
  })

  test('claim problems and Pre-registration problems are in line order together', async () => {
    const claim = lines('```claim', 'id: Q1.c1', '```')
    const parts = lines('# S', '', claim, '', '## Pre-registration', 'Hypothesis: h', '', claim)
    const text = spike('sp-0001-aa', 'planned', parts)
    const [task] = await tasksOf(repoOf(spikeAt('sp-0001-aa', text)))
    const found = task?.partProblems as Row[]
    const order = found.map((problem) => problem.line)
    expect(order).toEqual([...order].sort((a, b) => a - b))
    expect(new Set(found.map((problem) => problem.part))).toEqual(new Set(['claims', 'Pre-registration']))
  })

  test('a record with no Pre-registration part has no preregistration key and no problem', async () => {
    const text = spike('sp-0001-aa', 'concluded', '# S\n\n## Summary\n', 'preregistered: "0123456789abcdef"\n')
    const [task] = await tasksOf(repoOf(spikeAt('sp-0001-aa', text)))
    expect(task).not.toHaveProperty('preregistration')
    expect(task).not.toHaveProperty('partProblems')
  })

  test('a frontmatter hash that reads as a number is a task.invalid, as any number field is', async () => {
    const text = spike('sp-0001-aa', 'planned', body(ORIGINAL), 'preregistered: 1e10\n')
    const { result, invalid } = await call(repoOf(spikeAt('sp-0001-aa', text)), 'list')
    expect(result?.tasks).toEqual([])
    expect(invalid).toEqual([
      expect.objectContaining({ reason: expect.stringMatching(/field "preregistered".*not a number; quote/) }),
    ])
  })

  test('the same hash, quoted, is text that does not match', async () => {
    const text = spike('sp-0001-aa', 'planned', body(ORIGINAL), 'preregistered: "1e10"\n')
    const [task] = await tasksOf(repoOf(spikeAt('sp-0001-aa', text)))
    expect(task?.preregistration.hashMatches).toBe(false)
  })

  test('only a spike has the part: a proposal with a Pre-registration heading has no preregistration', async () => {
    const folder = `${ROOT}/epics/ep-001-aa-alpha`
    const head = 'id: pr-0001-aa\nkind: proposal\nstatus: draft\ntitle: "Made-up"\nowners: []\nlinks: []'
    const proposal = `---\n${head}\n---\n\n${ORIGINAL}\n`
    const root = repoOf({ [`${folder}/epic.md`]: EPIC_FILE, [`${folder}/proposals/pr-0001-aa-p.md`]: proposal })
    for (const task of await tasksOf(root)) {
      for (const key of ['preregistration', 'partProblems']) expect(task, task.id).not.toHaveProperty(key)
    }
  })
})
