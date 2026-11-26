import { renderToString } from 'react-dom/server'
import { afterEach, describe, expect, test, vi } from 'vitest'
import {
  DEFAULTS_WORDS,
  DefaultsBanner,
  LEFT_LINES,
  ResetNotice,
  ResetWindow,
  movedLines,
} from '../src/defaults/DefaultsBanner.tsx'
import {
  type ClearableStorage,
  StateStoreProvider,
  clearStoredState,
  defaultsFromHash,
  hashForNormalStart,
  hashWithoutDefaults,
  hashWithoutReset,
  resetFolderFromHash,
  useKeptStore,
} from '../src/defaults/mode.ts'
import { toggleShut } from '../src/drawer/folds.ts'
import { readWidth, writeWidth } from '../src/drawer/width.ts'
import { setStripFolded } from '../src/epics/fold.ts'
import { setBarFolded } from '../src/filters/fold.ts'
import { type FoldStore, memoryStore, toggleFold } from '../src/field/folds.ts'
import { writeLayout } from '../src/field/layout.ts'

// Defaults mode (ts-0215-gt): the flag on the fragment, the store the window uses while it is on, the banner, the Reset
// window's list and the clearing of the stored state. The page's clicks and its reloads are in
// `browser/fake-defaults.spec.ts` and `browser/defaults.spec.ts`, which run the page.

