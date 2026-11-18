import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { DEFAULT_LAYOUT } from '../src/layout.ts'
import { SUFFIX_LETTERS, takenIds } from '../src/ids.ts'
import { applyOp } from '../src/write.ts'
import { commitAll, connect, EPIC_FILE, frontmatter, git, tempDir } from './helpers.ts'

const SHARED = '.collide/planning'
const EPIC = 'ep-001-aa'
const SHARED_TASK = 'ts-0001-aa'
const SHARED_FILE = `${SHARED}/epics/ep-001-aa-alpha/tasks/${SHARED_TASK}-made-up.md`

type Client = ReturnType<typeof connect>

/** A git repository with one epic and one task in the shared root, and an empty pack data folder beside it. */
function world(files: Record<string, string> = {}) {
  const repo = tempDir()
  const all = {
    'README.md': 'fixture\n',
    [`${SHARED}/epics/ep-001-aa-alpha/epic.md`]: EPIC_FILE,
    [SHARED_FILE]: frontmatter(SHARED_TASK),
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

/** Runs `body` with a client that `initialize` has given the world's repository and pack data folder. */
async function withClient<T>(
  w: World,
  body: (client: Client) => Promise<T>,
  packData: string | null = w.data,
): Promise<T> {
  const client = connect({ packData })
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
type Row = Record<string, any>
const list = async (client: Client, params: Record<string, unknown> = {}) =>
  (await client.request('list', params)).result as { tasks: Row[]; invalid: Row[] }
const versionOf = async (client: Client, id: string): Promise<string> =>
  (await client.request('get', { id })).result?.task.version
/** A `link` to `to` on `id`, written against the version that is on disk. */
const link = async (client: Client, id: string, to: string) =>
  apply(client, { op: 'link', id, version: await versionOf(client, id), args: { to } })
const status = (w: World): string => git(w.repo, 'status', '--porcelain')
const idsOf = (listing: { tasks: Array<Record<string, any>> }): string[] => listing.tasks.map((task) => task.id).sort()

const privateItem = (kind: string, title: string, extra: Record<string, unknown> = {}) =>
  ({ kind, epic: EPIC, title, root: 'private', ...extra })
const proposal = (title: string, extra: Record<string, unknown> = {}) => privateItem('proposal', title, extra)
const task = (title: string, extra: Record<string, unknown> = {}) => privateItem('task', title, extra)

describe('a private item', () => {
  test('is created under the pack data folder, lists with part-of its epic, and reports where it is', async () => {
    const w = world()
    const { created, listing } = await withClient(w, async (client) => ({
      created: await create(client, proposal('Private idea')),
      listing: await list(client),
    }))
    expect(created).toMatchObject({ outcome: 'ok', files: [], task: { kind: 'proposal', fields: { root: 'private' } } })
    expect(created?.task.path).toMatch(/^private:epics\/ep-001-aa\/proposals\/pr-0001-[a-z]{2}-private-idea\.md$/)
    expect(created?.task.links).toEqual([{ type: 'part-of', to: EPIC }])
    const files = readdirSync(join(w.planning, 'epics', EPIC, 'proposals'))
    expect(files).toHaveLength(1)
    expect(created?.task.version).toMatch(/^[0-9a-f]{64}$/)

    const listed = listing.tasks.find((candidate) => candidate.id === created?.task.id)
    expect(listed).toMatchObject({
      path: created?.task.path,
      fields: { root: 'private' },
      links: [{ type: 'part-of', to: EPIC }],
    })
    expect(status(w)).toBe('')
  })

  test('a shared item has no root field, so it is not taken for a private one', async () => {
    const w = world()
    const listing = await withClient(w, (client) => list(client))
    expect(listing.tasks.find((candidate) => candidate.id === SHARED_TASK)?.fields).toBeUndefined()
  })

  test('changes by setStatus, link, claim and unclaim write no file of the repository, and report none', async () => {
    const w = world()
    const replies = await withClient(w, async (client) => {
      const made = await create(client, proposal('Private idea'))
      const id = made?.task.id as string
      const step = async (op: string, args: Record<string, unknown>) =>
        apply(client, { op, id, version: await versionOf(client, id), args })
      return [
        await step('setStatus', { status: 'proposed' }),
        await step('link', { to: SHARED_TASK }),
        await step('claim', { claimant: 'someone' }),
        await step('unclaim', {}),
      ]
    })
    expect(replies.map((reply) => reply?.outcome)).toEqual(['ok', 'ok', 'ok', 'ok'])
    expect(replies.map((reply) => reply?.files)).toEqual([[], [], [], []])
    expect(replies[0]?.task.state.status).toBe('proposed')
    expect(replies[1]?.task.links).toContainEqual({ type: 'blocked-by', to: SHARED_TASK })
    expect(status(w)).toBe('')
    expect(existsSync(join(w.repo, SHARED, '.write-lock'))).toBe(false)
  })

  test('never touches the repository, so one with no planning folder stays bare', async () => {
    const w = world()
    const bare = tempDir()
    writeFileSync(join(bare, 'README.md'), 'bare\n')
    git(bare, 'init', '--quiet')
    commitAll(bare, 'bare')
    const client = connect({ packData: w.data })
    try {
      await client.request('initialize', { config: {}, root: bare })
      const epic = await create(client, { kind: 'epic', title: 'Private epic', root: 'private' })
      const made = await create(client, { kind: 'task', epic: epic?.task.id, title: 'A task', root: 'private' })
      await apply(client, { op: 'claim', id: made?.task.id, version: made?.task.version, args: { claimant: 'me' } })
    } finally {
      await client.close()
    }
    expect(existsSync(join(bare, '.collide'))).toBe(false)
    expect(git(bare, 'status', '--porcelain')).toBe('')
    expect(existsSync(join(w.planning, '.write-lock'))).toBe(false)
  })

  test('an epic that is private has a folder named by its id alone, and holds private items', async () => {
    const w = world()
    const { epic, made } = await withClient(w, async (client) => {
      const epic = await create(client, { kind: 'epic', title: 'Private epic', root: 'private' })
      const made = await create(client, task('Inside it', { epic: epic?.task.id }))
      return { epic, made }
    })
    expect(epic?.task.id).toBe('ep-002-' + (epic?.task.id as string).slice(-2))
    expect(epic?.task.path).toBe(`private:epics/${epic?.task.id}/epic.md`)
    const inside = `^private:epics/${epic?.task.id}/tasks/ts-0002-[a-z]{2}-inside-it\\.md$`
    expect(made?.task.path).toMatch(new RegExp(inside))
    expect(made?.task.links).toEqual([{ type: 'part-of', to: epic?.task.id }])
    expect(status(w)).toBe('')
  })

  test('for an epic that is nowhere is refused, and nothing is made', async () => {
    const w = world()
    const reply = await withClient(w, (client) => create(client, task('Lost', { epic: 'ep-009-zz' })))
    expect(reply).toMatchObject({ outcome: 'refused' })
    expect(reply?.message).toContain('there is no epic ep-009-zz')
    expect(existsSync(join(w.planning, 'epics', 'ep-009-zz'))).toBe(false)
  })

  test('a private and a shared item link to each other, and both links list', async () => {
    const w = world()
    const { reply, listing } = await withClient(w, async (client) => {
      const made = await create(client, task('Private task'))
      const id = made?.task.id as string
      await apply(client, { op: 'link', id, version: made?.task.version, args: { to: SHARED_TASK } })
      const back = await link(client, SHARED_TASK, id)
      return { reply: back, listing: await list(client) }
    })
    const privateTask = listing.tasks.find((candidate) => candidate.fields?.root === 'private')
    const sharedTask = listing.tasks.find((candidate) => candidate.id === SHARED_TASK)
    expect(privateTask?.links).toContainEqual({ type: 'blocked-by', to: SHARED_TASK })
    expect(sharedTask?.links).toContainEqual({ type: 'blocked-by', to: privateTask?.id })
    expect(reply?.files).toEqual([SHARED_FILE])
    expect(status(w)).toBe(` M ${SHARED_FILE}\n`)
  })

  test('four creates at once get four numbers, under the private root\'s lock', async () => {
    const w = world()
    const replies = await withClient(w, (client) =>
      Promise.all([1, 2, 3, 4].map((seed) => create(client, task(`Private ${seed}`, { seed })))),
    )
    expect(replies.map((reply) => reply?.outcome)).toEqual(['ok', 'ok', 'ok', 'ok'])
    const ids = replies.map((reply) => reply?.task.id as string)
    expect(new Set(ids.map((id) => id.slice(0, 7))).size).toBe(4)
    expect(readdirSync(join(w.planning, 'epics', EPIC, 'tasks'))).toHaveLength(4)
    expect(status(w)).toBe('')
  })
})

describe('the lock', () => {
  /** A lock file that a live process (this one) holds, so that a writer for it waits. */
  const hold = (dir: string): void => {
    mkdirSync(dir, { recursive: true })
    writeFileSync(join(dir, '.write-lock'), `${process.pid}\n`)
  }
  /** `apply` straight on the provider code, with a lock wait that is short. */
  const direct = (w: World, params: Record<string, unknown>) =>
    applyOp(w.repo, params as never, { timeoutMs: 300 }, {}, DEFAULT_LAYOUT, w.planning)
  const createPrivate = (w: World, seed: number) =>
    direct(w, { op: 'create', args: { seed, ...task('Held') } }) as Promise<Row>
  const createShared = (w: World) =>
    direct(w, { op: 'create', args: { epic: EPIC, title: 'Shared', seed: 3 } })
  const waiting = (w: World, root: string) => `waiting for the lock file ${join(root, '.write-lock')}`

  test('of the private root holds back private writes, not a shared write', async () => {
    const w = world()
    const { task: made } = await createPrivate(w, 1)
    hold(w.planning)
    const claim = { op: 'claim', id: made.id, version: made.version, args: { claimant: 'me' } }
    await expect(createPrivate(w, 2)).rejects.toThrow(waiting(w, w.planning))
    await expect(direct(w, claim)).rejects.toThrow(waiting(w, w.planning))
    expect(await createShared(w)).toMatchObject({ outcome: 'ok' })
  })

  test('of the shared root holds back a shared write, not a private one', async () => {
    const w = world()
    hold(join(w.repo, SHARED))
    await expect(createShared(w)).rejects.toThrow(waiting(w, join(w.repo, SHARED)))
    expect(await createPrivate(w, 1)).toMatchObject({ outcome: 'ok' })
  })

  test('of the private root is held by a reid too, which may rewrite files in it', async () => {
    const w = world()
    const { task: made } = await createPrivate(w, 1)
    hold(w.planning)
    await expect(direct(w, { op: 'reid', id: made.id, args: { seed: 5 } })).rejects.toThrow(waiting(w, w.planning))
    expect(existsSync(join(w.repo, SHARED, '.write-lock'))).toBe(false)
  })
})

describe('the root a create names', () => {
  test('is shared when none is named, and shared when it says so', async () => {
    const w = world()
    const replies = await withClient(w, async (client) => [
      await create(client, { epic: EPIC, title: 'Plain' }),
      await create(client, { epic: EPIC, title: 'Said so', root: 'shared', seed: 2 }),
    ])
    for (const reply of replies) {
      expect(reply).toMatchObject({ outcome: 'ok' })
      expect(reply?.task.path).toMatch(new RegExp(`^${SHARED}/epics/ep-001-aa-alpha/tasks/`))
      expect(reply?.task.fields).toBeUndefined()
    }
    expect(existsSync(w.planning)).toBe(false)
  })

  test('is one of shared and private, and anything else is refused', async () => {
    const w = world()
    const reply = await withClient(w, (client) => create(client, { epic: EPIC, title: 'Odd', root: 'team' }))
    expect(reply).toMatchObject({ outcome: 'refused' })
    expect(reply?.message).toContain('root must be "shared" or "private"')
  })

  test('private is refused with the reason when the pack has no data folder', async () => {
    const w = world()
    for (const packData of [null, 'relative/data']) {
      const reply = await withClient(w, (client) => create(client, task('No home')), packData)
      expect(reply).toMatchObject({ outcome: 'refused' })
      expect(reply?.message).toContain('COLLIDE_PACK_DATA')
    }
    expect(status(w)).toBe('')
  })

  test('private is refused for a checkout that is not the main one', async () => {
    const w = world()
    const linked = join(tempDir(), 'linked')
    git(w.repo, 'worktree', 'add', '--quiet', '-b', 'side', linked)
    const reply = await withClient(w, (client) => create(client, task('In a worktree'), { checkout: linked }))
    expect(reply).toMatchObject({ outcome: 'refused' })
    expect(reply?.message).toContain('main checkout')
    expect(existsSync(w.planning)).toBe(false)
  })
})

describe('a repository with no private root', () => {
  test('lists as it did before, with or without a data folder', async () => {
    const w = world()
    const without = await withClient(w, (client) => list(client), null)
    const empty = await withClient(w, (client) => list(client))
    expect(empty).toEqual(without)
    expect(idsOf(empty)).toEqual([EPIC, SHARED_TASK])
  })
})

describe('a named checkout', () => {
  test('lists only the shared root, and the main checkout named out loud lists both', async () => {
    const w = world()
    const linked = join(tempDir(), 'linked')
    git(w.repo, 'worktree', 'add', '--quiet', '-b', 'side', linked)
    const { id, inLinked, inMain, got, written } = await withClient(w, async (client) => {
      const made = await create(client, task('Private task'))
      const id = made?.task.id as string
      return {
        id,
        inLinked: await list(client, { checkout: linked }),
        inMain: await list(client, { checkout: w.repo }),
        got: (await client.request('get', { id, checkout: linked })).result,
        written: await apply(client, {
          op: 'claim',
          id,
          version: made?.task.version,
          checkout: linked,
          args: { claimant: 'me' },
        }),
      }
    })
    expect(idsOf(inLinked)).toEqual([EPIC, SHARED_TASK])
    expect(idsOf(inMain)).toEqual([EPIC, SHARED_TASK, id].sort())
    expect(got).toEqual({})
    expect(written).toMatchObject({ outcome: 'refused' })
    expect(written?.message).toContain(`there is no item ${id}`)
  })
})

describe('ids across the two roots', () => {
  test('a private and a shared create get different numbers', async () => {
    const w = world()
    const ids = await withClient(w, async (client) => [
      (await create(client, task('One')))?.task.id as string,
      (await create(client, { epic: EPIC, title: 'Two', seed: 2 }))?.task.id as string,
      (await create(client, task('Three', { seed: 3 })))?.task.id as string,
    ])
    expect(ids.map((id) => id.slice(0, 7))).toEqual(['ts-0002', 'ts-0003', 'ts-0004'])
  })

  test('takenIds reads the private root only when it is given one', async () => {
    const w = world()
    await withClient(w, (client) => create(client, task('One')))
    const without = await takenIds(w.repo)
    const withPrivate = await takenIds(w.repo, DEFAULT_LAYOUT, w.planning)
    expect([...without].filter((id) => id.startsWith('ts-'))).toEqual([SHARED_TASK])
    expect([...withPrivate].filter((id) => id.startsWith('ts-'))).toHaveLength(2)
  })

  /** The suffixes in the order a rigged generator gives them: each letter is the middle of its slot. */
  const rigged = (...suffixes: string[]) => {
    const letters = suffixes.join('').split('')
    return () => () => (SUFFIX_LETTERS.indexOf(letters.shift() ?? 'a') + 0.5) / SUFFIX_LETTERS.length
  }

  test.each(['shared', 'private'])('a %s create redraws an id the other root takes', async (into) => {
    const w = world()
    const taken = 'ts-0002-ab'
    let looks = 0
    const otherFolder =
      into === 'shared' ? join(w.planning, 'epics', EPIC, 'tasks') : join(w.repo, SHARED, 'epics/ep-001-aa-alpha/tasks')
    const result = await applyOp(
      w.repo,
      { op: 'create', args: { epic: EPIC, title: 'Raced', seed: 1, root: into } },
      {},
      {
        generator: rigged('ab', 'cd'),
        look: async (path, layout, privateDir) => {
          looks++
          // The first look finds number 2 free. A file lands in the other root before the first draw is checked.
          if (looks === 2) {
            mkdirSync(otherFolder, { recursive: true })
            writeFileSync(join(otherFolder, `${taken}-landed.md`), frontmatter(taken))
          }
          return takenIds(path, layout, privateDir)
        },
      },
      DEFAULT_LAYOUT,
      w.planning,
    )
    expect(result).toMatchObject({ outcome: 'ok', task: { id: 'ts-0002-cd' } })
  })
})

describe('two files with one id, one in each root', () => {
  test('the shared one is the item, and the private one is invalid for the second', async () => {
    const w = world()
    const folder = join(w.planning, 'epics', EPIC, 'tasks')
    mkdirSync(folder, { recursive: true })
    writeFileSync(join(folder, `${SHARED_TASK}-copy.md`), frontmatter(SHARED_TASK))
    const listing = await withClient(w, (client) => list(client))
    expect(idsOf(listing)).toEqual([EPIC, SHARED_TASK])
    expect(listing.tasks.find((candidate) => candidate.id === SHARED_TASK)?.path).toBe(SHARED_FILE)
    expect(listing.invalid).toHaveLength(1)
    expect(listing.invalid[0]?.path).toBe(`private:epics/${EPIC}/tasks/${SHARED_TASK}-copy.md`)
    expect(listing.invalid[0]?.reason).toBe(`id "${SHARED_TASK}" is already used by ${SHARED_FILE}`)
  })

  test('a private file that does not read is invalid, named with the private prefix', async () => {
    const w = world()
    const folder = join(w.planning, 'epics', EPIC, 'tasks')
    mkdirSync(folder, { recursive: true })
    writeFileSync(join(folder, 'ts-0005-bb-broken.md'), 'no frontmatter here\n')
    const listing = await withClient(w, (client) => list(client))
    expect(listing.invalid.map((item) => item.path)).toEqual([`private:epics/${EPIC}/tasks/ts-0005-bb-broken.md`])
  })
})

describe('reid with a private item', () => {
  test('renames the private file, rewrites links in both roots, and reports only files of the repository', async () => {
    const w = world()
    const { oldId, reply, listing } = await withClient(w, async (client) => {
      const made = await create(client, task('Private task'))
      const oldId = made?.task.id as string
      await link(client, SHARED_TASK, oldId)
      await link(client, oldId, SHARED_TASK)
      const reply = await apply(client, { op: 'reid', id: oldId, args: { seed: 7 } })
      return { oldId, reply, listing: await list(client) }
    })
    const newId = reply?.task.id as string
    expect(newId).not.toBe(oldId)
    expect(newId.slice(0, 7)).toBe(oldId.slice(0, 7))
    expect(reply?.task.path).toMatch(/^private:epics\/ep-001-aa\/tasks\//)
    expect(reply?.files).toEqual([SHARED_FILE])
    const shared = listing.tasks.find((candidate) => candidate.id === SHARED_TASK)
    expect(shared?.links).toContainEqual({ type: 'blocked-by', to: newId })
    expect(readFileSync(join(w.repo, SHARED_FILE), 'utf8')).not.toContain(oldId)
    expect(status(w)).toBe(` M ${SHARED_FILE}\n`)
  })

  test('on a private item that nothing in the repository names changes no file of the repository', async () => {
    const w = world()
    const reply = await withClient(w, async (client) => {
      const made = await create(client, task('Private task'))
      return apply(client, { op: 'reid', id: made?.task.id, args: { seed: 7 } })
    })
    expect(reply).toMatchObject({ outcome: 'ok', files: [] })
    expect(status(w)).toBe('')
  })
})
