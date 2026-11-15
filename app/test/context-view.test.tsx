import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { CONTEXT_SECTIONS, ContextView, NOT_AVAILABLE } from '../src/components/queue/ContextView.tsx'
import type { ContextRecord } from '../src/components/types.ts'

// The context view is a data contract (0.11): the six sections are the fields of one record, and a section the data
// lacks reads "Not available in this view", never hidden. The agent's own words come last.

const filled = (text: string) => ({ status: 'filled' as const, text })

const FULL: ContextRecord = {
  why_it_asks: filled('WHY-TEXT'),
  what_it_would_do: filled('DO-TEXT'),
  task: filled('TASK-TEXT'),
  what_led_here: filled('LED-TEXT'),
  history: filled('HISTORY-TEXT'),
  agents_own_words: filled('WORDS-TEXT'),
}

const KEYS = Object.keys(FULL) as (keyof ContextRecord)[]
const TEXT: Record<keyof ContextRecord, string> = {
  why_it_asks: 'WHY-TEXT',
  what_it_would_do: 'DO-TEXT',
  task: 'TASK-TEXT',
  what_led_here: 'LED-TEXT',
  history: 'HISTORY-TEXT',
  agents_own_words: 'WORDS-TEXT',
}

/** What `renderToString` writes for a title: it escapes the apostrophe. */
const escaped = (text: string) => text.replaceAll("'", '&#x27;')
const count = (html: string, needle: string) => html.split(needle).length - 1

test('the message is the one the plan names', () => {
  expect(NOT_AVAILABLE).toBe('Not available in this view')
})

test('there are six sections and the agent\'s own words are last', () => {
  expect(CONTEXT_SECTIONS.map((s) => s.key)).toEqual([...KEYS])
  expect(CONTEXT_SECTIONS.at(-1)!.key).toBe('agents_own_words')
})

test('a full record shows all six sections and no gap', () => {
  const html = renderToString(<ContextView record={FULL} />)
  for (const text of Object.values(TEXT)) expect(html).toContain(text)
  expect(count(html, NOT_AVAILABLE)).toBe(0)
})

describe.each(KEYS)('with %s missing', (missing) => {
  test('marked not available, the section still shows and says so once', () => {
    const record: ContextRecord = { ...FULL, [missing]: { status: 'not_available' } }
    const html = renderToString(<ContextView record={record} />)
    expect(count(html, NOT_AVAILABLE)).toBe(1)
    expect(html).not.toContain(TEXT[missing])
    for (const key of KEYS.filter((k) => k !== missing)) expect(html).toContain(TEXT[key])
    const section = CONTEXT_SECTIONS.find((s) => s.key === missing)!
    expect(html).toContain(escaped(section.title))
  })

  test('left out of the record altogether, it reads the same', () => {
    const { [missing]: _left, ...rest } = FULL
    const html = renderToString(<ContextView record={rest} />)
    expect(count(html, NOT_AVAILABLE)).toBe(1)
    expect(html).toContain(escaped(CONTEXT_SECTIONS.find((s) => s.key === missing)!.title))
  })
})

test('an empty record shows all six as not available', () => {
  const html = renderToString(<ContextView record={{}} />)
  expect(count(html, NOT_AVAILABLE)).toBe(6)
})

test('the sections come in the plan\'s order', () => {
  const html = renderToString(<ContextView record={FULL} />)
  const at = Object.values(TEXT).map((text) => html.indexOf(text))
  expect(at).toEqual([...at].sort((a, b) => a - b))
})

test('the agent\'s words are marked unverified and shown as text', () => {
  const record: ContextRecord = { ...FULL, agents_own_words: filled('<b>trust me</b>') }
  const html = renderToString(<ContextView record={record} />)
  expect(html).toContain('Unverified')
  expect(html).not.toContain('<b>trust me')
})
