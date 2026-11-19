import { mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { call, connect, EPIC_FILE, fileVersion, makeRepo, spawnProvider, tempDir } from './helpers.ts'

const EPICS = '.collide/planning/epics'
const FOLDER = 'ep-001-aa-alpha'

type Row = Record<string, any>

const proposalText = (id: string, status: string, body: string): string =>
  `---\nid: ${id}\nkind: proposal\nstatus: ${status}\ntitle: "Made-up proposal"\nowners: []\nlinks: []\n---\n\n${body}`
const crlf = (text: string): string => text.replace(/\n/g, '\r\n')

const fileOf = (id: string): string => `${FOLDER}/proposals/${id}-made-up.md`
const repoOf = (proposals: Record<string, string>): string =>
  makeRepo({
    [`${FOLDER}/epic.md`]: EPIC_FILE,
    ...Object.fromEntries(Object.entries(proposals).map(([id, text]) => [fileOf(id), text])),
  })
const pathOf = (root: string, id: string): string => join(root, EPICS, fileOf(id))
const read = (root: string, id: string): string => readFileSync(pathOf(root, id), 'utf8')
/** The Features part and what follows it, as a file's body: the text a test compares. */
const bodyOf = (root: string, id: string): string => read(root, id).split('---\n\n').slice(1).join('---\n\n')

const send = (root: string, id: string, op: string, args: Row, extra: Row = {}) =>
  call(root, 'apply', { op, id, args, version: fileVersion(pathOf(root, id)), ...extra })
const add = (root: string, id: string, args: Row, extra: Row = {}) => send(root, id, 'addFeature', args, extra)
const move = (root: string, id: string, args: Row, extra: Row = {}) => send(root, id, 'moveFeature', args, extra)
const featuresOf = async (root: string, id: string): Promise<Row> =>
  (await call(root, 'get', { id })).result?.task as Row

const PACKS = '### Packs\n- **F1 Trust prompt**: a repo pack asks first.\n- **F2 Pack lock**: pins each pack.\n'
const ENTRY_F3 = '- **F3 Socket**: the daemon socket.\n'
const ENTRY_F1_LONG = '- **F1 Trust prompt**: a repo pack\n  asks first.\n'
const ENTRY_F2 = '- **F2 Pack lock**: pins each pack.\n'
/** A part with two sections, a continuation line, and a part after it. */
const TWO_SECTIONS = `## Features\n\n### Packs\n${ENTRY_F1_LONG}${ENTRY_F2}\n### Daemon\n${ENTRY_F3}\n## Decision\n`

describe('addFeature', () => {
  test('with an empty section writes under a new ### Unsorted at the end, with the next number', async () => {
    const body = `## Features\n\n${PACKS}\n## Decision\n`
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', body) })
    const reply = await add(root, 'pr-0001-aa', { section: '', title: 'Menu badge', summary: 'the Queue count.' })
    expect(reply.result).toMatchObject({ outcome: 'ok', files: [`${EPICS}/${fileOf('pr-0001-aa')}`] })
    expect(bodyOf(root, 'pr-0001-aa')).toBe(
      `## Features\n\n${PACKS}\n### Unsorted\n- **F3 Menu badge**: the Queue count.\n\n## Decision\n`,
    )
    const task = await featuresOf(root, 'pr-0001-aa')
    expect(task.features.at(-1)).toEqual({ id: 'F3', title: 'Menu badge', section: '', summary: 'the Queue count.' })
    expect(task).not.toHaveProperty('featureProblems')
  })

  test('the reply is the task as read back, its version is the file, and the frontmatter is not touched', async () => {
    const text = proposalText('pr-0001-aa', 'draft', `## Features\n\n${PACKS}`)
    const root = repoOf({ 'pr-0001-aa': text })
    const reply = await add(root, 'pr-0001-aa', { section: 'Packs', title: 'A', summary: 'b' })
    expect(reply.result).toMatchObject({ outcome: 'ok', version: fileVersion(pathOf(root, 'pr-0001-aa')) })
    expect(reply.result?.task.features.map((feature: Row) => feature.id)).toEqual(['F1', 'F2', 'F3'])
    expect(reply.result?.items).toEqual([
      {
        key: 'plan:pr-0001-aa',
        version: reply.result?.version,
        from: { status: 'draft', category: 'active' },
        to: { status: 'draft', category: 'active' },
      },
    ])
    expect(read(root, 'pr-0001-aa').startsWith(text.slice(0, text.indexOf('## Features')))).toBe(true)
  })

  test('a second one with an empty section appends under the ### Unsorted the first made', async () => {
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', `## Features\n\n${PACKS}`) })
    await add(root, 'pr-0001-aa', { section: '', title: 'One', summary: 'a' })
    await add(root, 'pr-0001-aa', { section: '', title: 'Two', summary: 'b' })
    expect(bodyOf(root, 'pr-0001-aa')).toBe(
      `## Features\n\n${PACKS}\n### Unsorted\n- **F3 One**: a\n- **F4 Two**: b\n`,
    )
  })

  test('into an existing section appends under it, matched after trimming and in any case', async () => {
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', TWO_SECTIONS) })
    await add(root, 'pr-0001-aa', { section: '  PACKS ', title: 'Pack sync', summary: 'keeps them in step.' })
    expect(bodyOf(root, 'pr-0001-aa')).toBe(
      `## Features\n\n### Packs\n${ENTRY_F1_LONG}${ENTRY_F2}- **F4 Pack sync**: keeps them in step.\n\n` +
        `### Daemon\n${ENTRY_F3}\n## Decision\n`,
    )
  })

  test('takes one past the highest number in the file, so a gap is not filled and no number is reused', async () => {
    const body = '## Features\n\n### Packs\n- **F1 One**: a\n- **F5 Five**: b\n- **F3 Three**: c\n'
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', body) })
    await add(root, 'pr-0001-aa', { section: 'Packs', title: 'Next', summary: 'd' })
    const task = await featuresOf(root, 'pr-0001-aa')
    expect(task.features.map((feature: Row) => feature.id)).toEqual(['F1', 'F5', 'F3', 'F6'])
  })

  test('an Unsorted heading in another case is the one used, for an empty section and the word', async () => {
    const body = '## Features\n\n### UNSORTED\n- **F1 One**: a\n\n### Packs\n- **F2 Two**: b\n'
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', body) })
    await add(root, 'pr-0001-aa', { section: '', title: 'Three', summary: 'c' })
    await add(root, 'pr-0001-aa', { section: 'unSorted', title: 'Four', summary: 'd' })
    expect(bodyOf(root, 'pr-0001-aa')).toBe(
      '## Features\n\n### UNSORTED\n- **F1 One**: a\n- **F3 Three**: c\n- **F4 Four**: d\n\n' +
        '### Packs\n- **F2 Two**: b\n',
    )
    const task = await featuresOf(root, 'pr-0001-aa')
    expect(task.features.map((feature: Row) => [feature.id, feature.section])).toEqual([
      ['F1', ''], ['F3', ''], ['F4', ''], ['F2', 'Packs'],
    ])
  })

  test('a part with entries and no heading gets a ### Unsorted at its end', async () => {
    const body = '## Features\n\n- **F1 One**: above any heading\n\n## Decision\n'
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', body) })
    await add(root, 'pr-0001-aa', { section: '', title: 'Two', summary: 'b' })
    expect(bodyOf(root, 'pr-0001-aa')).toBe(
      '## Features\n\n- **F1 One**: above any heading\n\n### Unsorted\n- **F2 Two**: b\n\n## Decision\n',
    )
  })

  test('an empty part, as a new proposal has it, gets the heading and F1', async () => {
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', '## Features\n\n## Decision\n') })
    await add(root, 'pr-0001-aa', { section: 'New section', title: 'First', summary: 'a' })
    expect(bodyOf(root, 'pr-0001-aa')).toBe('## Features\n\n### New section\n- **F1 First**: a\n\n## Decision\n')
  })

  test('a part at the end of a file with no final line end gets the entry and a final line end', async () => {
    const body = '## Features\n\n### Packs\n- **F1 A**: b'
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', body) })
    await add(root, 'pr-0001-aa', { section: 'Packs', title: 'C', summary: 'd' })
    expect(bodyOf(root, 'pr-0001-aa')).toBe('## Features\n\n### Packs\n- **F1 A**: b\n- **F2 C**: d\n')
  })

  test('a file with CRLF line ends keeps them on every line it writes', async () => {
    const root = repoOf({ 'pr-0001-aa': crlf(proposalText('pr-0001-aa', 'draft', TWO_SECTIONS)) })
    await add(root, 'pr-0001-aa', { section: '', title: 'Menu', summary: 'one\ntwo' })
    await add(root, 'pr-0001-aa', { section: 'Daemon', title: 'Pipe', summary: 'three' })
    const text = read(root, 'pr-0001-aa')
    expect(text.replace(/\r\n/g, '')).not.toMatch(/[\r\n]/)
    expect(text).toContain('### Daemon\r\n- **F3 Socket**: the daemon socket.\r\n- **F5 Pipe**: three\r\n\r\n')
    expect(text).toContain('### Unsorted\r\n- **F4 Menu**: one\r\n  two\r\n\r\n## Decision\r\n')
    const { features } = await featuresOf(root, 'pr-0001-aa')
    expect(features.find((feature: Row) => feature.id === 'F5').summary).toBe('three')
  })

  test('a summary of several lines is written as continuation lines and reads back as one', async () => {
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', '## Features\n\n### Packs\n') })
    await add(root, 'pr-0001-aa', { section: 'Packs', title: 'Long', summary: '  first\r\n   second line \n- third  ' })
    expect(bodyOf(root, 'pr-0001-aa')).toBe(
      '## Features\n\n### Packs\n- **F1 Long**: first\n  second line\n  - third\n',
    )
    expect((await featuresOf(root, 'pr-0001-aa')).features[0].summary).toBe('first second line - third')
  })

  test('an agent may add a feature, and an item of any status takes one', async () => {
    const actor = { kind: 'agent', at: '2026-06-15', run: 'R-7' }
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'rejected', '## Features\n\n') })
    const reply = await add(root, 'pr-0001-aa', { section: '', title: 'A', summary: 'b' }, { actor })
    expect(reply.result?.outcome).toBe('ok')
    expect(bodyOf(root, 'pr-0001-aa')).toContain('- **F1 A**: b')
  })

  test.each([
    ['a section with a #', { section: 'Pack#s' }, 'a section name has no #'],
    ['a section over 40 characters', { section: 'x'.repeat(41) }, 'a section name is at most 40 characters'],
    ['a section with a line break', { section: 'Pa\ncks' }, 'a section name has no line break'],
    ['no section', { section: undefined }, 'section is required'],
    ['a section that is not text', { section: 4 }, 'section must be text'],
    ['a title with **', { title: 'Bold **title**' }, 'title has no **'],
    ['a title with a line break', { title: 'Two\nlines' }, 'title is one line'],
    ['a title that is blank', { title: '   ' }, 'title must be text of 1 to 200 characters'],
    ['a title over 200 characters', { title: 'x'.repeat(201) }, 'title must be text of 1 to 200 characters'],
    ['a summary that is blank', { summary: ' \n ' }, 'summary must be text'],
    ['a summary with a blank line inside', { summary: 'one\n\ntwo' }, 'summary has no blank line'],
    ['a summary with a control character', { summary: 'one\u0007two' }, 'summary has no control character'],
    ['an argument the op does not have', { extra: 'x' }, 'unknown argument "extra"'],
  ])('refuses %s, naming the argument, and writes nothing', async (_name, change, message) => {
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', `## Features\n\n${PACKS}`) })
    const before = read(root, 'pr-0001-aa')
    const args = { section: 'Packs', title: 'A title', summary: 'a summary', ...change }
    const reply = await add(root, 'pr-0001-aa', args)
    expect(reply.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining(message) })
    expect(read(root, 'pr-0001-aa')).toBe(before)
  })

  test('a summary line that would open a fenced block is refused with the problem, its line and the text', async () => {
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', `## Features\n\n${PACKS}`) })
    const before = read(root, 'pr-0001-aa')
    const reply = await add(root, 'pr-0001-aa', { section: 'Packs', title: 'Code', summary: 'see\n```sh' })
    expect(reply.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('line 16') })
    expect(reply.result?.message).toContain('fenced code block')
    expect(reply.result?.message).toContain('```sh')
    expect(read(root, 'pr-0001-aa')).toBe(before)
  })

  test('a part that has problems is refused with each one and its line, and is not written', async () => {
    const body = '## Features\n\nstray text\n### Packs\n- **F1 A**: b\n- **F1 B**: c\n'
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', body) })
    const before = read(root, 'pr-0001-aa')
    const reply = await add(root, 'pr-0001-aa', { section: 'Packs', title: 'C', summary: 'd' })
    expect(reply.result?.outcome).toBe('refused')
    expect(reply.result?.message).toContain('2 problems')
    expect(reply.result?.message).toContain('line 12: this line is neither an entry nor a continuation')
    expect(reply.result?.message).toContain('line 15: F1 is already used on line 14')
    expect(reply.result?.message).toContain('stray text')
    expect(read(root, 'pr-0001-aa')).toBe(before)
  })

  test('a stale version answers conflict with the current task and writes nothing', async () => {
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', `## Features\n\n${PACKS}`) })
    const stale = fileVersion(pathOf(root, 'pr-0001-aa'))
    writeFileSync(pathOf(root, 'pr-0001-aa'), `${read(root, 'pr-0001-aa')}\nA line a person added.\n`)
    const before = read(root, 'pr-0001-aa')
    const reply = await call(root, 'apply', {
      op: 'addFeature',
      id: 'pr-0001-aa',
      version: stale,
      args: { section: '', title: 'A', summary: 'b' },
    })
    expect(reply.result).toMatchObject({ outcome: 'conflict', current: { id: 'pr-0001-aa' } })
    expect(reply.result?.current.version).toBe(fileVersion(pathOf(root, 'pr-0001-aa')))
    expect(read(root, 'pr-0001-aa')).toBe(before)
  })

  test('an item that is not a proposal is refused, and a proposal with no Features part is refused', async () => {
    const task = proposalText('ts-0001-aa', 'ready', '## Features\n\n').replace('kind: proposal', 'kind: task')
    const root = makeRepo({
      [`${FOLDER}/epic.md`]: EPIC_FILE,
      [`${FOLDER}/tasks/ts-0001-aa-made-up.md`]: task,
      [fileOf('pr-0001-aa')]: proposalText('pr-0001-aa', 'draft', '## Context\n\nText.\n'),
    })
    const onTask = await call(root, 'apply', {
      op: 'addFeature',
      id: 'ts-0001-aa',
      version: fileVersion(join(root, EPICS, `${FOLDER}/tasks/ts-0001-aa-made-up.md`)),
      args: { section: '', title: 'A', summary: 'b' },
    })
    expect(onTask.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('is a task') })
    const none = await add(root, 'pr-0001-aa', { section: '', title: 'A', summary: 'b' })
    expect(none.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('no ## Features part') })
  })
})

