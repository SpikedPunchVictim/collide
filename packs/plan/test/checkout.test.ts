import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { call, commitAll, EPIC_FILE, fileVersion, git, makeRepo, tempDir } from './helpers.ts'

const PLANNING = '.collide/planning'
const TASKS = `${PLANNING}/epics/ep-001-aa-alpha/tasks`
const ONE = `${TASKS}/ts-0001-aa-one.md`

const task = (id: string, status = 'ready'): string => `---\nid: ${id}\nkind: task\nstatus: ${status}\ntitle: "Item ${id}"\nowners: []\nlinks: []\n---\n\n## Goal\n\nMade-up content.\n`

interface Pair {
  main: string
  linked: string
}

/** A repository with one committed item, and a linked worktree of it on its own branch. */
function withWorktree(): Pair {
  const main = makeRepo({ 'ep-001-aa-alpha/epic.md': EPIC_FILE, 'ep-001-aa-alpha/tasks/ts-0001-aa-one.md': task('ts-0001-aa') })
  git(main, 'init', '--quiet')
  commitAll(main, 'fixture')
  const linked = join(tempDir(), 'linked')
  git(main, 'worktree', 'add', '--quiet', '-b', 'side', linked)
  return { main, linked }
}

const read = (root: string, path = ONE): string => readFileSync(join(root, path), 'utf8')
const setStatus = (root: string, version: string, extra: Record<string, unknown>) =>
  call(root, 'apply', { op: 'setStatus', id: 'ts-0001-aa', args: { category: 'active' }, version, ...extra })

