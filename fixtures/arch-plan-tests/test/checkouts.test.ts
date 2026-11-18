import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, describe, expect, test } from 'vitest'
import { gitDir, gitHead, gitRenames } from '../../../fixtures/packs/orchard/providers/plan/git.ts'
import { readIdMap, renamedTo } from '../../../fixtures/packs/orchard/providers/plan/id-map.ts'
import { makeRepo, plan } from './repo.ts'
import type { Repo } from './repo.ts'

const ARCH = ['--root', 'arch']
const STATUS_OF = (result: any, shortId: string) =>
  result.tasks.find((task: any) => task.key === `plan:${shortId}`)?.state.status

/** A repository and a second checkout of it, with T-03-003 `ready` in one and `doing` in the other. */
async function twoCheckouts(): Promise<{ main: Repo; other: Repo }> {
  const main = makeRepo()
  const other = main.worktree('feature')
  expect(plan(other, ['set', 'T-03-003', 'status=doing', ...ARCH]).code).toBe(0)
  return { main, other }
}

describe('list and get across checkouts', () => {
  test('each checkout gives its own state for the same task, under the same key', async () => {
    const { main, other } = await twoCheckouts()
    const client = await main.provider()
    try {
      const here = (await client.request('list')).result as any
      const there = (await client.request('list', { checkout: other.root })).result as any
      expect(STATUS_OF(here, 'T-03-003')).toBe('ready')
      expect(STATUS_OF(there, 'T-03-003')).toBe('doing')
      const a = here.tasks.find((task: any) => task.key === 'plan:T-03-003')
      const b = there.tasks.find((task: any) => task.key === 'plan:T-03-003')
      expect(a).toMatchObject({ checkout: main.root, path: main.taskPath('T-03-003') })
      expect(b).toMatchObject({ checkout: other.root, path: other.taskPath('T-03-003') })
      expect(a.path).toBe(b.path)
      expect(a.version).not.toBe(b.version)
    } finally {
      await client.close()
    }
  })

  test('every task and invalid file of a listing names the checkout it was read in', async () => {
    const { main, other } = await twoCheckouts()
    other.write(other.taskPath('T-03-001'), 'no frontmatter at all\n')
    const client = await main.provider()
    try {
      const there = (await client.request('list', { checkout: other.root })).result as any
      expect(there.tasks.every((task: any) => task.checkout === other.root)).toBe(true)
      expect(there.invalid).toEqual([
        { path: other.taskPath('T-03-001'), checkout: other.root, reason: expect.any(String) },
      ])
    } finally {
      await client.close()
    }
  })

  test('get takes a checkout, and finds the task as that checkout has it', async () => {
    const { main, other } = await twoCheckouts()
    const client = await main.provider()
    try {
      const here = (await client.request('get', { id: 'T-03-003' })).result as any
      const there = (await client.request('get', { id: 'T-03-003', checkout: other.root })).result as any
      expect(here.task).toMatchObject({ checkout: main.root, state: { status: 'ready' } })
      expect(there.task).toMatchObject({ checkout: other.root, state: { status: 'doing' } })
    } finally {
      await client.close()
    }
  })

  test.each([
    ['a path that is not absolute', 'feature'],
    ['a path that is not there', join(tmpdir(), 'arch-plan-no-such-checkout')],
  ])('refuses %s as the checkout of a list, a get and a write', async (_name, checkout) => {
    const main = makeRepo()
    const client = await main.provider()
    try {
      const replies = [
        await client.request('list', { checkout }),
        await client.request('get', { id: 'T-03-003', checkout }),
        await client.request('apply', {
          op: 'setStatus',
          id: 'T-03-003',
          args: { status: 'doing' },
          version: 'x',
          checkout,
        }),
      ]
      for (const reply of replies) expect(reply.error?.message).toMatch(/checkout/)
    } finally {
      await client.close()
    }
  })

  test('a write names its checkout: it runs that checkout\'s own plan and changes that checkout only', async () => {
    const { main, other } = await twoCheckouts()
    const before = main.snapshot()
    const client = await main.provider()
    try {
      const version = (await client.request('get', { id: 'T-03-002', checkout: other.root })).result as any
      const reply = await client.request('apply', {
        op: 'setStatus',
        id: 'T-03-002',
        args: { status: 'in_review' },
        version: version.task.version,
        checkout: other.root,
      })
      expect(reply.result).toMatchObject({ outcome: 'ok', task: { checkout: other.root } })
      expect(other.read(other.taskPath('T-03-002'))).toContain('status: in_review')
      expect(main.snapshot()).toEqual(before)
    } finally {
      await client.close()
    }
  })
})

