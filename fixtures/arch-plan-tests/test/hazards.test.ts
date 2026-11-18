import { mkdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { call, connect, copyTree, keysOf, listAll, tree } from './helpers.ts'

const HAZARDS = tree('hazards')
const DIR = 'arch/proposals/04-hazard-cases/tasks'

const invalidOf = async (root: string, file: string) => {
  const result = await listAll(root)
  return (result.invalid as Array<Record<string, any>>).find((entry) => entry.path === `${DIR}/${file}`)
}

describe('hazard files go to invalid[] with the path, the checkout and a reason', () => {
  const cases: Array<[string, string, RegExp]> = [
    ['a conflict-marker file', 'T-04-002-conflict-markers.md', /conflict marker/i],
    ['bad YAML', 'T-04-003-bad-yaml.md', /not valid YAML/],
    ['a duplicate key', 'T-04-004-duplicate-key.md', /unique|duplicate/i],
    ['an anchor', 'T-04-005-anchor.md', /anchor/i],
    ['an alias', 'T-04-006-alias.md', /anchor|alias/i],
    ['a merge key', 'T-04-007-merge-key.md', /merge/i],
    ['a title that is not a string', 'T-04-008-number-title.md', /title/],
    ['a dependency that is not a string', 'T-04-009-number-dependency.md', /depends_on/],
    ['a dependency that is not a task id', 'T-04-010-not-a-task-dependency.md', /something-else/],
    ['a status the adapter did not declare', 'T-04-011-undeclared-status.md', /status "blocked"/],
    ['a file with no frontmatter', 'T-04-012-no-frontmatter.md', /no frontmatter/],
    ['a missing status', 'T-04-014-missing-status.md', /status/],
    ['an id that names another proposal', 'T-09-001-wrong-scope.md', /proposal/],
  ]
  test.each(cases)('%s', async (_name, file, reason) => {
    const entry = await invalidOf(HAZARDS, file)
    expect(entry).toBeDefined()
    expect(entry?.checkout).toBe(HAZARDS)
    expect(entry?.reason).toMatch(reason)
  })

  test('two files with one short id are both invalid and neither lists', async () => {
    const result = await listAll(HAZARDS)
    for (const file of ['T-04-013-twin-a.md', 'T-04-013-twin-b.md']) {
      expect((await invalidOf(HAZARDS, file))?.reason).toMatch(/T-04-013.*another file|another file.*T-04-013/)
    }
    expect(keysOf(result)).not.toContain('plan:T-04-013')
  })

  test('the other tasks still list, and a file not named like a task is neither listed nor invalid', async () => {
    const result = await listAll(HAZARDS)
    expect(keysOf(result)).toEqual(['plan:T-04-001'])
    expect((result.invalid as Array<Record<string, any>>).map((entry) => entry.path)).not.toContain(`${DIR}/notes.md`)
    expect(result.invalid).toHaveLength(15)
  })

  test('get answers no task for an invalid file and for a twin', async () => {
    expect((await call(HAZARDS, 'get', { id: 'T-04-003' })).result).toEqual({})
    expect((await call(HAZARDS, 'get', { id: 'T-04-013' })).result).toEqual({})
  })

  test('an epic file that cannot be read is logged and skipped, and the list still answers', async () => {
    const client = connect()
    try {
      await client.request('initialize', { config: {}, root: HAZARDS })
      const reply = await client.request('list')
      expect(reply.result?.collections.map((c: any) => c.id)).toEqual(['plan:E-09'])
      expect(reply.logs.join('\n')).toMatch(/E-10-broken-epic\.md/)
    } finally {
      await client.close()
    }
  })
})

describe('the caps', () => {
  const write = (root: string, name: string, text: string) => {
    const dir = join(root, 'arch/proposals/06-cap-cases/tasks')
    mkdirSync(dir, { recursive: true })
    writeFileSync(join(dir, name), text)
    return `arch/proposals/06-cap-cases/tasks/${name}`
  }
  const head = 'title: "Cap case"\nstatus: ready\nepic: E-09\ndepends_on: []\nblocked_by: []'

  test('a frontmatter block over 64 KiB is invalid, and says the cap', async () => {
    const root = copyTree('hazards')
    const notes = 'x'.repeat(70 * 1024)
    const path = write(root, 'T-06-001-big-frontmatter.md', `---\n${head}\nnotes: "${notes}"\n---\n\nBody.\n`)
    const entry = (await listAll(root)).invalid.find((item: any) => item.path === path)
    expect(entry.reason).toMatch(/over the 65536-byte cap/)
  })

  test('a body over its cap is invalid and says the cap; list does not read it', async () => {
    const root = copyTree('hazards')
    const path = write(root, 'T-06-002-big-body.md', `---\n${head}\n---\n\n${'x'.repeat(1024 * 1024 + 10)}\n`)
    const entry = (await listAll(root)).invalid.find((item: any) => item.path === path)
    expect(entry.reason).toMatch(/body is \d+ bytes, over the 1048576-byte cap/)
  })

  test('a body just under its cap is a task, and get returns all of it', async () => {
    const root = copyTree('hazards')
    const body = 'y'.repeat(1024 * 1024 - 100)
    write(root, 'T-06-003-large-body.md', `---\n${head}\n---\n${body}`)
    const { result } = await call(root, 'get', { id: 'T-06-003' })
    expect(result?.task.body).toBe(body)
    expect(keysOf(await listAll(root))).toContain('plan:T-06-003')
  })

  test('a file that is not UTF-8 is invalid', async () => {
    const root = copyTree('hazards')
    const path = write(root, 'T-06-004-not-utf8.md', '---\ntitle: "x"\n---\n')
    const text = Buffer.from(`---\n${head}\n---\n`)
    writeFileSync(join(root, path), Buffer.concat([text, Buffer.from([0xff, 0xfe, 0xfd])]))
    const entry = (await listAll(root)).invalid.find((item: any) => item.path === path)
    expect(entry.reason).toMatch(/UTF-8/)
  })
})
