import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { Markdown } from '../src/markdown/Markdown.tsx'
import { IdBook } from '../src/peek/book.ts'
import { IdBookContext } from '../src/peek/context.ts'
import { MAX_LOOKUPS, candidateWords, compilePatterns, splitIds } from '../src/peek/ids.ts'
import { peekWorld } from './support/changes-world.ts'
import { type FieldWorld, SCRATCH, drawerWorld } from './support/field-world.ts'

// Which words of a task's text become links to a peek (ts-0189-fg; note 34): only a word that matches a pattern one of
// the board's sources declares, and that `task.get` then finds. An id that is not found stays plain text.

const plan = peekWorld()
const patterns = compilePatterns(plan.sources)

function bookOf(world: FieldWorld): IdBook {
  return new IdBook(world.owner, 'b-alpha', compilePatterns(world.sources))
}

/** The text as Markdown draws it with the book, once the book has looked up each word the text could mean. */
async function render(book: IdBook, text: string): Promise<string> {
  await book.ensure(book.candidates(text))
  return renderToString(
    <IdBookContext value={book}>
      <Markdown text={text} />
    </IdBookContext>,
  )
}

/** The words drawn as id links, in order. */
const linksIn = (html: string) => [...html.matchAll(/<button[^>]*data-id="([^"]+)"/g)].map((each) => each[1]!)

describe('which words could be ids', () => {
  test('a word of a source pattern, however the sentence goes on', () => {
    expect(candidateWords('Do ts-0001-aa first, (ts-0002-bb), and ts-0003-cc.', patterns)).toEqual([
      'ts-0001-aa',
      'ts-0002-bb',
      'ts-0003-cc',
    ])
  })

  test('each word once, in the order they come', () => {
    expect(candidateWords('ts-0002-bb ts-0001-aa ts-0002-bb', patterns)).toEqual(['ts-0002-bb', 'ts-0001-aa'])
  })

  test('a word of the short pattern too: the number and the two letters', () => {
    expect(candidateWords('see 0189-fg and 12', patterns)).toEqual(['0189-fg', '12'])
  })

  test('not a word that is only a part of a longer one', () => {
    expect(candidateWords('ts-0001-aa-more and xts-0001-aa and ts-0001-aab', patterns)).toEqual([])
  })

  test('not a word in a path, a fragment or an address', () => {
    const text = 'src/ts-0001-aa.md and #ts-0002-bb and a.ts-0003-cc and ../ts-0004-dd'
    expect(candidateWords(text, patterns)).toEqual([])
  })

  test('not a word in another case, since the pattern says lower case', () => {
    expect(candidateWords('TS-0001-AA', patterns)).toEqual([])
  })

  test('at most MAX_LOOKUPS of them, so a long text cannot ask the daemon for each of its words', () => {
    const text = Array.from({ length: 5 * MAX_LOOKUPS }, (_, n) => `ts-${String(1000 + n)}-aa`).join(' ')
    const found = candidateWords(text, patterns)
    expect(found).toHaveLength(MAX_LOOKUPS)
    expect(found[0]).toBe('ts-1000-aa')
  })

  test('a source with no pattern, or one that is not a pattern, names no word', () => {
    const none = compilePatterns([{ id_pattern: '', short_id_pattern: '' }, { id_pattern: '(', short_id_pattern: '[' }])
    expect(none).toEqual([])
    expect(candidateWords('ts-0001-aa', none)).toEqual([])
  })

  test('the patterns of every source count, the id pattern and the short one', () => {
    const both = compilePatterns([
      { id_pattern: '^[a-z]{2}-[0-9]{4}-[a-z]{2}$', short_id_pattern: '^[0-9]+$' },
      { id_pattern: '^PROJ-[0-9]+$', short_id_pattern: '^[0-9]+$' },
    ])
    expect(candidateWords('PROJ-12 and ts-0001-aa and 7', both)).toEqual(['PROJ-12', 'ts-0001-aa', '7'])
  })
})