describe('the id map', () => {
  const temp = mkdtempSync(join(tmpdir(), 'arch-plan-idmap-'))
  afterAll(() => rmSync(temp, { recursive: true, force: true }))
  const read = async (text: string | null): Promise<Array<{ from: string; to: string }>> => {
    const root = mkdtempSync(join(temp, 'root-'))
    if (text !== null) {
      mkdirSync(join(root, 'arch'))
      writeFileSync(join(root, 'arch', 'ID-MAP.md'), text)
    }
    return readIdMap(root, () => undefined)
  }

  test('has rows of old and new ids, each bare or with its slug, and skips every other line', async () => {
    const rows = await read(
      [
        '# Heading',
        '',
        '| old id | new id | renamed | detail |',
        '|---|---|---|---|',
        '| T-05-009-wire-the-bus | T-05-010-wire-the-bus | 2026-01-05 | moved: proposals/05-x/tasks/a.md |',
        '| T-05-011 | T-05-012 | 2026-01-06 | moved: b |',
        '| not an id | T-05-013 | day | note |',
        '| T-05-014 |',
        'T-05-015 | T-05-016 | no leading bar | x',
        '',
      ].join('\n'),
    )
    expect(rows).toEqual([
      { from: 'T-05-009', to: 'T-05-010' },
      { from: 'T-05-011', to: 'T-05-012' },
    ])
  })

  test('is empty when the file is missing', async () => {
    expect(await read(null)).toEqual([])
  })

  test('is followed to its end: a chain of renumbers is one hop, and a loop stops where it began', () => {
    const rows = [
      { from: 'T-05-001', to: 'T-05-002' },
      { from: 'T-05-002', to: 'T-05-003' },
    ]
    expect(renamedTo(rows, 'T-05-001')).toBe('T-05-003')
    expect(renamedTo(rows, 'T-05-002')).toBe('T-05-003')
    expect(renamedTo(rows, 'T-05-003')).toBeNull()
    expect(renamedTo([...rows, { from: 'T-05-003', to: 'T-05-001' }], 'T-05-001')).toBe('T-05-003')
  })
})

describe('git', () => {
  test('names the commit HEAD is at and the folder that holds its HEAD file, in a worktree too', () => {
    const main = makeRepo()
    const other = main.worktree('feature')
    return Promise.all([
      gitHead(main.root, main.env),
      gitDir(main.root, main.env),
      gitDir(other.root, other.env),
    ]).then(([head, mainDir, otherDir]) => {
      expect(head).toBe(main.head())
      expect(mainDir?.endsWith('/.git')).toBe(true)
      expect(otherDir?.endsWith('/.git/worktrees/feature')).toBe(true)
    })
  })

  test('has no HEAD and no folder for a folder that is no repository', async () => {
    const main = makeRepo()
    const bare = mkdtempSync(join(tmpdir(), 'arch-plan-norepo-'))
    try {
      const env = { ...main.env, GIT_CEILING_DIRECTORIES: tmpdir() }
      expect(await gitHead(bare, env)).toBeNull()
      expect(await gitDir(bare, env)).toBeNull()
    } finally {
      rmSync(bare, { recursive: true, force: true })
    }
  })

  test('lists the renames git sees under arch/ between two commits, and no other change', async () => {
    const main = makeRepo()
    const from = main.head()
    const old = main.taskPath('T-03-003')
    main.git('mv', old, old.replace('T-03-003', 'T-03-050'))
    main.write(main.taskPath('T-03-001'), `${main.read(main.taskPath('T-03-001'))}\nOne more line.\n`)
    main.git('add', '-A')
    main.git('commit', '--quiet', '-m', 'rename one, edit one')
    expect(await gitRenames(main.root, from, main.head(), main.env)).toEqual([
      { from: old, to: old.replace('T-03-003', 'T-03-050') },
    ])
  })

  test('has no renames for a commit it does not know', async () => {
    const main = makeRepo()
    expect(await gitRenames(main.root, '0'.repeat(40), main.head(), main.env)).toEqual([])
  })
})
