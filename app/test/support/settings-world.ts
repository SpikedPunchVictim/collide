// A scripted world for the Settings page (ts-0193-gk): a Field world that also answers `config.get`, `config.set`,
// `config.pack_list` and `config.pack_add` over plain data, as the daemon does. The settings are the mock's: a value
// the repo overrode, one of the user's, defaults, a problem on a row, and two secrets, one set. The fake-owner page
// and the vitest tests share it. It keeps no secret: `config.set` of one only marks it set, and a test reads what was
// sent from `owner.calls`, which are in memory and never written anywhere.
import type { ConfigGetResultOutput, ConfigPackResultOutput } from '../../src/owner/generated.ts'
import { OwnerError } from '../../src/owner/index.ts'
import { PLAN, TRACKER, FieldWorld } from './field-world.ts'

export type Listing = ConfigGetResultOutput
export type PackEntry = ConfigPackResultOutput['pack']

export const CONFLICT = -32000

/** The mock's settings: what a board with a repo file, a user file and two packs reads. */
export function mockListing(): Listing {
  return {
    entries: [
      { key: 'ask.agent', value: 'claude', layer: 'default', secret: false },
      { key: 'ask.timeout_s', value: 120, layer: 'user', secret: false },
      {
        key: 'ask.budget_usd',
        value: 0.25,
        layer: 'repo',
        secret: false,
        overridden: { value: 0.5, layer: 'user' },
      },
      { key: 'disk.floor_gib', value: 5, layer: 'default', secret: false },
      { key: 'claude.check_published', value: true, layer: 'default', secret: false },
      { key: 'notify.webhook', layer: 'user', secret: true, is_set: false },
      { key: 'gateway.apiKey', layer: 'user', secret: true, is_set: true },
    ],
    files: [
      { layer: 'user', path: '/home/mock/.config/collide/config.yaml', exists: true },
      { layer: 'user', for_board: true, path: '/home/mock/.config/collide/boards/b-alpha/config.yaml', exists: false },
      { layer: 'repo', path: '/work/alpha/.collide/config.yaml', exists: true },
    ],
  }
}

export function pack(name: string, source: string, origin: PackEntry['origin'], over: Partial<PackEntry> = {}) {
  return { name, source, origin, enabled: true, dev: false, ...over } satisfies PackEntry
}

/** What `config.pack_add` shows for a pack that is about to be added. */
export function reviewed(name: string, source: string): PackEntry {
  return pack(name, source, 'user', {
    enabled: false,
    dev: source.startsWith('dev:'),
    review: {
      name,
      version: '1.2.0',
      tree: '0123456789abcdef0123456789abcdef01234567',
      hooks: [{ key: 'on_start', script: 'hooks/start.mjs' }],
      providers: [{ key: 'tasks', script: 'providers/tasks.mjs' }],
      observers: [],
      requires: ['git'],
    },
  })
}

export class SettingsWorld extends FieldWorld {
  listing_ = mockListing()
  packs: PackEntry[] = [
    pack('collide/plan', 'builtin', 'builtin'),
    pack('demo/tracker', 'repo:.collide/packs/tracker', 'repo'),
  ]
  /** Set to make `config.get` refuse with these words. */
  refuseConfig: string | null = null
  /** Set to make `config.set` refuse with these words (the daemon's words never hold the value). */
  refuseSet: string | null = null
  /** The sources `config.pack_add` knows, by the source typed, with the host each needs a yes for, if any. */
  addable = new Map<string, { pack: PackEntry; needsHost?: string }>()
  /** The sources `config.pack_add` was asked to write, in order. */
  added: string[] = []

  constructor() {
    super()
    this.sources = [PLAN, TRACKER]
    this.owner.handle('config.get', () => {
      if (this.refuseConfig !== null) throw new OwnerError(CONFLICT, this.refuseConfig)
      return this.listing_
    })
    this.owner.handle('config.set', (params) => {
      if (this.refuseSet !== null) throw new OwnerError(CONFLICT, this.refuseSet)
      const found = this.listing_.entries.find((entry) => entry.key === params.key && entry.secret)
      if (found === undefined) throw new OwnerError(CONFLICT, `\`${params.key}\` is not a secret`)
      found.is_set = true
      return { entry: { key: found.key, layer: 'user', secret: true, is_set: true } }
    })
    this.owner.handle('config.pack_list', () => ({ packs: this.packs }))
    this.owner.handle('config.pack_add', ({ source, confirm, approve_hosts }) => {
      const known = this.addable.get(source)
      if (known === undefined) throw new OwnerError(CONFLICT, `\`${source}\`: no pack is there`)
      const { needsHost } = known
      if (needsHost !== undefined && !(approve_hosts ?? []).includes(needsHost)) {
        return { pack: pack(source, source, 'user', { enabled: false }), written: false, needs_host: needsHost }
      }
      if (!confirm) return { pack: known.pack, written: false, note: 'nothing was written: confirm to add it' }
      const { name, dev } = known.pack
      this.packs = [...this.packs, pack(name, source, 'user', { enabled: false, dev })]
      this.added.push(source)
      return { pack: known.pack, written: true }
    })
  }
}

/** A board with the mock's settings. */
export function settingsWorld(): SettingsWorld {
  const world = new SettingsWorld()
  world.addable.set('dev:/work/tools/tidy', { pack: reviewed('tidy', 'dev:/work/tools/tidy') })
  world.addable.set('git:https://example.test/pack.git', {
    pack: reviewed('remote', 'git:https://example.test/pack.git'),
    needsHost: 'example.test',
  })
  return world
}

/** A board where a value in the user's file is refused, and a key names no row. */
export function settingsProblemsWorld(): SettingsWorld {
  const world = settingsWorld()
  world.listing_.problems = [
    {
      key: 'ask.timeout_s',
      layer: 'user',
      message: "`ask.timeout_s` in the user's config must be at least 10",
    },
    { key: 'ferry', layer: 'repo', message: "`config.ferry` is a name Collide's own settings use, so it is no pack" },
  ]
  return world
}

/** A board whose settings the daemon refuses to read: the page shows its words. */
export function settingsRefusedWorld(): SettingsWorld {
  const world = settingsWorld()
  world.refuseConfig = 'the settings have problems:\n  /home/mock/.config/collide/config.yaml: it does not read'
  return world
}

/** A board whose daemon refuses to take a secret, in words that hold no value. */
export function settingsSecretRefusedWorld(): SettingsWorld {
  const world = settingsWorld()
  world.refuseSet = '/home/mock/.config/collide/config.yaml: cannot read it, so nothing was set'
  return world
}