describe('splitting a text at the words that link', () => {
  test('gives the text whole when nothing links', () => {
    expect(splitIds('Start with ts-0002-bb.', new Set(), patterns)).toEqual(['Start with ts-0002-bb.'])
  })

  test('gives the text around a linked word, and the word as an id', () => {
    expect(splitIds('Start with ts-0002-bb, then ts-0003-cc.', new Set(['ts-0002-bb']), patterns)).toEqual([
      'Start with ',
      { id: 'ts-0002-bb' },
      ', then ts-0003-cc.',
    ])
  })

  test('a linked word at the start or the end of the text leaves no empty piece', () => {
    expect(splitIds('ts-0002-bb', new Set(['ts-0002-bb']), patterns)).toEqual([{ id: 'ts-0002-bb' }])
  })

  test('a linked word in a path is not split out', () => {
    expect(splitIds('src/ts-0002-bb.md', new Set(['ts-0002-bb']), patterns)).toEqual(['src/ts-0002-bb.md'])
  })
})

describe('Markdown with the board\'s ids', () => {
  test('links a word that matches a pattern and that task.get finds', async () => {
    const html = await render(bookOf(plan), 'Start with ts-0002-bb, which draws it.')
    expect(linksIn(html)).toEqual(['ts-0002-bb'])
    expect(html).toMatch(/Start with <button[^>]*>ts-0002-bb<\/button>, which draws it\./)
  })

  test('leaves a word of the pattern that no task has as plain text', async () => {
    const book = bookOf(plan)
    const html = await render(book, 'Not ts-9999-zz, but ts-0002-bb.')
    expect(linksIn(html)).toEqual(['ts-0002-bb'])
    expect(html).toContain('Not ts-9999-zz, but')
    expect(plan.callsTo('task.get')).toContainEqual({ board: 'b-alpha', id: 'ts-9999-zz' })
  })

  test('leaves a word that a source would match but that is part of a path, a fragment or a longer word', async () => {
    const html = await render(bookOf(plan), 'src/ts-0004-dd.md and #ts-0004-dd and ts-0004-dd-more')
    expect(linksIn(html)).toEqual([])
  })

  test('leaves a number of a version as text when no task has it', async () => {
    const html = await render(bookOf(plan), 'Needs version 1.2 of it.')
    expect(linksIn(html)).toEqual([])
    expect(html).toContain('Needs version 1.2 of it.')
  })

  test('links an inline code span that is only an id, and no other code', async () => {
    const html = await render(bookOf(plan), 'Run `ts-0003-cc` and `pnpm test ts-0003-cc`.')
    expect(html).toMatch(/<code[^>]*><button[^>]*data-id="ts-0003-cc"[^>]*>ts-0003-cc<\/button><\/code>/)
    expect(linksIn(html)).toEqual(['ts-0003-cc'])
    expect(html).toContain('<code>pnpm test ts-0003-cc</code>')
  })

  test('leaves an id in a fenced block as it is', async () => {
    const html = await render(bookOf(plan), '```\nts-0003-cc\n```')
    expect(linksIn(html)).toEqual([])
  })

  test('leaves an id in the text of a link as the text of that link, with no button inside a button', async () => {
    const html = await render(bookOf(plan), '[ts-0005-ee as link text](https://example.com/x)')
    expect(linksIn(html)).toEqual([])
    expect(html.match(/<button/g)).toHaveLength(1)
  })

  test('links an id in a list item, a table cell, a heading and emphasis', async () => {
    const text = '- ts-0001-aa\n\n| a |\n| - |\n| ts-0002-bb |\n\n## About ts-0003-cc\n\n**ts-0005-ee**'
    expect(linksIn(await render(bookOf(plan), text))).toEqual(['ts-0001-aa', 'ts-0002-bb', 'ts-0003-cc', 'ts-0005-ee'])
  })

  test('asks task.get once for an id, however often it is named, in one text or in several', async () => {
    const world = peekWorld()
    const book = bookOf(world)
    await render(book, 'ts-0001-aa and ts-0001-aa and ts-0002-bb')
    await render(book, 'ts-0001-aa again, and ts-0002-bb, and ts-0003-cc')
    const asked = (world.callsTo('task.get') as { id: string }[]).map((call) => call.id)
    expect([...asked].sort()).toEqual(['ts-0001-aa', 'ts-0002-bb', 'ts-0003-cc'])
  })

  test('asks for at most MAX_LOOKUPS words of a long text', async () => {
    const world = peekWorld()
    const text = Array.from({ length: 5 * MAX_LOOKUPS }, (_, n) => `ts-${String(1000 + n)}-aa`).join(' ')
    await render(bookOf(world), text)
    expect(world.callsTo('task.get')).toHaveLength(MAX_LOOKUPS)
  })

  test('asks nothing of a source whose patterns match no word of the text', async () => {
    const world = peekWorld()
    world.sources = [{ ...world.sources[0]!, id_pattern: '^NEVER$', short_id_pattern: '^NEVER$' }]
    await render(bookOf(world), 'ts-0001-aa')
    expect(world.callsTo('task.get')).toEqual([])
  })

  test('leaves a word as text when task.get fails for it', async () => {
    const world = peekWorld()
    world.refuseGet = 'the daemon is busy'
    const html = await render(bookOf(world), 'ts-0001-aa')
    expect(linksIn(html)).toEqual([])
    expect(html).toContain('ts-0001-aa')
  })

  test('leaves a short id that two tasks have as text, since the daemon names no one task for it', async () => {
    const world = drawerWorld()
    const html = await render(bookOf(world), 'ts-0009-ii and ts-0001-aa')
    expect(linksIn(html)).toEqual(['ts-0001-aa'])
  })

  test('links a word of a source of another shape: a number that a tracker has', async () => {
    const world = drawerWorld()
    expect(world.sources).toContain(SCRATCH)
    const html = await render(bookOf(world), 'Fix 12 and 1 first, not 77.')
    expect(linksIn(html)).toEqual(['12', '1'])
  })

  test('links nothing without a book, and the text is as it was', () => {
    const html = renderToString(<Markdown text="Start with ts-0002-bb." />)
    expect(linksIn(html)).toEqual([])
    expect(html).toContain('Start with ts-0002-bb.')
  })

  test('draws no id link as a link: no address, nothing to navigate to', async () => {
    const html = await render(bookOf(plan), 'ts-0001-aa')
    expect(html).not.toMatch(/<a[ >]/)
    expect(html).not.toContain('href=')
  })
})

