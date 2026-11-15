import { existsSync, readdirSync, readFileSync, statSync } from 'node:fs'
import { join } from 'node:path'
import { expect, test } from 'vitest'
import { EPIC_FILE, fileVersion, makeRepo, spawnProvider } from './helpers.ts'

const PLANNING = '.collide/planning'
const TASKS = join(PLANNING, 'epics/ep-001-aa/tasks')

const item = (id: string, links = 'links: []\n'): string => `---\nid: ${id}\nkind: task\nstatus: ready\ntitle: "Made-up item"\nowners: []\n${links}---\n\nMade-up content.\n`

/** Sends one op for `id`, starting from the version on disk and, on a conflict, retrying from the current task's version. */
async function applyWithRetry(client: Awaited<ReturnType<typeof spawnProvider>>, op: string, id: string, args: object): Promise<Record<string, any>> {
  let version = (await client.request('get', { id })).task.version
  for (;;) {
    const result = await client.request('apply', { op, id, args, version })
    if (result.outcome !== 'conflict') return result
    version = result.current.version
  }
}

const statusOf = (path: string): string => /^status: (.*)$/m.exec(readFileSync(path, 'utf8'))?.[1] as string

test('setStatus calls from separate provider processes on one file: every result is true to what is on disk', async () => {
  const root = makeRepo({ 'ep-001-aa/epic.md': EPIC_FILE, 'ep-001-aa/tasks/ts-0001-aa-one.md': item('ts-0001-aa') })
  const file = join(root, TASKS, 'ts-0001-aa-one.md')
  const processes = 6
  const rounds = 3
  const cycle = ['active', 'review', 'done', 'dropped']

  const results = await Promise.all(
    Array.from({ length: processes }, async (_, index) => {
      const client = await spawnProvider(root)
      const replies: Array<{ category: string; reply: Record<string, any> }> = []
      for (let round = 0; round < rounds; round++) {
        const category = cycle[(index + round) % cycle.length] as string
        replies.push({ category, reply: await applyWithRetry(client, 'setStatus', 'ts-0001-aa', { category }) })
      }
      await client.close()
      return replies
    }),
  )

  const all = results.flat()
  expect(all.every(({ reply }) => reply.outcome === 'ok')).toBe(true)
  // Each reply says the status it asked for, and the version it gives is a version that was on disk.
  expect(all.every(({ category, reply }) => reply.task.state.category === category)).toBe(true)

  const writes = all.filter(({ reply }) => reply.files.length === 1)
  // A version is a hash of content, so a file that returns to an earlier status has that status's version again.
  const finalVersion = fileVersion(file)
  const lastWrites = writes.filter(({ reply }) => reply.version === finalVersion)
  expect(lastWrites.length).toBeGreaterThan(0)
  expect(lastWrites.every(({ category }) => category === statusOf(file))).toBe(true)
  expect(writes.filter(({ category, reply }) => reply.version !== finalVersion && category === statusOf(file))).toEqual([])

  // The `from` and `to` of the writes chain into one serial history that starts at the file's first status and ends at its last:
  // a lost update (two writers that read the same `from`) would break the count at some status.
  const net = new Map<string, number>()
  for (const { reply } of writes) {
    const { from, to } = reply.items[0]
    net.set(from.status, (net.get(from.status) ?? 0) + 1)
    net.set(to.status, (net.get(to.status) ?? 0) - 1)
  }
  const nonZero = Object.fromEntries([...net].filter(([, count]) => count !== 0))
  expect(nonZero).toEqual(statusOf(file) === 'ready' ? {} : { ready: 1, [statusOf(file)]: -1 })

  expect(readdirSync(join(root, PLANNING)).sort()).toEqual(['epics'])
  expect(readdirSync(join(root, TASKS)).sort()).toEqual(['ts-0001-aa-one.md'])
}, 30_000)

