import { existsSync, mkdirSync, readdirSync, readFileSync, symlinkSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { idsInPaths } from '../src/ids.ts'
import { callUninitialized, commitAll, connect, EPIC_FILE, fileVersion, frontmatter, git, tempDir } from './helpers.ts'
import type { Reply } from './helpers.ts'

const DEFAULT_ROOT = '.collide/planning'
const EPIC = 'ep-001-aa'

/** A git repository holding `files` (paths from the repository root), committed on main. */
function repoOf(files: Record<string, string>): string {
  const dir = tempDir()
  for (const [path, text] of Object.entries({ 'README.md': 'fixture\n', ...files })) {
    mkdirSync(dirname(join(dir, path)), { recursive: true })
    writeFileSync(join(dir, path), text)
  }
  git(dir, 'init', '--quiet')
  commitAll(dir, 'fixture')
  return dir
}

/** The files of an epic `ep-001-aa-alpha` under `root`, with one task per id. */
function epicAt(root: string, ...taskIds: string[]): Record<string, string> {
  const files: Record<string, string> = { [`${root}/epics/ep-001-aa-alpha/epic.md`]: EPIC_FILE }
  for (const id of taskIds) files[`${root}/epics/ep-001-aa-alpha/tasks/${id}-made-up.md`] = frontmatter(id)
  return files
}

/** One request to a provider that `initialize` has given `root` and `layout` (no `layout` key when it is undefined). */
async function callWith(root: string, layout: unknown, method: string, params: unknown = {}): Promise<Reply> {
  const client = connect()
  try {
    const init = await client.request('initialize', { config: layout === undefined ? {} : { layout }, root })
    if (init.error !== undefined) throw new Error(`initialize failed: ${JSON.stringify(init.error)}`)
    return await client.request(method, params)
  } finally {
    await client.close()
  }
}

/** The reply to `initialize` alone. */
async function initializeWith(root: string, layout: unknown): Promise<Reply> {
  const client = connect()
  try {
    return await client.request('initialize', { config: { layout }, root })
  } finally {
    await client.close()
  }
}

const idsOf = (reply: Reply): string[] => (reply.result?.tasks as Array<{ id: string }>).map((task) => task.id).sort()
const pathsOf = (reply: Reply): string[] =>
  (reply.result?.tasks as Array<{ path: string }>).map((task) => task.path).sort()
const create = (root: string, layout: unknown, args: Record<string, unknown>) =>
  callWith(root, layout, 'apply', { op: 'create', args })

/** `setStatus` to `category` on the item `id`, whose file is `file` (a path from the repository root). */
const setStatus = (root: string, layout: unknown, id: string, file: string, category: string) =>
  callWith(root, layout, 'apply', {
    op: 'setStatus',
    id,
    version: fileVersion(join(root, file)),
    args: { category },
  })

const NEW_TASK = 'docs/plan/epics/ep-001-aa-alpha/tasks/ts-0001-aa-made-up.md'
const OLD_TASK = `${DEFAULT_ROOT}/epics/ep-002-bb-older/tasks/ts-0001-aa-old.md`

describe('a path that layout may not use is refused at initialize', () => {
  const cases: Array<[string, string, string?]> = [
    ['../outside', '".."'],
    ['docs/../../outside', '".."'],
    ['/abs/path', 'absolute'],
    ['', 'empty'],
    ['.', 'empty'],
    ['.git/planning', '.git'],
    ['.git', '.git'],
    ['.GIT/planning', '.git'],
    ['docs\\sub', 'backslash'],
    ['.collide/config.yaml', 'protected', '.collide/config.yaml'],
    ['.collide/board.yaml', 'protected', '.collide/board.yaml'],
    ['.collide/packs.lock', 'protected', '.collide/packs.lock'],
    ['.collide/views', 'protected', '.collide/views'],
    ['.collide/views/mine/deeper', 'protected', '.collide/views'],
    ['.collide/profiles/', 'protected', '.collide/profiles'],
    ['./.collide/views', 'protected', '.collide/views'],
    ['.Collide/Config.yaml', 'protected', '.collide/config.yaml'],
  ]

  test.each(cases)('root %j is refused for its rule', async (root, rule, covered) => {
    const reply = await initializeWith(repoOf({}), { root })
    expect(reply.result).toBeUndefined()
    expect(reply.error).toMatchObject({ code: -32603, message: expect.stringContaining('layout.root') })
    expect(reply.error?.message).toContain(rule)
    if (covered !== undefined) expect(reply.error?.message).toContain(`would cover ${covered}`)
  })

  test('a root that only starts like a protected name, or sits above one, is not refused', async () => {
    const roots = ['.collide/views-old', '.collide/config.yaml.d', '.collide', '.github/planning', 'git/planning']
    for (const root of roots) {
      const reply = await initializeWith(repoOf({}), { root })
      expect(reply.error, root).toBeUndefined()
    }
  })

  test('a root with a NUL or another control character is refused', async () => {
    const reply = await initializeWith(repoOf({}), { root: 'docs/pl\u0000an' })
    expect(reply.error?.message).toContain('control character')
  })

  test('a refused initialize leaves a new provider without a root', async () => {
    const client = connect()
    try {
      const refused = await client.request('initialize', { config: { layout: { root: '../x' } }, root: repoOf({}) })
      expect(refused.error).toBeDefined()
      expect((await client.request('list')).error?.message).toContain('send initialize')
    } finally {
      await client.close()
    }
  })

  test('a layout that is not an object, or has a key it does not know, is refused naming the key', async () => {
    const repo = repoOf({})
    expect((await initializeWith(repo, 'docs/plan')).error?.message).toContain('layout must be an object')
    expect((await initializeWith(repo, { rooot: 'docs/plan' })).error?.message).toContain('layout.rooot')
    expect((await initializeWith(repo, { root: 7 })).error?.message).toContain('layout.root must be text')
    expect((await initializeWith(repo, { previous: 'old' })).error?.message).toContain('layout.previous must be a list')
  })
})

describe('a symbolic link in the path', () => {
  test('one that leaves the repository is refused, naming the rule and the link', async () => {
    const repo = repoOf({})
    symlinkSync(tempDir(), join(repo, 'link'))
    const reply = await initializeWith(repo, { root: 'link/plan' })
    expect(reply.error?.message).toContain('layout.root')
    expect(reply.error?.message).toContain('symbolic link')
    expect(reply.error?.message).toContain('"link"')
    expect(reply.error?.message).toContain('leaves the repository')
  })

  test('a chain of links that ends outside is refused, and so is a link that points at nothing', async () => {
    const repo = repoOf({})
    symlinkSync(tempDir(), join(repo, 'far'))
    symlinkSync('far', join(repo, 'near'))
    symlinkSync('missing-target', join(repo, 'dangling'))
    expect((await initializeWith(repo, { root: 'near' })).error?.message).toContain('leaves the repository')
    expect((await initializeWith(repo, { root: 'dangling/plan' })).error?.message).toContain('symbolic link')
  })

  test('a link inside the repository that ends at .git or at a protected folder is refused as that', async () => {
    const repo = repoOf({})
    mkdirSync(join(repo, '.collide/views'), { recursive: true })
    symlinkSync('.git', join(repo, 'hidden'))
    symlinkSync('.collide/views', join(repo, 'shortcut'))
    expect((await initializeWith(repo, { root: 'hidden/planning' })).error?.message).toContain('.git')
    const viaView = await initializeWith(repo, { root: 'shortcut' })
    expect(viaView.error?.message).toContain('protected')
    expect(viaView.error?.message).toContain('would cover .collide/views')
  })

  test('a link that stays inside the repository is accepted and its items are listed', async () => {
    const repo = repoOf(epicAt('docs/plan', 'ts-0001-aa'))
    symlinkSync('docs', join(repo, 'alias'))
    const reply = await callWith(repo, { root: 'alias/plan' }, 'list')
    expect(idsOf(reply)).toEqual([EPIC, 'ts-0001-aa'])
    // The link is resolved, so a path is the one git knows.
    expect(pathsOf(reply).every((path) => path.startsWith('docs/plan/epics/'))).toBe(true)
  })

  test('the same rule holds for a previous entry and for paths.spike', async () => {
    const repo = repoOf({})
    symlinkSync(tempDir(), join(repo, 'link'))
    const previous = await initializeWith(repo, { previous: ['link/old'] })
    expect(previous.error?.message).toContain('layout.previous[0]')
    expect(previous.error?.message).toContain('symbolic link')
    const spike = await initializeWith(repo, { paths: { spike: 'link/spikes' } })
    expect(spike.error?.message).toContain('layout.paths.spike')
    expect(spike.error?.message).toContain('symbolic link')
  })
})

describe('previous, paths and code are checked like root', () => {
  test('a previous entry is refused for each rule, naming its place in the list', async () => {
    const repo = repoOf({})
    const reply = await initializeWith(repo, { previous: ['old/plan', '.git/old', '.collide/views'] })
    expect(reply.error?.message).toContain('layout.previous[1]')
    expect(reply.error?.message).toContain('.git')
    const protectedOne = await initializeWith(repo, { previous: ['old/plan', '.collide/views'] })
    expect(protectedOne.error?.message).toContain('layout.previous[1]')
    expect(protectedOne.error?.message).toContain('would cover .collide/views')
    expect((await initializeWith(repo, { previous: ['/old'] })).error?.message).toContain('absolute')
    expect((await initializeWith(repo, { previous: ['../old'] })).error?.message).toContain('".."')
  })

  test('only spike can be placed: another kind is refused with the reason', async () => {
    const reply = await initializeWith(repoOf({}), { paths: { spike: 'docs/decisions', task: 'docs/tasks' } })
    expect(reply.error?.message).toContain('layout.paths.task')
    expect(reply.error?.message).toContain('only spike can be placed')
    expect(reply.error?.message).toContain("the other kinds live in their epic's folder")
  })

  test('paths.spike is checked, and a good one is accepted', async () => {
    const repo = repoOf({})
    expect((await initializeWith(repo, { paths: { spike: '../x' } })).error?.message).toContain('layout.paths.spike')
    const overProfiles = await initializeWith(repo, { paths: { spike: '.collide/profiles/x' } })
    expect(overProfiles.error?.message).toContain('would cover .collide/profiles')
    expect((await initializeWith(repo, { paths: { spike: 'docs/decisions' } })).error).toBeUndefined()
    expect((await initializeWith(repo, { paths: 'docs' })).error?.message).toContain('layout.paths must be an object')
    const notText = await initializeWith(repo, { paths: { spike: 3 } })
    expect(notText.error?.message).toContain('layout.paths.spike must be text')
  })

  test('code is accepted and changes nothing: the listing is what it is without it', async () => {
    const repo = repoOf(epicAt(DEFAULT_ROOT, 'ts-0001-aa'))
    const without = await callWith(repo, {}, 'list')
    const withCode = await callWith(repo, { code: 'spikes', paths: { spike: 'docs/decisions' } }, 'list')
    expect(withCode.result).toEqual(without.result)
    expect((await initializeWith(repo, { code: '../spikes' })).error?.message).toContain('layout.code')
    expect((await initializeWith(repo, { code: 4 })).error?.message).toContain('layout.code must be text')
  })
})

describe('with no layout nothing changes', () => {
  test('no config, an empty layout and the default root named out loud all give the default root', async () => {
    const repo = repoOf(epicAt(DEFAULT_ROOT, 'ts-0001-aa'))
    const none = await callWith(repo, undefined, 'list')
    expect(idsOf(none)).toEqual([EPIC, 'ts-0001-aa'])
    expect(pathsOf(none)).toContain(`${DEFAULT_ROOT}/epics/ep-001-aa-alpha/tasks/ts-0001-aa-made-up.md`)
    for (const layout of [{}, { root: DEFAULT_ROOT }, { root: `./${DEFAULT_ROOT}/` }]) {
      expect((await callWith(repo, layout, 'list')).result).toEqual(none.result)
    }
  })

  test('create goes to the default root', async () => {
    const repo = repoOf(epicAt(DEFAULT_ROOT, 'ts-0001-aa'))
    const { result } = await create(repo, {}, { epic: EPIC, title: 'New one', seed: 1 })
    expect(result?.files).toEqual([`${DEFAULT_ROOT}/epics/ep-001-aa-alpha/tasks/ts-0002-ra-new-one.md`])
  })
})

describe('layout.root: docs/plan', () => {
  const layout = { root: 'docs/plan' }
  const strayDefault = epicAt(DEFAULT_ROOT, 'ts-0090-zz')

  test('lists the items under docs/plan/epics/ and not the ones left under the default root', async () => {
    const repo = repoOf({ ...epicAt('docs/plan', 'ts-0001-aa', 'ts-0002-bb'), ...strayDefault })
    const reply = await callWith(repo, layout, 'list')
    expect(idsOf(reply)).toEqual([EPIC, 'ts-0001-aa', 'ts-0002-bb'])
    expect(reply.invalid).toEqual([])
    expect(pathsOf(reply).every((path) => path.startsWith('docs/plan/epics/'))).toBe(true)
  })

  test('get finds an item under the new root, with its path', async () => {
    const repo = repoOf(epicAt('docs/plan', 'ts-0001-aa'))
    const { result } = await callWith(repo, layout, 'get', { id: 'ts-0001-aa' })
    expect(result?.task.path).toBe('docs/plan/epics/ep-001-aa-alpha/tasks/ts-0001-aa-made-up.md')
    expect(result?.task.links).toEqual([{ type: 'part-of', to: EPIC }])
  })

  test('create writes a task and an epic under docs/plan and nothing under the default root', async () => {
    const repo = repoOf(epicAt('docs/plan', 'ts-0001-aa'))
    const task = await create(repo, layout, { epic: EPIC, title: 'New one', seed: 1 })
    expect(task.result).toMatchObject({ outcome: 'ok', task: { id: 'ts-0002-ra' } })
    expect(task.result?.files).toEqual(['docs/plan/epics/ep-001-aa-alpha/tasks/ts-0002-ra-new-one.md'])
    const epic = await create(repo, layout, { kind: 'epic', title: 'Second epic', seed: 1 })
    expect(epic.result?.files).toEqual(['docs/plan/epics/ep-002-ra-second-epic/epic.md'])
    expect(existsSync(join(repo, '.collide'))).toBe(false)
    const left = readdirSync(join(repo, 'docs/plan'), { recursive: true }).map(String)
    expect(left.filter((name) => name.endsWith('.write-lock'))).toEqual([])
    const listed = await callWith(repo, layout, 'list')
    expect(idsOf(listed)).toContain('ts-0002-ra')
    expect(listed.invalid).toEqual([])
  })

  test('the first create into a root that does not exist yet makes it', async () => {
    const repo = repoOf({})
    const made = await create(repo, layout, { kind: 'epic', title: 'First epic', seed: 1 })
    expect(made.result?.files).toEqual(['docs/plan/epics/ep-001-ra-first-epic/epic.md'])
    expect(existsSync(join(repo, '.collide'))).toBe(false)
  })

  test('a write op changes the file under docs/plan and nothing else', async () => {
    const repo = repoOf(epicAt('docs/plan', 'ts-0001-aa'))
    const { result } = await setStatus(repo, layout, 'ts-0001-aa', NEW_TASK, 'active')
    expect(result).toMatchObject({ outcome: 'ok', files: [NEW_TASK] })
    expect(readFileSync(join(repo, NEW_TASK), 'utf8')).toContain('status: active')
    expect(existsSync(join(repo, '.collide'))).toBe(false)
    expect(git(repo, 'status', '--porcelain')).toBe(` M ${NEW_TASK}\n`)
  })

  test('reid renames the item and rewrites the links under docs/plan', async () => {
    const files = epicAt('docs/plan', 'ts-0001-aa')
    const blockedBy = 'links:\n  - { type: blocked-by, to: ts-0001-aa }\n'
    files['docs/plan/epics/ep-001-aa-alpha/tasks/ts-0002-bb-blocked.md'] = frontmatter('ts-0002-bb', blockedBy).replace(
      'links: []\n',
      '',
    )
    const repo = repoOf(files)
    const { result } = await callWith(repo, layout, 'apply', { op: 'reid', id: 'ts-0001-aa', args: { seed: 1 } })
    expect(result?.outcome).toBe('ok')
    const newId = result?.task.id as string
    expect(newId).not.toBe('ts-0001-aa')
    expect(existsSync(join(repo, '.collide'))).toBe(false)
    const listed = await callWith(repo, layout, 'list')
    expect(listed.invalid).toEqual([])
    expect(idsOf(listed)).toEqual([EPIC, newId, 'ts-0002-bb'].sort())
    const tasks = listed.result?.tasks as Array<{ id: string; links: Array<{ to: string }> }>
    const blocked = tasks.find((task) => task.id === 'ts-0002-bb')
    expect(blocked?.links.map((link) => link.to)).toContain(newId)
  })

  test.each(['plans', 'a/b/c/d/plan', '.github/planning'])('a root of another depth works: %s', async (root) => {
    const repo = repoOf(epicAt(root, 'ts-0001-aa'))
    expect(idsOf(await callWith(repo, { root }, 'list'))).toEqual([EPIC, 'ts-0001-aa'])
    const made = await create(repo, { root }, { epic: EPIC, title: 'Deep one', seed: 1 })
    expect(made.result?.files).toEqual([`${root}/epics/ep-001-aa-alpha/tasks/ts-0002-ra-deep-one.md`])
  })

  test('the number is one past every id on every ref, and an id on another branch is not reused', async () => {
    const repo = repoOf(epicAt('docs/plan', 'ts-0001-aa'))
    git(repo, 'checkout', '--quiet', '-b', 'other')
    const elsewhere = join(repo, 'docs/plan/epics/ep-001-aa-alpha/tasks/ts-0007-zz-on-a-branch.md')
    writeFileSync(elsewhere, frontmatter('ts-0007-zz'))
    commitAll(repo, 'a branch item')
    git(repo, 'checkout', '--quiet', 'main')
    const made = await create(repo, layout, { epic: EPIC, title: 'After the branch', seed: 1 })
    expect(made.result?.task.id).toMatch(/^ts-0008-/)
  })

  test('the new root is read from the repository root, not from the working directory', async () => {
    const repo = repoOf(epicAt('docs/plan', 'ts-0001-aa'))
    expect(process.cwd()).not.toBe(repo)
    expect(idsOf(await callWith(repo, layout, 'list'))).toEqual([EPIC, 'ts-0001-aa'])
  })
})

describe('layout.previous', () => {
  const layout = { root: 'docs/plan', previous: [DEFAULT_ROOT] }
  const both = (): string =>
    repoOf({
      ...epicAt('docs/plan', 'ts-0003-cc'),
      [`${DEFAULT_ROOT}/epics/ep-002-bb-older/epic.md`]: EPIC_FILE.replace('ep-001-aa', 'ep-002-bb'),
      [`${DEFAULT_ROOT}/epics/ep-002-bb-older/tasks/ts-0001-aa-old.md`]: frontmatter('ts-0001-aa'),
      [`${DEFAULT_ROOT}/epics/ep-002-bb-older/tasks/ts-0004-dd-old.md`]: frontmatter('ts-0004-dd'),
    })

  test('items in both roots are listed, each with the path where it was found', async () => {
    const reply = await callWith(both(), layout, 'list')
    expect(idsOf(reply)).toEqual([EPIC, 'ep-002-bb', 'ts-0001-aa', 'ts-0003-cc', 'ts-0004-dd'])
    expect(reply.invalid).toEqual([])
    expect(pathsOf(reply)).toContain(`${DEFAULT_ROOT}/epics/ep-002-bb-older/tasks/ts-0001-aa-old.md`)
    expect(pathsOf(reply)).toContain('docs/plan/epics/ep-001-aa-alpha/tasks/ts-0003-cc-made-up.md')
  })

  test('without previous, the items at the old root are not listed', async () => {
    expect(idsOf(await callWith(both(), { root: 'docs/plan' }, 'list'))).toEqual([EPIC, 'ts-0003-cc'])
  })

  test('the number is one past the highest id in either root', async () => {
    const repo = both()
    const made = await create(repo, layout, { kind: 'epic', title: 'Third epic', seed: 1 })
    expect(made.result?.task.id).toMatch(/^ep-003-/)
    const task = await create(repo, layout, { epic: EPIC, title: 'Next task', seed: 1 })
    expect(task.result?.task.id).toMatch(/^ts-0005-/)
  })

  test('an id on a branch at the old root is not reused', async () => {
    const repo = repoOf({ ...epicAt('docs/plan', 'ts-0001-aa'), 'README.md': 'x\n' })
    git(repo, 'checkout', '--quiet', '-b', 'old-branch')
    const files = epicAt(DEFAULT_ROOT, 'ts-0020-zz')
    for (const [path, text] of Object.entries(files)) {
      mkdirSync(dirname(join(repo, path)), { recursive: true })
      writeFileSync(join(repo, path), text)
    }
    commitAll(repo, 'old root on a branch')
    git(repo, 'checkout', '--quiet', 'main')
    git(repo, 'clean', '-fdq')
    const made = await create(repo, layout, { epic: EPIC, title: 'After the branch', seed: 1 })
    expect(made.result?.task.id).toMatch(/^ts-0021-/)
  })

  test('the same id in both roots lists the first as the item and the second as invalid', async () => {
    const repo = repoOf({ ...epicAt('docs/plan', 'ts-0001-aa'), ...epicAt(DEFAULT_ROOT, 'ts-0001-aa') })
    const reply = await callWith(repo, layout, 'list')
    expect(idsOf(reply).filter((id) => id === 'ts-0001-aa')).toHaveLength(1)
    expect(reply.invalid.map((entry) => entry.reason).join('\n')).toContain('already used by')
  })

  test('a previous entry that names the root again is listed once', async () => {
    const repo = repoOf(epicAt('docs/plan', 'ts-0001-aa'))
    const reply = await callWith(repo, { root: 'docs/plan', previous: ['docs/plan', './docs/plan/'] }, 'list')
    expect(reply.invalid).toEqual([])
    expect(idsOf(reply)).toEqual([EPIC, 'ts-0001-aa'])
  })

  test('a write to an item at the old root stays at its path', async () => {
    const repo = both()
    const { result } = await setStatus(repo, layout, 'ts-0001-aa', OLD_TASK, 'done')
    expect(result).toMatchObject({ outcome: 'ok', files: [OLD_TASK] })
    expect(readFileSync(join(repo, OLD_TASK), 'utf8')).toContain('status: done')
    expect(git(repo, 'status', '--porcelain')).toBe(` M ${OLD_TASK}\n`)
  })

  test('a write to an item at the old root works while the new root does not exist yet', async () => {
    const repo = repoOf(epicAt(DEFAULT_ROOT, 'ts-0001-aa'))
    const file = `${DEFAULT_ROOT}/epics/ep-001-aa-alpha/tasks/ts-0001-aa-made-up.md`
    const { result } = await setStatus(repo, layout, 'ts-0001-aa', file, 'active')
    expect(result?.outcome).toBe('ok')
    expect(readFileSync(join(repo, file), 'utf8')).toContain('status: active')
    expect(existsSync(join(repo, 'docs/plan/epics'))).toBe(false)
  })

  test('a write to an id nobody has says so, whether or not a root exists', async () => {
    const missing = { op: 'setStatus', id: 'ts-0009-zz', version: 'x', args: { category: 'active' } }
    const { result } = await callWith(repoOf({}), layout, 'apply', missing)
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('no item ts-0009-zz') })
  })

  test('create into an epic that only the old root has is refused, naming where it is', async () => {
    const { result } = await create(both(), layout, { epic: 'ep-002-bb', title: 'No folder here', seed: 1 })
    expect(result?.outcome).toBe('refused')
    expect(result?.message).toContain('docs/plan/epics/')
    expect(result?.message).toContain(`${DEFAULT_ROOT}/epics/ep-002-bb-older`)
  })

  test('a link written to an item at the old root can name an item at the new root', async () => {
    const repo = both()
    const version = fileVersion(join(repo, OLD_TASK))
    const link = { op: 'link', id: 'ts-0001-aa', version, args: { to: 'ts-0003-cc' } }
    const { result } = await callWith(repo, layout, 'apply', link)
    expect(result?.outcome).toBe('ok')
    expect(readFileSync(join(repo, OLD_TASK), 'utf8')).toContain('ts-0003-cc')
  })
})

describe('ids across roots', () => {
  test('idsInPaths reads every epics folder it is given and no other', () => {
    const ids = idsInPaths(
      [
        'docs/plan/epics/ep-001-aa-alpha/tasks/ts-0001-aa-one.md',
        '.collide/planning/epics/ep-002-bb-beta/tasks/ts-0002-bb-two.md',
        'elsewhere/epics/ep-003-cc-gamma/tasks/ts-0003-cc-three.md',
      ],
      ['docs/plan/epics', '.collide/planning/epics'],
    )
    expect([...ids].sort()).toEqual(['ep-001-aa', 'ep-002-bb', 'ts-0001-aa', 'ts-0002-bb'])
  })

  test('a provider that has not been initialized still fails list with the old message', async () => {
    expect((await callUninitialized('list')).error?.message).toContain('send initialize')
  })
})
