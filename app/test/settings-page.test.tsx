import { renderToString } from 'react-dom/server'
import { describe, expect, test } from 'vitest'
import { GeneralTab } from '../src/settings/GeneralTab.tsx'
import { PackReview } from '../src/settings/PackReview.tsx'
import { PacksTab } from '../src/settings/PacksTab.tsx'
import { SecretsTab } from '../src/settings/SecretsTab.tsx'
import { PLAN, TRACKER } from './support/field-world.ts'
import { mockListing, pack, reviewed, settingsProblemsWorld } from './support/settings-world.ts'

// The tabs of the Settings page drawn to text (ts-0193-gk): the table with its badges and its struck value, problems in
// place, the secrets list, the packs and a pack's review. The page's loading, its windows and its clicks are in
// `browser/fake-settings.spec.ts` and `browser/settings.spec.ts`, which run the page.

const noop = () => {}
const text = (html: string) => html.replace(/<[^>]*>/g, ' ').replace(/&#x27;/g, "'").replace(/\s+/g, ' ')
const count = (html: string, part: string) => html.split(part).length - 1

describe('the General tab', () => {
  const html = renderToString(<GeneralTab listing={mockListing()} />)

  test('is one table: Setting, Value in use, Set in', () => {
    expect(count(html, '<table')).toBe(1)
    expect(text(html)).toMatch(/Setting\s+Value in use\s+Set in/)
  })

  test('names each setting with its key beneath', () => {
    expect(text(html)).toContain('Spend cap for one Ask ask.budget_usd')
    expect(text(html)).toContain('Free disk space to keep disk.floor_gib')
  })

  test('badges each row with its layer', () => {
    expect(text(html)).toMatch(/Time limit for one Ask.*120 s.*You/)
    expect(text(html)).toMatch(/Spend cap for one Ask.*\$0\.25.*This repo/)
    expect(text(html)).toMatch(/Free disk space to keep.*5 GiB.*Default/)
  })

  test('strikes out the value the repo overrode, and says whose it was', () => {
    expect(html).toMatch(/<s[^>]*>\$0\.50<\/s>/)
    expect(text(html)).toContain('yours, overridden by this repo')
    expect(count(html, '<s ')).toBe(1)
  })

  test('says nothing of a pack beating the user\'s value', () => {
    expect(text(html)).not.toContain('yours wins over')
  })

  test('shows no secret in the table', () => {
    expect(text(html)).not.toContain('notify.webhook')
  })

  test('lists the files the daemon reports, and none of its own', () => {
    expect(html).toContain('/home/mock/.config/collide/config.yaml')
    expect(html).toContain('/work/alpha/.collide/config.yaml')
    expect(text(html)).toMatch(/boards\/b-alpha\/config\.yaml \(no file yet\)/)
    expect(html).not.toContain('Application Support')
  })
})

describe('problems in place', () => {
  const { listing_ } = settingsProblemsWorld()
  const html = renderToString(<GeneralTab listing={listing_} />)

  test('show on the row of their setting, in the daemon\'s words', () => {
    const words = "`ask.timeout_s` in the user's config must be at least 10"
    expect(text(html)).toContain(`Time limit for one Ask ask.timeout_s 120 s ${words}`)
  })

  test('show above the table when they name no row', () => {
    expect(html).toMatch(/aria-label="Problems with the settings"/)
    expect(text(html)).toContain("is a name Collide's own settings use")
  })

  test('are absent when there are none', () => {
    expect(renderToString(<GeneralTab listing={mockListing()} />)).not.toContain('Problems with the settings')
  })
})

describe('the Secrets tab', () => {
  test('lists each secret as Set or Not set, with Set… or Replace…', () => {
    const html = renderToString(<SecretsTab listing={mockListing()} onSet={noop} />)
    expect(text(html)).toMatch(/webhook notify asks for it Not set/)
    expect(text(html)).toMatch(/apiKey gateway asks for it Set/)
    expect(html).toContain('aria-label="Set webhook"')
    expect(html).toContain('aria-label="Replace apiKey"')
  })

  test('shows no value, even one the answer should not have held', () => {
    const listing = mockListing()
    listing.entries = listing.entries.map((entry) =>
      entry.secret ? { ...entry, value: 'not-a-real-secret-0193' } : entry,
    )
    const html = renderToString(<SecretsTab listing={listing} onSet={noop} />)
    expect(html).not.toContain('not-a-real-secret-0193')
  })

  test('says so when no pack asks for a secret', () => {
    const html = renderToString(<SecretsTab listing={{ entries: [] }} onSet={noop} />)
    expect(text(html)).toContain('No pack of this board asks for a secret.')
  })
})

describe('the Packs tab', () => {
  const packs = [
    pack('collide/plan', 'builtin', 'builtin'),
    pack('demo/tracker', 'repo:.collide/packs/tracker', 'repo'),
    pack('tidy', 'dev:/work/tools/tidy', 'user', { enabled: false }),
  ]
  const html = renderToString(<PacksTab packs={packs} sources={[PLAN, TRACKER]} onAdd={noop} />)

  test('lists each pack with where it is from and its source', () => {
    expect(text(html)).toContain('collide/plan Built in')
    expect(text(html)).toContain('demo/tracker Brought by this repo · repo:.collide/packs/tracker')
    expect(text(html)).toContain('tidy Added by you · dev:/work/tools/tidy')
  })

  test('gives each pack its status count', () => {
    expect(count(html, `${PLAN.statuses.length} statuses`)).toBeGreaterThanOrEqual(1)
    expect(text(html)).toMatch(new RegExp(`demo/tracker.*${TRACKER.statuses.length} statuses`))
  })

  test('marks a pack that is off, and offers Add a pack…', () => {
    expect(text(html)).toMatch(/tidy.*Off/)
    expect(text(html)).toContain('Add a pack…')
  })
})

describe('a pack\'s review', () => {
  const added = reviewed('tidy', 'dev:/work/tools/tidy')
  const html = renderToString(<PackReview source={added.source} dev={added.dev} review={added.review!} />)

  test('says where it is from, what it runs and what it needs', () => {
    expect(text(html)).toContain('tidy is a pack. A pack runs code on this Mac')
    expect(text(html)).toContain('From: dev:/work/tools/tidy')
    expect(text(html)).toContain('hook on_start : hooks/start.mjs')
    expect(text(html)).toContain('provider tasks : providers/tasks.mjs')
    expect(text(html)).toContain('It needs: git')
  })

  test('names the tree it is trusted at', () => {
    expect(text(html)).toContain('tree 0123456789ab')
  })

  // 0.9: a `dev:` pack is trusted per folder, so a change to it does not ask again.
  test('says a folder being developed is trusted as a folder and its changes do not ask again', () => {
    expect(text(html)).toContain('trusted as a folder')
    expect(text(html)).toContain('A change to it does not ask again')
    expect(text(html)).not.toContain('A change to it asks again')
  })

  test('says a pinned pack asks again when it changes', () => {
    const pinned = reviewed('tidy', 'git:/work/tools/tidy//pack@main')
    const shown = text(renderToString(<PackReview source={pinned.source} dev={pinned.dev} review={pinned.review!} />))
    expect(shown).toContain('A change to it asks again')
    expect(shown).not.toContain('trusted as a folder')
  })
})