describe('apply with a checkout', () => {
  test('setStatus is found, checked and written in that checkout, and the main checkout is untouched', async () => {
    const { main, linked } = withWorktree()
    const before = read(main)
    const { result } = await setStatus(main, fileVersion(join(linked, ONE)), { checkout: linked })
    expect(result).toMatchObject({ outcome: 'ok', files: [ONE], task: { id: 'ts-0001-aa', state: { status: 'active' } } })
    expect(read(linked)).toContain('status: active')
    expect(read(main)).toBe(before)
    expect(git(main, 'status', '--porcelain')).toBe('')
    expect(git(linked, 'status', '--porcelain')).toBe(` M ${ONE}\n`)
  })

  test('the version is the checkout\'s own: a file that differs there is a conflict, and the checkout\'s version is accepted', async () => {
    const { main, linked } = withWorktree()
    writeFileSync(join(linked, ONE), task('ts-0001-aa', 'review'))
    const stale = await setStatus(main, fileVersion(join(main, ONE)), { checkout: linked })
    expect(stale.result).toMatchObject({ outcome: 'conflict', current: { state: { status: 'review' } } })
    expect(read(linked)).toContain('status: review')
    const fresh = await setStatus(main, fileVersion(join(linked, ONE)), { checkout: linked })
    expect(fresh.result).toMatchObject({ outcome: 'ok' })
    expect(read(linked)).toContain('status: active')
  })

  test('the checkout may be the main checkout itself, and a symbolic link to a checkout', async () => {
    const { main, linked } = withWorktree()
    const alias = join(tempDir(), 'alias')
    symlinkSync(linked, alias)
    expect((await setStatus(main, fileVersion(join(linked, ONE)), { checkout: alias })).result).toMatchObject({ outcome: 'ok' })
    expect((await setStatus(main, fileVersion(join(main, ONE)), { checkout: main })).result).toMatchObject({ outcome: 'ok' })
    expect(read(main)).toContain('status: active')
  })

  test('a path that is not a checkout of the repository is refused, and nothing is written', async () => {
    const { main, linked } = withWorktree()
    const stranger = tempDir()
    mkdirSync(join(stranger, PLANNING), { recursive: true })
    const inside = join(linked, PLANNING)
    const cases: Array<[string, string]> = [
      ['an unrelated folder', stranger],
      ['a folder inside a checkout', inside],
      ['a path that does not exist', join(stranger, 'nope')],
      ['a relative path', 'linked'],
      ['an empty path', ''],
    ]
    for (const [name, checkout] of cases) {
      const { result } = await setStatus(main, fileVersion(join(main, ONE)), { checkout })
      expect(result, name).toMatchObject({ outcome: 'refused', message: expect.stringContaining('checkout') })
    }
    expect(git(main, 'status', '--porcelain')).toBe('')
    expect(git(linked, 'status', '--porcelain')).toBe('')
    expect(readdirSync(join(stranger, PLANNING))).toEqual([])
  })

  test('a checkout of another repository is refused', async () => {
    const { main } = withWorktree()
    const other = makeRepo({ 'ep-001-aa-alpha/epic.md': EPIC_FILE, 'ep-001-aa-alpha/tasks/ts-0001-aa-one.md': task('ts-0001-aa') })
    git(other, 'init', '--quiet')
    commitAll(other, 'other')
    const { result } = await setStatus(main, fileVersion(join(other, ONE)), { checkout: other })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('not a checkout of this repository') })
    expect(read(other)).toContain('status: ready')
  })

  test('each checkout has its own lock file: a lock held in the main checkout does not hold up a write in the other', async () => {
    const { main, linked } = withWorktree()
    const lock = join(main, PLANNING, '.write-lock')
    writeFileSync(lock, `${process.pid}\n`)
    try {
      const { result } = await setStatus(main, fileVersion(join(linked, ONE)), { checkout: linked })
      expect(result).toMatchObject({ outcome: 'ok' })
      expect(existsSync(join(linked, PLANNING, '.write-lock'))).toBe(false)
    } finally {
      rmSync(lock)
    }
  })

  test('create writes the new item into that checkout, numbered past every ref', async () => {
    const { main, linked } = withWorktree()
    git(main, 'checkout', '--quiet', '-b', 'elsewhere')
    writeFileSync(join(main, TASKS, 'ts-0040-zz-far-ahead.md'), task('ts-0040-zz'))
    commitAll(main, 'a high number on another branch')
    git(main, 'checkout', '--quiet', 'main')
    const { result } = await call(main, 'apply', { op: 'create', checkout: linked, args: { title: 'Made in the worktree', epic: 'ep-001-aa', seed: 1 } })
    expect(result).toMatchObject({ outcome: 'ok', task: { id: expect.stringMatching(/^ts-0041-[a-z]{2}$/) } })
    const [file] = (result as { files: string[] }).files
    expect(file).toMatch(new RegExp(`^${TASKS}/ts-0041-[a-z]{2}-made-in-the-worktree\\.md$`))
    expect(existsSync(join(linked, file as string))).toBe(true)
    expect(existsSync(join(main, file as string))).toBe(false)
    expect(git(main, 'status', '--porcelain')).toBe('')
  })

  test('create needs the epic in that checkout\'s working tree', async () => {
    const { main, linked } = withWorktree()
    const { result } = await call(main, 'apply', { op: 'create', checkout: linked, args: { title: 'Nowhere', epic: 'ep-009-zz' } })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('ep-009-zz') })
  })

  test('reid renames and rewrites in that checkout only', async () => {
    const { main, linked } = withWorktree()
    const { result } = await call(main, 'apply', { op: 'reid', id: 'ts-0001-aa', checkout: linked, args: { seed: 1 } })
    expect(result).toMatchObject({ outcome: 'ok', files: expect.arrayContaining([ONE]) })
    expect(existsSync(join(linked, ONE))).toBe(false)
    expect(existsSync(join(main, ONE))).toBe(true)
    expect(git(main, 'status', '--porcelain')).toBe('')
  })
})

describe('list and get with a checkout', () => {
  test('read that checkout, and without one read the main checkout', async () => {
    const { main, linked } = withWorktree()
    writeFileSync(join(linked, ONE), task('ts-0001-aa', 'review'))
    const inLinked = await call(main, 'get', { id: 'ts-0001-aa', checkout: linked })
    expect(inLinked.result).toMatchObject({ task: { state: { status: 'review' }, version: fileVersion(join(linked, ONE)) } })
    const inMain = await call(main, 'get', { id: 'ts-0001-aa' })
    expect(inMain.result).toMatchObject({ task: { state: { status: 'ready' } } })
    const listed = await call(main, 'list', { checkout: linked })
    expect((listed.result as { tasks: Array<{ id: string; state: { status: string } }> }).tasks.find((t) => t.id === 'ts-0001-aa')?.state.status).toBe('review')
  })

  test('a path that is not a checkout is an error reply', async () => {
    const { main } = withWorktree()
    const stranger = tempDir()
    expect((await call(main, 'list', { checkout: stranger })).error).toMatchObject({ message: expect.stringContaining('not a checkout of this repository') })
    expect((await call(main, 'get', { id: 'ts-0001-aa', checkout: stranger })).error).toMatchObject({ message: expect.stringContaining('not a checkout of this repository') })
  })
})