describe('the book', () => {
  test('knows which words found a task, and tells a listener when that changes', async () => {
    const book = bookOf(peekWorld())
    const heard: number[] = []
    const stop = book.subscribe(() => heard.push(book.linked().size))
    await book.ensure(['ts-0001-aa', 'ts-9999-zz'])
    expect([...book.linked()]).toEqual(['ts-0001-aa'])
    expect(heard.at(-1)).toBe(1)
    stop()
    await book.ensure(['ts-0002-bb'])
    expect(heard.at(-1)).toBe(1)
  })

  test('keeps what task.get gave for a word, for the peek to show', async () => {
    const book = bookOf(peekWorld())
    await book.ensure(['ts-0002-bb', 'ts-9999-zz'])
    expect(book.get('ts-0002-bb')?.task?.title).toBe('Design the board')
    expect(book.get('ts-9999-zz')).toBeNull()
    expect(book.get('never-asked')).toBeUndefined()
  })

  test('gives the same set until a word is found, so a view of it does not draw again for nothing', async () => {
    const book = bookOf(peekWorld())
    await book.ensure(['ts-0001-aa'])
    const before = book.linked()
    await book.ensure(['ts-0001-aa', 'ts-9999-zz'])
    expect(book.linked()).toBe(before)
  })
})
