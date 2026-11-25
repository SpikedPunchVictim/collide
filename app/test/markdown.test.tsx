import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { Markdown } from '../src/markdown/Markdown.tsx'
import { openableLink, schemeOf } from '../src/markdown/links.ts'
import { ELSEWHERE, HOSTILE_BODY } from './support/hostile.ts'

// Task, issue and agent text is rendered to React elements and never to HTML strings (ts-0183-ee). These tests
// render to text, so what a browser would run or fetch is what the markup holds.

const render = (text: string) => renderToString(<Markdown text={text} />)

/** The real tags of rendered markup. Text is escaped, so `&lt;script&gt;` in the output is text and not a tag. */
function tagsOf(html: string): { name: string; attributes: string[] }[] {
  return [...html.matchAll(/<([a-z][a-z0-9]*)((?:\s+[^\s=>]+(?:="[^"]*")?)*)\s*\/?>/g)].map((match) => ({
    name: match[1]!,
    attributes: [...match[2]!.matchAll(/\s+([^\s=>]+)/g)].map((attribute) => attribute[1]!),
  }))
}

/** What markdown may produce: the elements of the text, and the one button of a kept link. */
const ALLOWED_TAGS = new Set([
  'div', 'p', 'h4', 'ul', 'ol', 'li', 'pre', 'code', 'strong', 'em', 'del', 'br', 'hr', 'blockquote', 'span', 'button',
  'table', 'thead', 'tbody', 'tr', 'th', 'td', 'input',
])

/**
 * The attributes an element may have. None of them loads, runs or navigates. A table's alignment is a `style`,
 * which React sets through the DOM in the browser, so the policy's `style-src 'self'` allows it.
 */
const ALLOWED_ATTRIBUTES = new Set(['class', 'type', 'title', 'checked', 'disabled', 'start', 'align', 'style'])

/** Fails when the markup holds an element or an attribute that could load, run or navigate. */
function expectOnlyInertMarkup(html: string): void {
  for (const { name, attributes } of tagsOf(html)) {
    expect(ALLOWED_TAGS.has(name), `<${name}> in ${html}`).toBe(true)
    for (const attribute of attributes) {
      expect(ALLOWED_ATTRIBUTES.has(attribute), `${attribute} on <${name}>`).toBe(true)
    }
  }
}

describe('what the markdown shows', () => {
  test('headings, with the page keeping its own outline', () => {
    const html = render('# One\n\n## Two\n\n###### Six')
    expect(html).toMatch(/<h4[^>]*>One<\/h4>/)
    expect(html).toMatch(/<h4[^>]*>Two<\/h4>/)
    expect(html).toMatch(/<h4[^>]*>Six<\/h4>/)
    expect(html).not.toMatch(/<h[1236]/)
  })

  test('bullet, numbered and task lists', () => {
    const html = render('- a\n- b\n\n1. first\n2. second\n\n- [x] done\n- [ ] todo')
    expect(html).toMatch(/<ul[^>]*>\s*<li[^>]*>a<\/li>\s*<li[^>]*>b<\/li>/)
    expect(html).toMatch(/<ol[^>]*>\s*<li[^>]*>first<\/li>\s*<li[^>]*>second<\/li>/)
    expect(html.match(/<input[^>]*type="checkbox"[^>]*>/g)).toHaveLength(2)
    expect(html.match(/<input[^>]*disabled/g)).toHaveLength(2)
  })

  test('inline code and a fenced block', () => {
    const html = render('Run `pnpm test` first.\n\n```ts\nconst x = 1 < 2\n```')
    expect(html).toMatch(/<code[^>]*>pnpm test<\/code>/)
    expect(html).toMatch(/<pre[^>]*><code[^>]*>const x = 1 &lt; 2\n<\/code><\/pre>/)
  })

  test('a table, with its alignment', () => {
    const html = render('| key | state |\n| :-- | --: |\n| ts-1 | ready |')
    expect(html).toMatch(/<table[^>]*>/)
    expect(html).toMatch(/<th[^>]*>key<\/th>/)
    expect(html).toMatch(/<td[^>]*>ready<\/td>/)
    expect(html).toContain('style="text-align:right"')
    expectOnlyInertMarkup(html)
  })

  test('emphasis and paragraphs', () => {
    const html = render('one **two** and *three*\n\nnext paragraph')
    expect(html).toMatch(/<strong>two<\/strong>/)
    expect(html).toMatch(/<em>three<\/em>/)
    expect(html.match(/<p[ >]/g)).toHaveLength(2)
  })
})

describe('what the markdown never does', () => {
  test('raw HTML is shown as text and nothing in it becomes an element', () => {
    const html = render(
      '<script>window.x = 1</script>\n\n<img src="x" onerror="window.x = 2">\n\n<a href="javascript:x">y</a>\n\n' +
        '<iframe src="https://example.com"></iframe>\n\n<div onclick="x()">z</div>',
    )
    expect(html).toContain('&lt;script&gt;window.x = 1&lt;/script&gt;')
    expect(html).toContain('&lt;iframe src=&quot;https://example.com&quot;&gt;&lt;/iframe&gt;')
    expectOnlyInertMarkup(html)
    expect(tagsOf(html).map((tag) => tag.name)).not.toContain('script')
  })

  test.each([
    ['javascript:alert(1)', 'javascript:'],
    ['JAVASCRIPT:alert(1)', 'javascript:'],
    ['file:///etc/passwd', 'file:'],
    ['data:text/html,hi', 'data:'],
    ['vbscript:x', 'vbscript:'],
    ['ftp://example.com/f', 'ftp:'],
    ['tel:+15550100', 'tel:'],
    ['blob:https://example.com/id', 'blob:'],
  ])('a link to %s shows as its text, with nothing to follow', (href, scheme) => {
    const html = render(`[the text](${href})`)
    expect(html).toContain('the text')
    expect(html).toContain(`(link not opened: ${scheme} links are never opened)`)
    expectOnlyInertMarkup(html)
    expect(tagsOf(html).map((tag) => tag.name)).not.toContain('a')
    expect(tagsOf(html).map((tag) => tag.name)).not.toContain('button')
    expect(html).not.toContain(href)
  })

  test('a link with no address of its own, or to the page itself, shows as its text', () => {
    for (const href of ['/settings', 'docs/readme.md', '//example.com/x', '?q=1']) {
      const html = render(`[the text](${href})`)
      expect(html, href).toContain('the text')
      expect(html, href).toContain('link not opened')
      expectOnlyInertMarkup(html)
      expect(tagsOf(html).map((tag) => tag.name), href).not.toContain('button')
    }
    // An anchor inside the text (a footnote's) is not a link to anywhere else.
    const anchor = render('[the text](#section)')
    expect(anchor).toContain('the text')
    expect(tagsOf(anchor).map((tag) => tag.name)).not.toContain('button')
    expect(anchor).not.toContain('link not opened')
  })

  test('an image shows as its alt text and its address, and is never an element', () => {
    const html = render(`![the chart](${ELSEWHERE}/c.png)`)
    expect(html).toContain(`[image not loaded: the chart, from ${ELSEWHERE}/c.png]`)
    expectOnlyInertMarkup(html)
    expect(tagsOf(html).map((tag) => tag.name)).not.toContain('img')
  })

  test('an image with no alt text says so, and an image in a link is still not loaded', () => {
    expect(render(`![](${ELSEWHERE}/c.png)`)).toContain('[image not loaded: no description, from')
    const html = render(`[![logo](${ELSEWHERE}/l.png)](https://example.com/)`)
    expectOnlyInertMarkup(html)
    expect(html).toContain('[image not loaded: logo, from')
  })

  test('the hostile body shows as text and loads and runs nothing', () => {
    const html = render(HOSTILE_BODY)
    expectOnlyInertMarkup(html)
    expect(html).toContain('the docs')
    expect(html).toContain('write to us')
    expect(html).toContain('script link')
    expect(html).toContain('file link')
    expect(html).toContain('data link')
  })
})

describe('the links that open', () => {
  test('a link to the web or to a mail address keeps its text and is a button, not an anchor', () => {
    const html = render('See [the docs](https://example.com/docs?a=1&b=2) or [mail us](mailto:a@example.com).')
    expect(html).toMatch(/<button[^>]*type="button"[^>]*>the docs<\/button>/)
    expect(html).toMatch(/<button[^>]*type="button"[^>]*>mail us<\/button>/)
    expectOnlyInertMarkup(html)
    expect(tagsOf(html).map((tag) => tag.name)).not.toContain('a')
    expect(html).toContain('title="https://example.com/docs?a=1&amp;b=2"')
  })

  test('the markup in a link keeps its shape', () => {
    const html = render('[**bold** text](https://example.com/)')
    expect(html).toMatch(/<button[^>]*><strong>bold<\/strong> text<\/button>/)
  })
})

describe('which addresses may open', () => {
  test.each([
    ['https://example.com/a?b=1#c', 'https://example.com/a?b=1#c'],
    ['http://example.com', 'http://example.com/'],
    ['HTTPS://EXAMPLE.COM/Path', 'https://example.com/Path'],
    ['mailto:person@example.com', 'mailto:person@example.com'],
    ['  https://example.com/x ', 'https://example.com/x'],
    ['https://example.com/a b', 'https://example.com/a%20b'],
  ])('%s opens as %s', (raw, opened) => {
    expect(openableLink(raw)).toBe(opened)
  })

  test.each([
    'javascript:alert(1)',
    'JaVaScRiPt:alert(1)',
    'java\nscript:alert(1)',
    ' \tjavascript:alert(1)',
    'file:///etc/passwd',
    'data:text/html,x',
    'ftp://example.com',
    '//example.com',
    '/path',
    'example.com',
    'https://',
    'https:',
    'mailto:',
    'https://exa mple.com',
    '',
  ])('%j does not', (raw) => {
    expect(openableLink(raw)).toBeNull()
  })

  test('nothing is said of an address that is not there', () => {
    expect(openableLink(undefined)).toBeNull()
  })

  test('the scheme a refused link is named by', () => {
    expect(schemeOf('JavaScript:alert(1)')).toBe('javascript:')
    expect(schemeOf('java\nscript:alert(1)')).toBe('javascript:')
    expect(schemeOf('/path')).toBeNull()
    expect(schemeOf(undefined)).toBeNull()
  })
})
