import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { LinkWindow } from '../src/markdown/ExternalLinks.tsx'

// The window that asks before a link opens (ts-0183-ee; the mock's `extModal`), drawn to text. What a click, Cancel and
// Escape do is in `browser/fake-hostile.spec.ts`.

const noop = () => {}

function draw(url: string, over: { problem?: string | null; opening?: boolean } = {}) {
  return renderToString(
    <LinkWindow
      url={url}
      problem={over.problem ?? null}
      opening={over.opening ?? false}
      onOpen={noop}
      onCancel={noop}
    />,
  )
}

describe('the link window', () => {
  test('asks, names where the link leads and shows the whole address', () => {
    const html = draw('https://docs.example.com:8443/a/b?x=1&y=2#top')
    expect(html).toContain('Open this link in your browser?')
    expect(html).toContain('<p>It leads to <b>docs.example.com:8443</b>:</p>')
    expect(html).toContain('https://docs.example.com:8443/a/b?x=1&amp;y=2#top</p>')
  })

  test('has Cancel first and Open in browser after it', () => {
    const html = draw('https://example.com/')
    expect(html.indexOf('>Cancel<')).toBeGreaterThan(0)
    expect(html.indexOf('>Open in browser<')).toBeGreaterThan(html.indexOf('>Cancel<'))
  })

  test('says Collide does not open links by itself', () => {
    expect(draw('https://example.com/')).toContain('Collide does not open links by itself.')
  })

  test('a mail address has no host to name', () => {
    const html = draw('mailto:team@example.com')
    expect(html).not.toContain('It leads to')
    expect(html).toContain('mailto:team@example.com</p>')
  })

  test('says why a link could not open, as an alert', () => {
    const html = draw('https://example.com/', { problem: 'Collide could not open the link: no browser' })
    expect(html).toMatch(/<p role="alert"[^>]*>Collide could not open the link: no browser<\/p>/)
    expect(draw('https://example.com/')).not.toContain('role="alert"')
  })

  test('Open cannot be pressed twice while it opens', () => {
    const pressed = draw('https://example.com/', { opening: true })
    expect(pressed).toMatch(/<button[^>]*disabled=""[^>]*>Open in browser<\/button>/)
    expect(draw('https://example.com/')).not.toMatch(/disabled=""/)
  })

  test('is the browser dialog, named by its title, with no inline script or style', () => {
    const html = draw('https://example.com/')
    expect(html).toMatch(/<dialog[^>]*aria-labelledby="([^"]+)"/)
    const id = /aria-labelledby="([^"]+)"/.exec(html)![1]
    expect(html).toContain(`<h2 id="${id}"`)
    expect(html).not.toMatch(/<script|<style| style=|onclick=/)
  })
})
