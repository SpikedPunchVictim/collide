import { describe, expect, test } from 'vitest'
import { PLAN, TRACKER } from './support/field-world.ts'
import { mockListing, pack } from './support/settings-world.ts'
import { files, general, nameOf, packRows, secrets, sourceOf, statusesWord, valueText } from '../src/settings/model.ts'

// What the Settings page shows, worked out from the daemon's answers (ts-0193-gk). The page's text is drawn in
// `settings-page.test.tsx`; the clicks and windows are in `browser/fake-settings.spec.ts`.

describe('the General rows', () => {
  test('hold every setting that is no secret, each with its key and its layer as a badge word', () => {
    const { rows } = general(mockListing())
    expect(rows.map((row) => [row.key, row.layerWord])).toEqual([
      ['ask.agent', 'Default'],
      ['ask.timeout_s', 'You'],
      ['ask.budget_usd', 'This repo'],
      ['disk.floor_gib', 'Default'],
      ['claude.check_published', 'Default'],
    ])
  })

  test('show a value the repo overrode struck out, with whose it was and who overrode it', () => {
    const row = general(mockListing()).rows.find((each) => each.key === 'ask.budget_usd')!
    expect(row.value).toBe('$0.25')
    expect(row.struck).toEqual({ value: '$0.50', words: 'yours, overridden by this repo' })
  })

  test('strike nothing for a setting no other layer set', () => {
    const rows = general(mockListing()).rows.filter((row) => row.key !== 'ask.budget_usd')
    expect(rows.map((row) => row.struck)).toEqual([null, null, null, null])
  })

  test('say whose a repo value was when the user overrode it', () => {
    const listing = mockListing()
    listing.entries[3] = {
      key: 'disk.floor_gib',
      value: 2,
      layer: 'user',
      secret: false,
      overridden: { value: 9, layer: 'repo' },
    }
    const row = general(listing).rows.find((each) => each.key === 'disk.floor_gib')!
    expect(row.struck).toEqual({ value: '9 GiB', words: "this repo's, overridden by you" })
  })

  test('put a problem on the row of its key, in the daemon\'s words', () => {
    const listing = mockListing()
    listing.problems = [{ key: 'ask.timeout_s', layer: 'user', message: 'must be at least 10' }]
    const { rows, loose } = general(listing)
    expect(rows.find((row) => row.key === 'ask.timeout_s')!.problems).toEqual(['must be at least 10'])
    expect(rows.filter((row) => row.problems.length > 0)).toHaveLength(1)
    expect(loose).toEqual([])
  })

  test('keep a problem that names no row apart', () => {
    const listing = mockListing()
    listing.problems = [{ key: 'ferry', layer: 'repo', message: 'a name Collide uses' }]
    const { rows, loose } = general(listing)
    expect(rows.every((row) => row.problems.length === 0)).toBe(true)
    expect(loose).toEqual([{ key: 'ferry', layer: 'repo', message: 'a name Collide uses' }])
  })
})

describe('values', () => {
  test.each([
    ['disk.floor_gib', 5, '5 GiB'],
    ['ask.timeout_s', 120, '120 s'],
    ['ask.budget_usd', 0.5, '$0.50'],
    ['claude.check_published', true, 'On, daily and at start'],
    ['claude.check_published', false, 'Off'],
    ['ask.agent', 'claude', 'claude'],
    ['ferry.lanePool', 3, '3'],
    ['ferry.greeting', 'hello', 'hello'],
    ['ferry.verbose', true, 'On'],
    ['ask.budget_usd', null, 'No value'],
    ['ask.budget_usd', undefined, 'No value'],
  ])('%s %j shows as %j', (key, value, shown) => {
    expect(valueText(key, value)).toBe(shown)
  })

  test('a key of a pack is its own name, and one of Collide\'s own has the mock\'s', () => {
    expect(nameOf('ferry.lanePool')).toBe('ferry.lanePool')
    expect(nameOf('disk.floor_gib')).toBe('Free disk space to keep')
  })
})

describe('the secrets', () => {
  test('are the entries marked secret, each with whether the keychain holds it', () => {
    expect(secrets(mockListing()).map((row) => [row.key, row.pack, row.name, row.set])).toEqual([
      ['notify.webhook', 'notify', 'webhook', false],
      ['gateway.apiKey', 'gateway', 'apiKey', true],
    ])
  })

  test('are not known when the daemon could not say', () => {
    const listing = mockListing()
    listing.entries = listing.entries.map((entry) => ({ ...entry, is_set: null }))
    expect(secrets(listing).map((row) => row.set)).toEqual([null, null])
  })

  test('have no value to show: the row holds none', () => {
    const [row] = secrets(mockListing())
    expect(Object.keys(row!).sort()).toEqual(['key', 'name', 'pack', 'problems', 'set'])
  })
})

describe('the packs', () => {
  const packs = [
    pack('collide/plan', 'builtin', 'builtin'),
    pack('demo/tracker', 'repo:.collide/packs/tracker', 'repo'),
    pack('tidy', 'dev:/work/tools/tidy', 'user', { enabled: false, dev: true }),
  ]

  test('each say where it is from, its source and the statuses the board\'s sources of it declare', () => {
    expect(packRows(packs, [PLAN, TRACKER])).toEqual([
      { name: 'collide/plan', from: 'Built in', source: null, statuses: PLAN.statuses.length, enabled: true },
      {
        name: 'demo/tracker',
        from: 'Brought by this repo',
        source: 'repo:.collide/packs/tracker',
        statuses: TRACKER.statuses.length,
        enabled: true,
      },
      { name: 'tidy', from: 'Added by you', source: 'dev:/work/tools/tidy', statuses: null, enabled: false },
    ])
  })

  test('add the statuses of every source a pack declares', () => {
    const second = { ...TRACKER, name: 'tracker-2' }
    const [, tracker] = packRows(packs, [PLAN, TRACKER, second])
    expect(tracker!.statuses).toBe(TRACKER.statuses.length * 2)
  })

  test.each([
    [1, '1 status'],
    [8, '8 statuses'],
  ])('%i is %j', (count, words) => {
    expect(statusesWord(count)).toBe(words)
  })
})

describe('the files', () => {
  test('are the ones the daemon reports, named for their layer', () => {
    expect(files(mockListing())).toEqual([
      { label: 'You', path: '/home/mock/.config/collide/config.yaml', exists: true },
      { label: 'You, for this board', path: '/home/mock/.config/collide/boards/b-alpha/config.yaml', exists: false },
      { label: 'This repo', path: '/work/alpha/.collide/config.yaml', exists: true },
    ])
  })

  test('are none when the daemon reports none', () => {
    expect(files({ entries: [] })).toEqual([])
  })
})

describe('the source typed in Add a pack', () => {
  test.each([
    ['/work/tools/tidy', 'dev:/work/tools/tidy'],
    ['  ./tidy ', 'dev:./tidy'],
    ['../tidy', 'dev:../tidy'],
    ['https://example.test/pack.git', 'git:https://example.test/pack.git'],
    ['ssh://git@example.test/pack.git', 'git:ssh://git@example.test/pack.git'],
    ['git@example.test:org/pack.git', 'git:git@example.test:org/pack.git'],
    ['git:file:///tmp/remote//pack@main', 'git:file:///tmp/remote//pack@main'],
    ['dev:/work/tools/tidy', 'dev:/work/tools/tidy'],
    ['tidy', 'tidy'],
  ])('%j is sent as %j', (typed, sent) => {
    expect(sourceOf(typed)).toBe(sent)
  })
})
