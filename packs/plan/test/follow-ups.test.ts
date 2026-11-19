import { existsSync, mkdirSync, readdirSync, readFileSync, statSync, symlinkSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { commitAll, connect, EPIC_FILE, frontmatter, git, tempDir } from './helpers.ts'

const SHARED = '.collide/planning'
const EPIC = 'ep-001-aa'
const SHARED_TASK = 'ts-0001-aa'
const EPIC_DIR = `${SHARED}/epics/ep-001-aa-alpha`

type Client = ReturnType<typeof connect>
type Row = Record<string, any>

/** A git repository with one epic and one task in the shared root, and an empty pack data folder beside it. */
function world(files: Record<string, string> = {}) {
  const repo = tempDir()
  const all = {
    'README.md': 'fixture\n',
    [`${EPIC_DIR}/epic.md`]: EPIC_FILE,
    [`${EPIC_DIR}/tasks/${SHARED_TASK}-made-up.md`]: frontmatter(SHARED_TASK),
    ...files,
  }
  for (const [path, text] of Object.entries(all)) {
    mkdirSync(dirname(join(repo, path)), { recursive: true })
    writeFileSync(join(repo, path), text)
  }
  git(repo, 'init', '--quiet')
  commitAll(repo, 'fixture')
  const data = tempDir()
  return { repo, data, planning: join(data, 'planning') }
}
type World = ReturnType<typeof world>

async function withClient<T>(w: World, body: (client: Client) => Promise<T>): Promise<T> {
  const client = connect({ packData: w.data })
  try {
    const init = await client.request('initialize', { config: {}, root: w.repo })
    if (init.error !== undefined) throw new Error(`initialize failed: ${JSON.stringify(init.error)}`)
    return await body(client)
  } finally {
    await client.close()
  }
}

const apply = async (client: Client, params: Record<string, unknown>) => (await client.request('apply', params)).result
const create = (client: Client, args: Record<string, unknown>, extra: Record<string, unknown> = {}) =>
  apply(client, { op: 'create', args: { seed: 1, ...args }, ...extra })
const list = async (client: Client) => (await client.request('list', {})).result as { tasks: Row[]; invalid: Row[] }
const versionOf = async (client: Client, id: string): Promise<string> =>
  (await client.request('get', { id })).result?.task.version
const keep = async (client: Client, id: string, extra: Record<string, unknown> = {}) =>
  apply(client, { op: 'move', id, version: await versionOf(client, id), args: {}, ...extra })
const status = (w: World): string => git(w.repo, 'status', '--porcelain', '--untracked-files=all')

const privateItem = (kind: string, title: string, extra: Record<string, unknown> = {}) =>
  ({ kind, epic: EPIC, title, root: 'private', ...extra })

describe('a create with raisedBy and run', () => {
  test('writes the run: line after status and a raised-by link, and lists them', async () => {
    const w = world()
    const { made, listing } = await withClient(w, async (client) => ({
      made: await create(client, privateItem('task', 'A found thing', { raisedBy: SHARED_TASK, run: 'R-7' })),
      listing: await list(client),
    }))
    expect(made).toMatchObject({ outcome: 'ok', files: [] })
    expect(made?.task.links).toEqual([
      { type: 'part-of', to: EPIC },
      { type: 'raised-by', to: SHARED_TASK },
    ])
    expect(made?.task.fields).toEqual({ run: 'R-7', root: 'private' })
    const tasks = join(w.planning, 'epics', EPIC, 'tasks')
    const file = join(tasks, readdirSync(tasks)[0] as string)
    expect(readFileSync(file, 'utf8')).toContain('status: ready\nrun: "R-7"\n')
    expect(listing.tasks.find((task) => task.id === made?.task.id)?.links).toContainEqual({
      type: 'raised-by',
      to: SHARED_TASK,
    })
    expect(listing.invalid).toEqual([])
  })

  test('run alone writes only the run: line', async () => {
    const w = world()
    const made = await withClient(w, (client) =>
      create(client, privateItem('question', 'Needs a sample', { run: 'R-8' })),
    )
    expect(made?.task.links).toEqual([{ type: 'part-of', to: EPIC }])
    expect(made?.task.fields).toMatchObject({ run: 'R-8' })
  })

  test('a malformed raisedBy or run is refused and nothing is written', async () => {
    const w = world()
    const replies = await withClient(w, async (client) => [
      await create(client, privateItem('task', 'Bad link', { raisedBy: 'not an id' })),
      await create(client, privateItem('task', 'Bad run', { run: 'R-7\nstatus: done' })),
      await create(client, privateItem('task', 'Empty run', { run: '' })),
    ])
    expect(replies.map((reply) => reply?.outcome)).toEqual(['refused', 'refused', 'refused'])
    expect(replies[0]?.message).toContain('raisedBy')
    expect(replies[1]?.message).toContain('run')
    expect(existsSync(join(w.planning, 'epics', EPIC))).toBe(false)
  })

  test('is not how a link is written: the link op refuses raised-by', async () => {
    const w = world()
    const reply = await withClient(w, async (client) => {
      const made = await create(client, privateItem('task', 'Plain'))
      return apply(client, {
        op: 'link',
        id: made?.task.id,
        version: made?.task.version,
        args: { to: SHARED_TASK, type: 'raised-by' },
      })
    })
    expect(reply).toMatchObject({ outcome: 'refused' })
    expect(reply?.message).toContain('type must be')
  })
})

describe('move (Keep)', () => {
  test('moves a private task to the shared epic folder, same id, links intact, and reports the file', async () => {
    const w = world()
    const { made, kept, listing } = await withClient(w, async (client) => {
      const made = await create(client, privateItem('task', 'Found work', { raisedBy: SHARED_TASK, run: 'R-7' }))
      const kept = await keep(client, made?.task.id)
      return { made, kept, listing: await list(client) }
    })
    const id = made?.task.id as string
    const path = `${EPIC_DIR}/tasks/${id}-found-work.md`
    expect(kept).toMatchObject({ outcome: 'ok', files: [path], version: made?.task.version })
    expect(kept?.task).toMatchObject({ id, path, state: { status: 'ready' } })
    expect(kept?.task.fields).toEqual({ run: 'R-7' })
    expect(kept?.task.links).toEqual([
      { type: 'part-of', to: EPIC },
      { type: 'raised-by', to: SHARED_TASK },
    ])
    expect(kept?.items).toEqual([
      {
        key: `plan:${id}`,
        version: made?.task.version,
        from: { status: 'ready', category: 'ready' },
        to: { status: 'ready', category: 'ready' },
      },
    ])
    expect(existsSync(join(w.planning, 'epics', EPIC, 'tasks', `${id}-found-work.md`))).toBe(false)
    expect(listing.tasks.filter((task) => task.id === id)).toHaveLength(1)
    expect(listing.tasks.find((task) => task.id === id)?.path).toBe(path)
    expect(listing.invalid).toEqual([])
    expect(status(w)).toBe(`?? ${path}\n`)
  })

  test('moves a private question the same way', async () => {
    const w = world()
    const { made, kept } = await withClient(w, async (client) => {
      const made = await create(client, privateItem('question', 'Which sample?'))
      return { made, kept: await keep(client, made?.task.id) }
    })
    const path = `${EPIC_DIR}/questions/${made?.task.id}-which-sample.md`
    expect(kept).toMatchObject({ outcome: 'ok', files: [path] })
    expect(readFileSync(join(w.repo, path), 'utf8')).toContain('kind: question')
    expect(status(w)).toBe(`?? ${path}\n`)
  })

  test('moves a private spike folder with every file in it, to the shared spikes folder', async () => {
    const w = world()
    const { made, kept } = await withClient(w, async (client) => {
      const made = await create(client, { kind: 'spike', title: 'Probe it', root: 'private' })
      const dir = dirname(join(w.planning, 'spikes', readdirSync(join(w.planning, 'spikes'))[0] as string, 'record.md'))
      mkdirSync(join(dir, 'scripts'))
      writeFileSync(join(dir, 'scripts', 'probe.sh'), '#!/bin/sh\necho probe\n', { mode: 0o755 })
      return { made, kept: await keep(client, made?.task.id) }
    })
    const folder = `${SHARED}/spikes/${made?.task.id}-probe-it`
    expect(kept).toMatchObject({ outcome: 'ok' })
    expect([...(kept?.files as string[])].sort()).toEqual([`${folder}/record.md`, `${folder}/scripts/probe.sh`])
    expect(kept?.task.path).toBe(`${folder}/record.md`)
    expect(statSync(join(w.repo, folder, 'scripts', 'probe.sh')).mode & 0o111).not.toBe(0)
    expect(readdirSync(join(w.planning, 'spikes'))).toEqual([])
    expect(status(w)).toBe(`?? ${folder}/record.md\n?? ${folder}/scripts/probe.sh\n`)
  })

  test('lands in the shared folder of the epic, whatever the folder is called', async () => {
    const w = world({ [`${SHARED}/epics/ep-002-bb-beta-work/epic.md`]: EPIC_FILE.replace('ep-001-aa', 'ep-002-bb') })
    const kept = await withClient(w, async (client) => {
      const made = await create(client, privateItem('task', 'In beta', { epic: 'ep-002-bb' }))
      return keep(client, made?.task.id)
    })
    const folder = `${SHARED}/epics/ep-002-bb-beta-work/tasks`
    expect(kept?.task.path).toMatch(new RegExp(`^${folder}/ts-0002-[a-z]{2}-in-beta\\.md$`))
  })

  test('refuses a proposal, which is shared whole, and moves nothing', async () => {
    const w = world()
    const { made, reply } = await withClient(w, async (client) => {
      const made = await create(client, privateItem('proposal', 'Big idea'))
      return { made, reply: await keep(client, made?.task.id) }
    })
    expect(reply).toMatchObject({ outcome: 'refused' })
    expect(reply?.message).toContain(`${made?.task.id} is a proposal`)
    expect(reply?.message).toContain('shared whole')
    expect(readdirSync(join(w.planning, 'epics', EPIC, 'proposals'))).toHaveLength(1)
    expect(status(w)).toBe('')
  })

  test('refuses an epic, which has a folder of items, and moves nothing', async () => {
    const w = world()
    const { made, reply } = await withClient(w, async (client) => {
      const made = await create(client, { kind: 'epic', title: 'Private epic', root: 'private' })
      return { made, reply: await keep(client, made?.task.id) }
    })
    expect(reply).toMatchObject({ outcome: 'refused' })
    expect(reply?.message).toContain(`${made?.task.id} is an epic`)
    expect(existsSync(join(w.planning, 'epics', made?.task.id, 'epic.md'))).toBe(true)
    expect(status(w)).toBe('')
  })

  test('refuses an item that links to a private item, naming it, and moves nothing', async () => {
    const w = world()
    const { first, second, reply } = await withClient(w, async (client) => {
      const first = await create(client, privateItem('task', 'First'))
      const second = await create(client, privateItem('task', 'Second', { seed: 2 }))
      await apply(client, {
        op: 'link',
        id: second?.task.id,
        version: second?.task.version,
        args: { to: first?.task.id },
      })
      return { first, second, reply: await keep(client, second?.task.id) }
    })
    expect(reply).toMatchObject({ outcome: 'refused' })
    expect(reply?.message).toContain(first?.task.id)
    expect(reply?.message).toContain('private')
    expect(existsSync(join(w.planning, 'epics', EPIC, 'tasks'))).toBe(true)
    expect(readdirSync(join(w.planning, 'epics', EPIC, 'tasks'))).toHaveLength(2)
    expect(second?.outcome).toBe('ok')
    expect(status(w)).toBe('')
  })

  test('keeps an item that links to a shared item, the link intact', async () => {
    const w = world()
    const kept = await withClient(w, async (client) => {
      const made = await create(client, privateItem('task', 'Linked'))
      const linked = await apply(client, {
        op: 'link',
        id: made?.task.id,
        version: made?.task.version,
        args: { to: SHARED_TASK },
      })
      return keep(client, made?.task.id, { version: linked?.version })
    })
    expect(kept).toMatchObject({ outcome: 'ok' })
    expect(kept?.task.links).toContainEqual({ type: 'blocked-by', to: SHARED_TASK })
  })

  test('refuses an item whose id the shared plan already has, on another branch, and moves nothing', async () => {
    const w = world()
    const { made, reply } = await withClient(w, async (client) => {
      const made = await create(client, privateItem('task', 'Clash'))
      const name = `${made?.task.id}-theirs.md`
      git(w.repo, 'checkout', '--quiet', '-b', 'other')
      mkdirSync(join(w.repo, EPIC_DIR, 'tasks'), { recursive: true })
      writeFileSync(join(w.repo, EPIC_DIR, 'tasks', name), frontmatter(made?.task.id))
      commitAll(w.repo, 'their item')
      git(w.repo, 'checkout', '--quiet', 'main')
      return { made, reply: await keep(client, made?.task.id) }
    })
    expect(reply).toMatchObject({ outcome: 'refused' })
    expect(reply?.message).toContain(`${made?.task.id} is already used in the shared plan`)
    expect(readdirSync(join(w.planning, 'epics', EPIC, 'tasks'))).toHaveLength(1)
  })

  test('refuses an item that is already in the shared root, and one whose id is in both roots', async () => {
    const w = world()
    const replies = await withClient(w, async (client) => {
      const shared = await keep(client, SHARED_TASK)
      const made = await create(client, privateItem('task', 'Doubled'))
      const copy = join(w.repo, EPIC_DIR, 'tasks', `${made?.task.id}-copy.md`)
      writeFileSync(copy, frontmatter(made?.task.id as string))
      return [shared, await keep(client, made?.task.id as string)]
    })
    expect(replies.map((reply) => reply?.outcome)).toEqual(['refused', 'refused'])
    expect(replies[0]?.message).toContain('already in the shared root')
    expect(replies[1]?.message).toContain('both roots')
  })

  test('refuses an item whose epic has no folder in the shared root, and moves nothing', async () => {
    const w = world()
    const { reply, made } = await withClient(w, async (client) => {
      const epic = await create(client, { kind: 'epic', title: 'Private epic', root: 'private' })
      const made = await create(client, privateItem('task', 'In a private epic', { epic: epic?.task.id }))
      return { made, reply: await keep(client, made?.task.id) }
    })
    expect(reply).toMatchObject({ outcome: 'refused' })
    expect(reply?.message).toContain('is private')
    expect(made?.outcome).toBe('ok')
    expect(status(w)).toBe('')
  })

  test('refuses an item with a link in a spike folder, a link file, and moves nothing', async () => {
    const w = world()
    const reply = await withClient(w, async (client) => {
      const made = await create(client, { kind: 'spike', title: 'Linked probe', root: 'private' })
      const dir = join(w.planning, 'spikes', readdirSync(join(w.planning, 'spikes'))[0] as string)
      symlinkSync('/nonexistent-target', join(dir, 'ref'))
      return keep(client, made?.task.id)
    })
    expect(reply).toMatchObject({ outcome: 'refused' })
    expect(reply?.message).toContain('not a file')
    expect(readdirSync(join(w.planning, 'spikes'))).toHaveLength(1)
    expect(status(w)).toBe('')
  })

  test('is refused to an agent, naming who may keep, and moves nothing', async () => {
    const w = world()
    const { made, reply } = await withClient(w, async (client) => {
      const made = await create(client, privateItem('task', 'Mine'))
      const actor = { kind: 'agent', at: '2026-06-15', run: 'R-7' }
      return { made, reply: await keep(client, made?.task.id, { actor }) }
    })
    expect(reply).toMatchObject({ outcome: 'refused' })
    expect(reply?.message).toContain('the agent of run R-7')
    expect(reply?.message).toContain('the app or the CLI')
    expect(readdirSync(join(w.planning, 'epics', EPIC, 'tasks'))).toHaveLength(1)
    expect(status(w)).toBe('')
  })

  test('takes a person or no actor, and needs the version it was read at', async () => {
    const w = world()
    const replies = await withClient(w, async (client) => {
      const made = await create(client, privateItem('task', 'Careful'))
      const stale = await keep(client, made?.task.id, { version: 'f'.repeat(64) })
      const none = await apply(client, { op: 'move', id: made?.task.id, args: {} })
      const person = await keep(client, made?.task.id, { actor: { kind: 'person', at: '2026-06-15' } })
      return [stale, none, person]
    })
    expect(replies[0]).toMatchObject({ outcome: 'conflict' })
    expect(replies[1]).toMatchObject({ outcome: 'refused' })
    expect(replies[1]?.message).toContain('version')
    expect(replies[2]).toMatchObject({ outcome: 'ok' })
  })

  test('takes no argument', async () => {
    const w = world()
    const reply = await withClient(w, async (client) => {
      const made = await create(client, privateItem('task', 'Plain'))
      return keep(client, made?.task.id, { args: { to: 'elsewhere' } })
    })
    expect(reply).toMatchObject({ outcome: 'refused' })
    expect(reply?.message).toContain('move: ')
  })

  test('is declared as an op of every kind', async () => {
    const w = world()
    const declared = await withClient(w, async (client) => {
      return (await client.request('initialize', { config: {}, root: w.repo })).result
    })
    expect(declared?.ops.map((op: Row) => op.name)).toContain('move')
  })
})
