import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { BoardMenuBody } from '../src/frame/BoardMenu.tsx'
import { Frame, type FrameProps } from '../src/frame/Frame.tsx'
import { TracksPreview } from '../src/frame/TracksPreview.tsx'
import { actionForKey, tabOfAction } from '../src/frame/actions.ts'
import { TABS, type TabId, isTyping } from '../src/frame/tabs.ts'
import { missingBoard } from './support/loop-world.ts'
import { twoBoards } from './support/worlds.ts'

// The frame (ts-0179-ke) is a function of its props, so it renders for any state with no browser. Clicks, keys and the
// menu's popover need Chrome: `browser/fake-shell.spec.ts` drives them over the same worlds.

const noop = () => {}

async function frameProps(over: Partial<FrameProps> = {}): Promise<FrameProps> {
  const { boards } = await twoBoards().owner.rpc('board.list')
  return {
    tab: 'field',
    onTab: noop,
    boards,
    boardId: boards[0]!.id,
    folderProblems: [],
    onChooseBoard: noop,
    onOpenFolder: async () => {},
    onOpenLoop: noop,
    field: <p>the field page</p>,
    settings: () => <p>the settings page</p>,
    ...over,
  }
}

async function show(over: Partial<FrameProps> = {}): Promise<string> {
  return renderToString(<Frame {...await frameProps(over)} />)
}

describe('the five tabs', () => {
  test('are named in order, each with its subtitle', async () => {
    const html = await show()
    const escaped = (words: string) => words.replaceAll("'", '&#x27;')
    const places = TABS.flatMap((tab) => [
      html.indexOf(`>${tab.name}</span>`),
      html.indexOf(`>${escaped(tab.subtitle)}</small>`),
    ])
    expect(places).toHaveLength(10)
    expect(places.every((place) => place >= 0)).toBe(true)
    expect(places).toEqual([...places].sort((a, b) => a - b))
  })

  test('are the five of the window, in the order of the keys', () => {
    expect(TABS.map((tab) => tab.name)).toEqual(['Field', 'Tracks', 'Queue', 'Planning', 'Comms'])
    expect(TABS.map((tab) => tab.subtitle)).toEqual([
      'the whole board',
      "what's running now",
      'what needs you',
      'what to build next',
      'sessions with an agent',
    ])
  })

  test('mark only the shown one as selected', async () => {
    const html = await show({ tab: 'planning' })
    expect(html.match(/aria-selected="true"/g)).toHaveLength(1)
    expect(html).toMatch(/aria-selected="true"[^>]*><span[^>]*>Planning/)
  })
})

describe('keys', () => {
  test.each([
    ['1', 'field'],
    ['2', 'tracks'],
    ['3', 'queue'],
    ['4', 'planning'],
    ['5', 'comms'],
  ] as const)('%s shows %s', (key, tab) => {
    expect(tabOfAction(actionForKey(key, 'window'))).toBe(tab)
  })

  test.each(['0', '6', '12', 'a', '/', 'Enter', ''])('%j shows no tab', (key) => {
    expect(tabOfAction(actionForKey(key, 'window'))).toBeNull()
  })

  test.each([
    { tagName: 'INPUT' },
    { tagName: 'TEXTAREA' },
    { tagName: 'SELECT' },
    { tagName: 'DIV', isContentEditable: true },
  ])('a text box does not take them: %j', (target) => {
    expect(isTyping(target)).toBe(true)
  })

  test.each([null, { tagName: 'BUTTON' }, { tagName: 'BODY' }, { tagName: 'DIV', isContentEditable: false }])(
    'anything else lets them through: %j',
    (target) => {
      expect(isTyping(target)).toBe(false)
    },
  )
})

describe('the top bar', () => {
  test('the board menu button names the board and its default line', async () => {
    const html = await show()
    expect(html).toContain('alpha')
    expect(html).toContain('· main')
  })

  test('says Boards when no board is open', async () => {
    const html = await show({ boards: [], boardId: null })
    expect(html).toContain('>Boards<')
  })

  test('has a Settings button and no Desk or autopilot control', async () => {
    const html = await show()
    expect(html).toContain('Settings')
    expect(html).not.toContain('Desk')
    expect(html).not.toContain('utopilot')
  })

  test('has an empty place for the view picker, which a picker fills', async () => {
    expect(await show()).not.toContain('the view picker')
    expect(await show({ viewPicker: <b>the view picker</b> })).toContain('the view picker')
  })
})