describe('moveFeature', () => {
  test('moves the entry with its continuation lines and its number to the end of another section', async () => {
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', TWO_SECTIONS) })
    const reply = await move(root, 'pr-0001-aa', { feature: 'F1', section: ' daemon' })
    expect(reply.result).toMatchObject({ outcome: 'ok', files: [`${EPICS}/${fileOf('pr-0001-aa')}`] })
    expect(bodyOf(root, 'pr-0001-aa')).toBe(
      '## Features\n\n### Packs\n- **F2 Pack lock**: pins each pack.\n\n' +
        `### Daemon\n${ENTRY_F3}- **F1 Trust prompt**: a repo pack\n  asks first.\n\n## Decision\n`,
    )
    const task = await featuresOf(root, 'pr-0001-aa')
    expect(task.features.find((feature: Row) => feature.id === 'F1')).toEqual({
      id: 'F1',
      title: 'Trust prompt',
      section: 'Daemon',
      summary: 'a repo pack asks first.',
    })
    expect(task).not.toHaveProperty('featureProblems')
  })

  test('makes the heading at the end of the part when the section is missing', async () => {
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', TWO_SECTIONS) })
    await move(root, 'pr-0001-aa', { feature: 'F3', section: 'Sockets' })
    expect(bodyOf(root, 'pr-0001-aa')).toBe(
      `## Features\n\n### Packs\n${ENTRY_F1_LONG}${ENTRY_F2}\n### Daemon\n\n### Sockets\n${ENTRY_F3}\n## Decision\n`,
    )
  })

  test('to an empty section moves the entry under ### Unsorted, made when it is missing', async () => {
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', TWO_SECTIONS) })
    await move(root, 'pr-0001-aa', { feature: 'F2', section: '' })
    expect(bodyOf(root, 'pr-0001-aa')).toBe(
      '## Features\n\n### Packs\n- **F1 Trust prompt**: a repo pack\n  asks first.\n\n' +
        `### Daemon\n${ENTRY_F3}\n### Unsorted\n- **F2 Pack lock**: pins each pack.\n\n## Decision\n`,
    )
    expect((await featuresOf(root, 'pr-0001-aa')).features.find((feature: Row) => feature.id === 'F2').section).toBe('')
  })

  test('a feature already in the section is left alone: ok, no files, no items, the file unchanged', async () => {
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', TWO_SECTIONS) })
    const before = read(root, 'pr-0001-aa')
    const reply = await move(root, 'pr-0001-aa', { feature: 'F2', section: 'PACKS' })
    expect(reply.result).toMatchObject({ outcome: 'ok', files: [], items: [] })
    expect(read(root, 'pr-0001-aa')).toBe(before)
  })

  test('a file with CRLF line ends keeps them', async () => {
    const root = repoOf({ 'pr-0001-aa': crlf(proposalText('pr-0001-aa', 'draft', TWO_SECTIONS)) })
    await move(root, 'pr-0001-aa', { feature: 'F1', section: 'Daemon' })
    const text = read(root, 'pr-0001-aa')
    expect(text.replace(/\r\n/g, '')).not.toMatch(/[\r\n]/)
    expect(text).toContain('- **F1 Trust prompt**: a repo pack\r\n  asks first.\r\n\r\n## Decision\r\n')
    expect((await featuresOf(root, 'pr-0001-aa'))).not.toHaveProperty('featureProblems')
  })

  test.each([
    ['a feature the proposal does not have', { feature: 'F9' }, 'has no feature F9; its features are F1 (Trust'],
    ['a feature that is not F<n>', { feature: 'pr-0001-aa#F1' }, 'feature must be'],
    ['F0', { feature: 'F0' }, 'feature must be'],
    ['a section with a #', { section: 'a#b' }, 'a section name has no #'],
    ['a section over 40 characters', { section: 'x'.repeat(41) }, 'a section name is at most 40 characters'],
    ['no section', { section: undefined }, 'section is required'],
    ['an argument the op does not have', { to: 'x' }, 'unknown argument "to"'],
  ])('refuses %s and writes nothing', async (_name, change, message) => {
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', TWO_SECTIONS) })
    const before = read(root, 'pr-0001-aa')
    const reply = await move(root, 'pr-0001-aa', { feature: 'F1', section: 'Daemon', ...change })
    expect(reply.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining(message) })
    expect(read(root, 'pr-0001-aa')).toBe(before)
  })

  test('a part that has problems is refused with the problem and its line, and is not written', async () => {
    const body = '## Features\n\n#### Deep\n- **F1 A**: b\n'
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', body) })
    const before = read(root, 'pr-0001-aa')
    const reply = await move(root, 'pr-0001-aa', { feature: 'F1', section: 'Packs' })
    expect(reply.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('line 12') })
    expect(reply.result?.message).toContain('only ### section headings')
    expect(read(root, 'pr-0001-aa')).toBe(before)
  })

  test('a stale version answers conflict with the current task and writes nothing', async () => {
    const root = repoOf({ 'pr-0001-aa': proposalText('pr-0001-aa', 'draft', TWO_SECTIONS) })
    const stale = fileVersion(pathOf(root, 'pr-0001-aa'))
    writeFileSync(pathOf(root, 'pr-0001-aa'), `${read(root, 'pr-0001-aa')}\nA line a person added.\n`)
    const before = read(root, 'pr-0001-aa')
    const reply = await call(root, 'apply', {
      op: 'moveFeature',
      id: 'pr-0001-aa',
      version: stale,
      args: { feature: 'F1', section: 'Daemon' },
    })
    expect(reply.result).toMatchObject({ outcome: 'conflict', current: { id: 'pr-0001-aa' } })
    expect(read(root, 'pr-0001-aa')).toBe(before)
  })
})

