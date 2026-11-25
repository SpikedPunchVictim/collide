import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { LocateWindow } from '../src/frame/LocateWindow.tsx'
import { MissingBoard } from '../src/frame/MissingBoard.tsx'

// The page of a board whose folder is gone, and the window that locates it (ts-0191-nv; note 15). Both are functions of
// their props, so they render with no browser. Typing, the picker and the daemon's refusal need Chrome:
// `browser/fake-missing.spec.ts` drives them.

const noop = () => {}

describe('the missing page', () => {
  const html = renderToString(<MissingBoard name="alpha" folder="/work/old/alpha" onLocate={noop} />)

  test('says the folder is gone and where Collide looked', () => {
    expect(html).toContain('alpha: its folder is gone')
    expect(html).toContain('<code class="font-mono [overflow-wrap:anywhere]">/work/old/alpha</code>')
    expect(html).toContain('found nothing there. It was moved or deleted.')
  })

  test('says that nothing was deleted and that the tasks are kept', () => {
    expect(html).toContain('Nothing has been deleted.')
    expect(html).toContain('Its tasks are kept')
  })

  test('offers to locate the folder, and shows removing the board as off, saying when it arrives', () => {
    expect(html).toContain('Locate the folder…')
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*title="Arrives with board removal"[^>]*>Remove from Collide/)
  })
})

describe('the locate window', () => {
  const props = { boardName: 'alpha', onLocate: async () => ({ problem: null, note: null }), onClosed: noop }

  test('asks where the folder is now, for the board it names', () => {
    const html = renderToString(<LocateWindow {...props} />)
    expect(html).toContain('Locate alpha')
    expect(html).toContain('Where is the folder now?')
    expect(html).toContain('Use this folder')
    expect(html).toContain('Collide checks that it is the same repository.')
  })

  test('has the typed path and no picker button where there is no picker', () => {
    const html = renderToString(<LocateWindow {...props} />)
    expect(html).toContain('<input')
    expect(html).not.toContain('Choose a folder…')
  })

  test('has the picker button as well in the shell', () => {
    const html = renderToString(<LocateWindow {...props} pick={async () => null} />)
    expect(html).toContain('<input')
    expect(html).toContain('Choose a folder…')
  })

  test('starts with the folder unchosen, so Use this folder is off', () => {
    const html = renderToString(<LocateWindow {...props} />)
    expect(html).toMatch(/<button[^>]*disabled=""[^>]*>Use this folder/)
  })
})