test('link calls from separate provider processes lose no update', async () => {
  const blockers = ['ts-0101-aa', 'ts-0102-aa', 'ts-0103-aa', 'ts-0104-aa', 'ts-0105-aa', 'ts-0106-aa']
  const root = makeRepo({
    'ep-001-aa/epic.md': EPIC_FILE,
    'ep-001-aa/tasks/ts-0001-aa-target.md': item('ts-0001-aa'),
    ...Object.fromEntries(blockers.map((id) => [`ep-001-aa/tasks/${id}-blocker.md`, item(id)])),
  })
  const file = join(root, TASKS, 'ts-0001-aa-target.md')

  const replies = await Promise.all(
    blockers.map(async (to) => {
      const client = await spawnProvider(root)
      const reply = await applyWithRetry(client, 'link', 'ts-0001-aa', { to })
      await client.close()
      return reply
    }),
  )

  const linkCount = (reply: Record<string, any>): number => reply.task.links.filter((link: { type: string }) => link.type === 'blocked-by').length
  expect(replies.map(linkCount).sort()).toEqual([1, 2, 3, 4, 5, 6])
  expect(new Set(replies.map((reply) => reply.version)).size).toBe(blockers.length)
  expect(replies.find((reply) => linkCount(reply) === 6)?.version).toBe(fileVersion(file))
  const onDisk = readFileSync(file, 'utf8')
  for (const to of blockers) expect(onDisk).toContain(`- { type: blocked-by, to: ${to} }\n`)
  expect(onDisk.match(/blocked-by/g)).toHaveLength(blockers.length)
}, 30_000)

test('a writer killed between the temp write and the rename leaves the old file whole, and the next writer carries on', async () => {
  const root = makeRepo({ 'ep-001-aa/epic.md': EPIC_FILE, 'ep-001-aa/tasks/ts-0001-aa-one.md': item('ts-0001-aa') })
  const dir = join(root, TASKS)
  const file = join(dir, 'ts-0001-aa-one.md')
  const before = readFileSync(file)
  const mode = statSync(file).mode

  const doomed = await spawnProvider(root, { COLLIDE_FAILPOINT: 'before-rename' })
  await expect(doomed.request('apply', { op: 'setStatus', id: 'ts-0001-aa', args: { category: 'done' }, version: fileVersion(file) })).rejects.toThrow('before it answered')
  expect(await doomed.exited).toEqual({ code: null, signal: 'SIGKILL' })

  expect(readFileSync(file).equals(before)).toBe(true)
  expect(statSync(file).mode).toBe(mode)
  // The failpoint was reached: a synced temp file and the dead writer's lock are still there.
  const temps = readdirSync(dir).filter((name) => name.endsWith('.collide-tmp'))
  expect(temps).toHaveLength(1)
  expect(readFileSync(join(dir, temps[0] as string), 'utf8')).toContain('status: done')
  expect(existsSync(join(root, PLANNING, '.write-lock'))).toBe(true)

  const next = await spawnProvider(root)
  expect((await next.request('list')).invalid).toEqual([])
  const reply = await next.request('apply', { op: 'setStatus', id: 'ts-0001-aa', args: { category: 'active' }, version: fileVersion(file) })
  await next.close()
  expect(reply).toMatchObject({ outcome: 'ok', task: { state: { category: 'active' } } })
  expect(readFileSync(file, 'utf8')).toBe(before.toString().replace('status: ready', 'status: active'))
  expect(readdirSync(dir)).toEqual(['ts-0001-aa-one.md'])
  expect(readdirSync(join(root, PLANNING)).sort()).toEqual(['epics'])
}, 30_000)

test('with COLLIDE_FAILPOINT set to a name that is not a failpoint, a write goes through', async () => {
  const root = makeRepo({ 'ep-001-aa/epic.md': EPIC_FILE, 'ep-001-aa/tasks/ts-0001-aa-one.md': item('ts-0001-aa') })
  const client = await spawnProvider(root, { COLLIDE_FAILPOINT: 'somewhere-else' })
  const reply = await applyWithRetry(client, 'setStatus', 'ts-0001-aa', { category: 'done' })
  await client.close()
  expect(reply.outcome).toBe('ok')
})
