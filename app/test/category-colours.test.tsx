import { readFileSync } from 'node:fs'
import { join } from 'node:path'
import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { TracksPreview } from '../src/frame/TracksPreview.tsx'

// The category colours of the board mock are tokens of `index.css` (Stage 6a, Category colours), and the frame's Tracks
// preview uses them in place of the neutral primary it was drawn with.

const css = readFileSync(join(import.meta.dirname, '..', 'src', 'index.css'), 'utf8')
const CATEGORIES = ['backlog', 'ready', 'active', 'review', 'done', 'dropped']

describe('the category colours', () => {
  test.each(CATEGORIES)('%s has a token, and a colour registered for utilities', (category) => {
    expect(css).toContain(`--c-${category}:`)
    expect(css).toContain(`--color-c-${category}: var(--c-${category})`)
  })

  test.each(['working', 'waiting', 'blocked', 'done'])('the state colour %s has a token for both themes', (state) => {
    expect(css.match(new RegExp(`--${state}:`, 'g'))).toHaveLength(2)
  })

  test('the Tracks preview draws its phases and its live dot with them', () => {
    const html = renderToString(<TracksPreview onOpenLoop={() => {}} />)
    expect(html).toContain('bg-c-done')
    expect(html).toContain('bg-c-active')
    expect(html).not.toMatch(/bg-primary\/(15|25)|ring-primary|bg-primary"/)
  })
})