describe('renameSection', () => {
  const rename = (root: string, args: Row, extra: Row = {}) =>
    call(root, 'apply', { op: 'renameSection', args, ...extra })

  /** The three that use `Pack`, in scan order. */
  const THREE = ['pr-0001-aa', 'pr-0002-bb', 'pr-0003-cc']

  /** Three proposals that use `Pack`, in three shapes, and a fourth that does not. */
  const FOUR = {
    'pr-0001-aa': proposalText(
      'pr-0001-aa',
      'draft',
      '## Features\n\n### Pack\n- **F1 One**: a\n\n### Daemon\n- **F2 Two**: b\n',
    ),
    'pr-0002-bb': proposalText(
      'pr-0002-bb',
      'accepted',
      '## Features\n\n### packs\n- **F1 One**: a\n\n### Pack\n- **F2 Two**: b\n\n### Other\n- **F3 Three**: c\n',
    ),
    'pr-0003-cc': proposalText(
      'pr-0003-cc',
      'rejected',
      '## Features\n\n### Pack\n- **F1 One**: a\n\n### packs\n- **F2 Two**: b\n',
    ),
    'pr-0004-dd': proposalText('pr-0004-dd', 'draft', '## Features\n\n### Daemon\n- **F1 One**: a\n'),
  }

  test('Pack to packs over three proposals changes the three files, merges, and leaves the fourth alone', async () => {
    const root = repoOf(FOUR)
    const fourth = readFileSync(pathOf(root, 'pr-0004-dd'))
    const reply = await rename(root, { from: 'Pack', to: 'packs' })
    const paths = THREE.map((id) => `${EPICS}/${fileOf(id)}`)
    expect(reply.result).toMatchObject({ outcome: 'ok', files: paths })
    expect(reply.result?.items.map((item: Row) => item.key)).toEqual([
      'plan:pr-0001-aa', 'plan:pr-0002-bb', 'plan:pr-0003-cc',
    ])
    for (const item of reply.result?.items as Row[]) {
      expect(item.version).toBe(fileVersion(pathOf(root, item.key.slice(5))))
    }
    expect(reply.result?.task.id).toBe('pr-0001-aa')
    expect(reply.result?.version).toBe(fileVersion(pathOf(root, 'pr-0001-aa')))
    expect(bodyOf(root, 'pr-0001-aa')).toBe(
      '## Features\n\n### packs\n- **F1 One**: a\n\n### Daemon\n- **F2 Two**: b\n',
    )
    expect(bodyOf(root, 'pr-0002-bb')).toBe(
      '## Features\n\n### packs\n- **F1 One**: a\n- **F2 Two**: b\n\n### Other\n- **F3 Three**: c\n',
    )
    expect(bodyOf(root, 'pr-0003-cc')).toBe('## Features\n\n### packs\n- **F2 Two**: b\n- **F1 One**: a\n')
    expect(readFileSync(pathOf(root, 'pr-0004-dd')).equals(fourth)).toBe(true)
    for (const id of THREE) {
      const task = await featuresOf(root, id)
      expect(task).not.toHaveProperty('featureProblems')
      const sections = task.features.map((feature: Row) => feature.section)
      expect(sections.filter((name: string) => /^pack/i.test(name))).not.toContain('Pack')
    }
  })

  test('touches a proposal of any status, and the frontmatter is not touched', async () => {
    const root = repoOf(FOUR)
    const before = read(root, 'pr-0003-cc')
    await rename(root, { from: 'pack', to: 'Libraries' })
    expect(before).toContain('status: rejected')
    expect(read(root, 'pr-0003-cc')).toContain('status: rejected\n')
    expect(bodyOf(root, 'pr-0003-cc')).toContain('### Libraries\n')
    expect(read(root, 'pr-0003-cc').split('---\n\n')[0]).toBe(before.split('---\n\n')[0])
  })

  test('a case change alone renames the heading, and headings that match each other merge', async () => {
    const root = repoOf({
      'pr-0001-aa': proposalText(
        'pr-0001-aa',
        'draft',
        '## Features\n\n### PACKS\n- **F1 A**: a\n\n### packs\n- **F2 B**: b\n',
      ),
    })
    await rename(root, { from: 'packs', to: 'Packs' })
    expect(bodyOf(root, 'pr-0001-aa')).toBe('## Features\n\n### Packs\n- **F1 A**: a\n- **F2 B**: b\n')
  })

  test('a section with no entry is renamed too, and a rename into Unsorted lands in the Unsorted heading', async () => {
    const root = repoOf({
      'pr-0001-aa': proposalText('pr-0001-aa', 'draft', '## Features\n\n### Misc\n\n### Unsorted\n- **F1 A**: a\n'),
    })
    await rename(root, { from: 'Misc', to: 'unsorted' })
    expect(bodyOf(root, 'pr-0001-aa')).toBe('## Features\n\n### Unsorted\n- **F1 A**: a\n')
  })

  test('a file with CRLF line ends keeps them', async () => {
    const root = repoOf({ 'pr-0001-aa': crlf(FOUR['pr-0002-bb'].replace('pr-0002-bb', 'pr-0001-aa')) })
    await rename(root, { from: 'Pack', to: 'packs' })
    const text = read(root, 'pr-0001-aa')
    expect(text.replace(/\r\n/g, '')).not.toMatch(/[\r\n]/)
    expect(text).toContain('### packs\r\n- **F1 One**: a\r\n- **F2 Two**: b\r\n\r\n### Other\r\n')
  })

  test('a stale version on any file answers conflict with that file\'s current task and writes none', async () => {
    const root = repoOf(FOUR)
    const versions = Object.fromEntries(THREE.map((id) => [id, fileVersion(pathOf(root, id))]))
    writeFileSync(pathOf(root, 'pr-0003-cc'), `${read(root, 'pr-0003-cc')}\nA line a person added.\n`)
    const before = THREE.map((id) => read(root, id))
    const reply = await rename(root, { from: 'Pack', to: 'packs', versions })
    expect(reply.result).toMatchObject({ outcome: 'conflict', current: { id: 'pr-0003-cc' } })
    expect(reply.result?.current.version).toBe(fileVersion(pathOf(root, 'pr-0003-cc')))
    expect(THREE.map((id) => read(root, id))).toEqual(before)
  })

  test('versions that are current go through, and one for a file the rename does not touch is ignored', async () => {
    const root = repoOf(FOUR)
    const versions = Object.fromEntries(Object.keys(FOUR).map((id) => [id, fileVersion(pathOf(root, id))]))
    const reply = await rename(root, { from: 'Pack', to: 'packs', versions })
    expect(reply.result?.outcome).toBe('ok')
    expect(reply.result?.files).toHaveLength(3)
  })

  test('a proposal that has the section and a problem in its part refuses the whole rename', async () => {
    const root = repoOf({
      ...FOUR,
      'pr-0002-bb': proposalText('pr-0002-bb', 'draft', '## Features\n\n### Pack\n- **F1 A**: a\n- **F1 B**: b\n'),
    })
    const before = Object.keys(FOUR).map((id) => read(root, id))
    const reply = await rename(root, { from: 'Pack', to: 'packs' })
    expect(reply.result?.outcome).toBe('refused')
    expect(reply.result?.message).toContain('pr-0002-bb')
    expect(reply.result?.message).toContain('line 14: F1 is already used on line 13')
    expect(Object.keys(FOUR).map((id) => read(root, id))).toEqual(before)
  })

  test('a proposal with a problem and no such section is not in the way', async () => {
    const root = repoOf({
      ...FOUR,
      'pr-0005-ee': proposalText('pr-0005-ee', 'draft', '## Features\n\n### Daemon\n- **F1 A**: a\n- **F1 B**: b\n'),
    })
    const reply = await rename(root, { from: 'Pack', to: 'packs' })
    expect(reply.result?.outcome).toBe('ok')
    expect(reply.result?.files).toHaveLength(3)
  })

  test('with no proposal that has the section it is refused and nothing is written', async () => {
    const root = repoOf(FOUR)
    const reply = await rename(root, { from: 'Nowhere', to: 'packs' })
    expect(reply.result).toMatchObject({
      outcome: 'refused',
      message: expect.stringContaining('no proposal has a section named'),
    })
  })

  test('running it again after it ran answers that nothing has the old name, and changes no file', async () => {
    const root = repoOf(FOUR)
    expect((await rename(root, { from: 'Pack', to: 'packs' })).result?.outcome).toBe('ok')
    const after = Object.keys(FOUR).map((id) => read(root, id))
    const again = await rename(root, { from: 'Pack', to: 'packs' })
    expect(again.result).toMatchObject({
      outcome: 'refused',
      message: expect.stringContaining('no proposal has a section'),
    })
    expect(Object.keys(FOUR).map((id) => read(root, id))).toEqual(after)
  })

  test.each([
    ['an id', { id: 'pr-0001-aa' }, 'renameSection works over every proposal: leave id out'],
    ['a version', { version: 'abc' }, 'renameSection takes no version'],
  ])('is refused with %s', async (_name, extra, message) => {
    const root = repoOf(FOUR)
    const reply = await rename(root, { from: 'Pack', to: 'packs' }, extra)
    expect(reply.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining(message) })
  })

  test.each([
    ['no from', { from: undefined }, 'from must be the name of a section'],
    ['a blank to', { to: '  ' }, 'to must be the name of a section'],
    ['a to with a #', { to: 'a#b' }, 'a section name has no #'],
    ['a to over 40 characters', { to: 'x'.repeat(41) }, 'a section name is at most 40 characters'],
    ['Unsorted as from', { from: 'unsorted' }, 'Unsorted is the part for features with no section'],
    ['the same name', { from: 'packs', to: ' packs ' }, 'from and to are the same name'],
    ['an argument the op does not have', { more: 1 }, 'unknown argument "more"'],
    ['versions that are not text', { versions: { 'pr-0001-aa': 3 } }, 'versions must be'],
    ['versions that are not a map', { versions: ['x'] }, 'versions must be'],
  ])('refuses %s', async (_name, change, message) => {
    const root = repoOf(FOUR)
    const before = Object.keys(FOUR).map((id) => read(root, id))
    const reply = await rename(root, { from: 'Pack', to: 'packs', ...change })
    expect(reply.result).toMatchObject({ outcome: 'refused', message: expect.stringContaining(message) })
    expect(Object.keys(FOUR).map((id) => read(root, id))).toEqual(before)
  })

  test('a writer killed between two files leaves each file whole, and running it again finishes', async () => {
    const root = repoOf(FOUR)
    const ids = THREE
    const before = ids.map((id) => read(root, id))
    const doomed = await spawnProvider(root, { COLLIDE_FAILPOINT: 'between-files' })
    await expect(doomed.request('apply', { op: 'renameSection', args: { from: 'Pack', to: 'packs' } })).rejects.toThrow(
      'before it answered',
    )
    expect(await doomed.exited).toEqual({ code: null, signal: 'SIGKILL' })

    // Scan order: the first file is renamed, the others are as they were, and every one of them reads.
    expect(read(root, ids[0]!)).not.toBe(before[0])
    expect(bodyOf(root, ids[0]!)).toContain('### packs\n')
    expect(read(root, ids[1]!)).toBe(before[1])
    expect(read(root, ids[2]!)).toBe(before[2])
    const dir = join(root, EPICS, FOLDER, 'proposals')
    expect(readdirSync(dir).filter((name) => name.endsWith('.collide-tmp'))).toEqual([])
    const next = await spawnProvider(root)
    const listed = (await next.request('list')) as Row
    expect(listed.invalid).toEqual([])
    for (const task of listed.tasks as Row[]) expect(task).not.toHaveProperty('featureProblems')

    const finished = await next.request('apply', { op: 'renameSection', args: { from: 'Pack', to: 'packs' } })
    await next.close()
    expect(finished.outcome).toBe('ok')
    expect(finished.files).toEqual([`${EPICS}/${fileOf(ids[1]!)}`, `${EPICS}/${fileOf(ids[2]!)}`])
    for (const id of ids) expect(bodyOf(root, id)).not.toMatch(/### Pack\n/)
    expect(bodyOf(root, ids[1]!)).toBe(
      '## Features\n\n### packs\n- **F1 One**: a\n- **F2 Two**: b\n\n### Other\n- **F3 Three**: c\n',
    )
  }, 30_000)

  describe('with the private root', () => {
    const renameOp = (from: string, to: string) => ({ op: 'renameSection', args: { from, to } })
    const world = (shared: Record<string, string> = { 'pr-0001-aa': FOUR['pr-0001-aa'] }) => {
      const repo = repoOf(shared)
      const data = tempDir()
      const priv = join(data, 'planning', 'epics', FOLDER, 'proposals', 'pr-0009-zz-made-up.md')
      mkdirSync(dirname(priv), { recursive: true })
      writeFileSync(priv, proposalText('pr-0009-zz', 'draft', '## Features\n\n### Pack\n- **F1 Mine**: a\n'))
      return { repo, data, priv }
    }

    test('in the main checkout a private proposal is renamed too, and is in items but not in files', async () => {
      const { repo, data, priv } = world()
      const client = connect({ packData: data })
      try {
        await client.request('initialize', { config: {}, root: repo })
        const reply = (await client.request('apply', renameOp('Pack', 'packs'))).result
        expect(reply?.outcome).toBe('ok')
        expect(reply?.files).toEqual([`${EPICS}/${fileOf('pr-0001-aa')}`])
        expect(reply?.items.map((item: Row) => item.key)).toEqual(['plan:pr-0001-aa', 'plan:pr-0009-zz'])
        expect(readFileSync(priv, 'utf8')).toContain('### packs\n- **F1 Mine**: a\n')
      } finally {
        await client.close()
      }
    })

    test('with only a private proposal that has the section, files is empty and the rename is done', async () => {
      const { repo, data, priv } = world({})
      const client = connect({ packData: data })
      try {
        await client.request('initialize', { config: {}, root: repo })
        const reply = (await client.request('apply', renameOp('Pack', 'packs'))).result
        expect(reply).toMatchObject({ outcome: 'ok', files: [], items: [{ key: 'plan:pr-0009-zz' }] })
        const again = (await client.request('apply', renameOp('packs', 'Mine'))).result
        expect(again?.outcome).toBe('ok')
        expect(readFileSync(priv, 'utf8')).toContain('### Mine\n')
      } finally {
        await client.close()
      }
    })
  })
})