describe('the board menu lists', () => {
  const body = async (over: Partial<Parameters<typeof BoardMenuBody>[0]> = {}) => {
    const { boards } = await twoBoards().owner.rpc('board.list')
    return renderToString(
      <BoardMenuBody
        boards={boards}
        boardId="b-beta"
        problems={[]}
        onChoose={noop}
        onOpenFolder={async () => {}}
        {...over}
      />,
    )
  }

  test('every board with its default line and its folder', async () => {
    const html = await body()
    expect(html).toContain('alpha')
    expect(html).toContain('beta')
    expect(html).toContain('/work/alpha')
    expect(html).toContain('/work/beta')
    expect(html).toMatch(/line <b>main<\/b>/)
    expect(html).toMatch(/line <b>develop<\/b>/)
  })

  test('the board in use as current', async () => {
    const html = await body()
    expect(html.match(/aria-current="true"/g)).toHaveLength(1)
    expect(html).toMatch(/aria-current="true"[^>]*>(?:(?!<\/button>)[^])*beta/)
  })

  test('a way to open a folder by its typed path', async () => {
    const html = await body()
    expect(html).toContain('Open a folder…')
    expect(html).toContain('aria-label="Folder path"')
  })

  test('why a folder was not opened', async () => {
    const html = await body({ problems: ['This folder is not a git repository.'] })
    expect(html).toContain('This folder is not a git repository.')
  })

  test('a board whose folder is gone, saying "folder not found" in its row and no other row', async () => {
    const { boards } = await twoBoards().owner.rpc('board.list')
    const html = await body({ boards: [boards[0]!, missingBoard('b-gone', '/work/gone')] })
    expect(html.match(/folder not found/g)).toHaveLength(1)
    expect(html).toMatch(/folder not found · <\/span>(?:(?!<\/button>)[^])*\/work\/gone/)
  })

  test('no folder picker where the page has none', async () => {
    expect(await body()).not.toContain('Choose a folder…')
    expect(await body({ pickFolder: null })).not.toContain('Choose a folder…')
  })

  test('a folder picker in the shell, beside the typed path', async () => {
    const html = await body({ pickFolder: async () => null })
    expect(html).toContain('Choose a folder…')
    expect(html).toContain('aria-label="Folder path"')
  })
})

describe('what each tab shows', () => {
  const shown = async (tab: TabId) => show({ tab })

  test('Field shows the page it is given, and Planning a placeholder', async () => {
    expect(await shown('field')).toContain('the field page')
    expect(await shown('planning')).toContain('What to build next')
  })

  test('Queue says what arrives, in which stage, and offers the Loop page', async () => {
    const html = await shown('queue')
    expect(html).toContain('Every decision waiting on you, longest wait first.')
    expect(html).toContain('It arrives in Stage 9a')
    expect(html).toContain('Open the Loop page')
  })

  test('Comms says what arrives and in which stage', async () => {
    const html = await shown('comms')
    expect(html).toContain('Sessions with an agent that are not a task or a plan.')
    expect(html).toContain('It arrives in Stage 9b')
  })

  test('Tracks shows the preview', async () => {
    const html = await shown('tracks')
    expect(html).toContain('A preview of Stage 9b.')
    expect(html).toContain('Open the Loop page')
  })
})

describe('the Tracks preview', () => {
  const html = renderToString(<TracksPreview onOpenLoop={noop} />)

  test('says it is a preview and that its runs are made up', () => {
    expect(html).toContain('A preview of Stage 9b.')
    expect(html).toContain('these runs are made up')
  })

  test('shows the made-up runs as rows of phases, each with its state in words', () => {
    expect(html).toContain('Run R-121')
    expect(html).toContain('Run R-119')
    expect(html).toContain('Build · 17 m so far')
    expect(html).toContain('Review · next')
    expect(html).toContain('Plan · 4 m')
  })

  test('has no control but the one that opens the Loop page', () => {
    expect(html.match(/<button/g)).toHaveLength(1)
    expect(html).toContain('Open the Loop page')
  })
})