const noop = () => {}
const text = (html: string) => html.replace(/<[^>]*>/g, ' ').replace(/&#x27;/g, "'").replace(/\s+/g, ' ')

/** A store that is also the storage of a webview: a map with `Storage`'s way to list its keys. */
class Storage implements FoldStore, ClearableStorage {
  readonly items = new Map<string, string>()
  get length() {
    return this.items.size
  }
  key(index: number) {
    return [...this.items.keys()][index] ?? null
  }
  getItem(key: string) {
    return this.items.get(key) ?? null
  }
  setItem(key: string, value: string) {
    this.items.set(key, value)
  }
  removeItem(key: string) {
    this.items.delete(key)
  }
}

describe('the flag on the fragment', () => {
  test('is the key `defaults`, with or without a value, beside the keys other pages keep there', () => {
    expect(defaultsFromHash('#defaults')).toBe(true)
    expect(defaultsFromHash('#defaults=')).toBe(true)
    expect(defaultsFromHash('#token=abc&defaults')).toBe(true)
    expect(defaultsFromHash('#world=two-boards&defaults=')).toBe(true)
  })

  test('is not there for a page that started normally', () => {
    for (const hash of ['', '#', '#token=abc', '#world=two-boards', '#defaultsx', '#x=defaults']) {
      expect(defaultsFromHash(hash), hash).toBe(false)
    }
  })

  test('comes off alone: the other keys stay', () => {
    expect(hashWithoutDefaults('#defaults')).toBe('')
    expect(hashWithoutDefaults('#token=abc&defaults=')).toBe('#token=abc')
    expect(hashWithoutDefaults('#token=abc')).toBe('#token=abc')
  })
})

describe('the fragment of the next start', () => {
  test('a normal start has no defaults flag, keeps the other keys, and carries the folder of a Reset', () => {
    expect(hashForNormalStart('#token=abc&defaults=', null)).toBe('#token=abc')
    expect(hashForNormalStart('#defaults=', null)).toBe('')
    const hash = hashForNormalStart('#token=abc&defaults=', '/h/.config/collide/set-aside/2026-11-26-101507')
    expect(defaultsFromHash(hash)).toBe(false)
    expect(new URLSearchParams(hash.slice(1)).get('token')).toBe('abc')
    expect(resetFolderFromHash(hash)).toBe('/h/.config/collide/set-aside/2026-11-26-101507')
  })

  test('a page that started with no Reset names no folder, and the folder comes off alone', () => {
    expect(resetFolderFromHash('')).toBeNull()
    expect(resetFolderFromHash('#reset=')).toBeNull()
    expect(hashWithoutReset('#token=abc&reset=%2Fx')).toBe('#token=abc')
    expect(hashWithoutReset('#reset=%2Fx')).toBe('')
  })
})

describe('the store of the window', () => {
  const kept = (given?: FoldStore) => {
    let store: FoldStore | null = null
    const Probe = () => {
      store = useKeptStore(given)
      return null
    }
    return { Probe, got: () => store! }
  }
  afterEach(() => vi.unstubAllGlobals())

  test('is the webview’s own storage when the window started normally', () => {
    const webview = new Storage()
    writeWidth(webview, 612)
    vi.stubGlobal('window', { localStorage: webview })
    const probe = kept()
    renderToString(<probe.Probe />)
    expect(readWidth(probe.got())).toBe(612)
  })

  test('in defaults mode ignores what the webview kept and writes nothing to it', () => {
    const webview = new Storage()
    writeWidth(webview, 612)
    toggleShut(webview, 'details')
    vi.stubGlobal('window', { localStorage: webview })
    const before = new Map(webview.items)
    const probe = kept()
    renderToString(
      <StateStoreProvider value={memoryStore()}>
        <probe.Probe />
      </StateStoreProvider>,
    )
    expect(readWidth(probe.got())).toBeNull()
    writeWidth(probe.got(), 700)
    expect(readWidth(probe.got())).toBe(700)
    expect(webview.items).toEqual(before)
  })

  test('is the store a test gives, whatever the mode', () => {
    const given = new Storage()
    const probe = kept(given)
    renderToString(
      <StateStoreProvider value={memoryStore()}>
        <probe.Probe />
      </StateStoreProvider>,
    )
    expect(probe.got()).toBe(given)
  })
})

describe('clearing the stored state', () => {
  test('removes every key the UI keeps, whichever part keeps it, and no other', () => {
    const webview = new Storage()
    const column = { key: 'open', label: 'Open', category: 'active' } as const
    toggleFold(webview, 'b1', 'mine', column as never)
    toggleShut(webview, 'details')
    writeWidth(webview, 500)
    writeLayout(webview, 'b1', null, 'graph')
    setBarFolded(webview, 'b1', true)
    setStripFolded(webview, 'b1', 'mine', true)
    webview.setItem('theme', 'dark')
    expect(webview.length).toBe(7)
    expect(clearStoredState(webview)).toBe(6)
    expect([...webview.items.keys()]).toEqual(['theme'])
  })

  test('is a count of zero for a storage with nothing of the UI’s', () => {
    expect(clearStoredState(new Storage())).toBe(0)
  })
})

describe('the banner', () => {
  const html = renderToString(<DefaultsBanner onNormal={noop} onReset={noop} />)

  test('says what the item says, and offers Start normally and Reset…', () => {
    expect(text(html)).toContain(DEFAULTS_WORDS)
    expect(DEFAULTS_WORDS).toBe(
      'Started with defaults. Your settings, views and packs are set aside for this window.',
    )
    expect(html).toMatch(/<button[^>]*>Start normally<\/button>/)
    expect(html).toMatch(/<button[^>]*>Reset…<\/button>/)
  })
})

describe('the Reset window', () => {
  const html = renderToString(<ResetWindow boardName="Octo" onConfirm={async () => null} onClosed={noop} />)

  test('lists the five things it moves', () => {
    const moves = text(html)
    expect(moves).toContain('config.yaml')
    expect(moves).toContain('packs.yaml')
    expect(moves).toContain('packs.lock')
    expect(moves).toContain('The folder of this board, Octo')
    expect(moves).toContain('stored state')
  })

  test('says what it leaves, and that nothing is deleted', () => {
    const words = text(html)
    for (const line of LEFT_LINES) expect(words).toContain(line)
    expect(words).toContain('other boards')
    expect(words).toContain('The repo')
    expect(words).toContain('keychain')
    expect(words).toContain('Nothing is deleted')
    expect(words).toContain('set-aside')
  })

  test('has Cancel and a button that moves them', () => {
    expect(html).toMatch(/<button[^>]*>Cancel<\/button>/)
    expect(html).toMatch(/<button[^>]*>Move them aside<\/button>/)
  })

  test('with no board open says there is no board folder to move', () => {
    expect(movedLines(null).join(' ')).toContain('No board is open')
  })
})

describe('the notice after a Reset', () => {
  test('names the folder the files went to', () => {
    const folder = '/home/me/.config/collide/set-aside/2026-11-26-101507'
    const html = renderToString(<ResetNotice folder={folder} onDismiss={noop} />)
    expect(text(html)).toContain(folder)
    expect(text(html)).toContain('Nothing was deleted')
  })
})