describe('initialize declares the feature ops with argument schemas', () => {
  const declared = async (): Promise<Row> => (await call(repoOf({}), 'initialize')).result as Row

  test('a proposal has addFeature and moveFeature after its transition ops, and no other kind has them', async () => {
    const kinds = (await declared()).kinds as Row[]
    const names = (kind: string): string[] | undefined =>
      kinds.find((found) => found.name === kind)?.ops?.map((op: Row) => op.name)
    expect(names('proposal')).toEqual(['setStatus', 'accept', 'defer', 'reject', 'addFeature', 'moveFeature'])
    expect(names('task')).toBeUndefined()
  })

  test('addFeature and moveFeature declare required arguments and no others', async () => {
    const proposal = ((await declared()).kinds as Row[]).find((kind) => kind.name === 'proposal') as Row
    const op = (name: string): Row => proposal.ops.find((found: Row) => found.name === name).args
    expect(op('addFeature')).toMatchObject({ required: ['section', 'title', 'summary'], additionalProperties: false })
    expect(op('addFeature').properties.section).toMatchObject({ type: 'string', maxLength: 40 })
    expect(op('moveFeature')).toMatchObject({ required: ['feature', 'section'], additionalProperties: false })
    expect(op('moveFeature').properties.feature).toMatchObject({ pattern: '^F[1-9][0-9]*$' })
  })

  test('renameSection is a source op, declared beside create, with from and to required', async () => {
    const ops = (await declared()).ops as Row[]
    expect(ops.map((op) => op.name)).toEqual([
      'setStatus', 'link', 'claim', 'unclaim', 'create', 'reid', 'renameSection',
    ])
    expect(ops.find((op) => op.name === 'renameSection')?.args).toMatchObject({
      required: ['from', 'to'],
      additionalProperties: false,
    })
  })
})
