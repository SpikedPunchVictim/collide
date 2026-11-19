import { existsSync, mkdirSync, readdirSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { drawSuffix, idsInPaths, nextNumber, seededGenerator, SUFFIX_LETTERS, takenIds } from '../src/ids.ts'
import { slugOf } from '../src/create.ts'
import { applyOp } from '../src/write.ts'
import { call, commitAll, EPIC_FILE, fileVersion, git, makeRepo, spawnProvider, tempDir } from './helpers.ts'

const EPICS = '.collide/planning/epics'
const EPIC = 'ep-001-aa'
const item = (id: string): string => `---\nid: ${id}\nkind: task\nstatus: ready\ntitle: "Made-up item"\nowners: []\nlinks: []\n---\n\n`

/** A git repository holding epic ep-001-aa and the given task files (names under its tasks/ folder), committed on main. */
function repoWith(...tasks: string[]): string {
  const files: Record<string, string> = { 'ep-001-aa-alpha/epic.md': EPIC_FILE }
  for (const name of tasks) files[`ep-001-aa-alpha/tasks/${name}.md`] = item(name.slice(0, 10))
  const root = makeRepo(files)
  git(root, 'init', '--quiet')
  commitAll(root, 'fixture')
  return root
}

const create = (root: string, args: Record<string, unknown>) => call(root, 'apply', { op: 'create', args })
const created = async (root: string, args: Record<string, unknown>): Promise<string> => {
  const { result } = await create(root, args)
  expect(result?.outcome, JSON.stringify(result)).toBe('ok')
  return result?.task.id as string
}

describe('the id parts', () => {
  test('a slug is the first six words of the title, as the existing file names show', () => {
    const cases: Array<[string, string]> = [
      ['Planning slice: markdown provider, read side', 'planning-slice-markdown-provider-read-side'],
      ['Shim protocol, journal, frames and exit.json', 'shim-protocol-journal-frames-and-exit'],
      ['CI: builds, lint, typecheck and structural checks', 'ci-builds-lint-typecheck-and-structural'],
      ["Planning slice: Collide's own items through the CLI", 'planning-slice-collides-own-items-through'],
      ['Apply S5–S8 to the plan (revision 11)', 'apply-s5-s8-to-the-plan'],
      ['Built-in packs compiled into the bundle', 'built-in-packs-compiled-into-the'],
      ['pack.yaml contract', 'pack-yaml-contract'],
      ['— !!', 'item'],
    ]
    for (const [title, slug] of cases) expect(slugOf(title)).toBe(slug)
  })

  test('the ids in a list of paths: epic and spike folders, and files directly in tasks/ or proposals/', () => {
    const ids = idsInPaths([
      `${EPICS}/ep-002-va-skeleton/epic.md`,
      `${EPICS}/ep-002-va-skeleton/tasks/ts-0007-ga-layout.md`,
      `${EPICS}/ep-002-va-skeleton/proposals/pr-0003-km-features.md`,
      `${EPICS}/ep-002-va-skeleton/proposals/sub/pr-0092-aa-nested.md`,
      `${EPICS}/ep-002-va-skeleton/tasks/sub/ts-0091-aa-nested.md`,
      `${EPICS}/ep-002-va-skeleton/tasks/notes.txt`,
      `${EPICS}/ep-003-da-core/`,
      `${EPICS}/README.md`,
      '.collide/planning/spikes/sp-0001-aa-demo/record.md',
      'src/ts-0099-zz-elsewhere.md',
    ])
    expect([...ids].sort()).toEqual(['ep-002-va', 'ep-003-da', 'pr-0003-km', 'sp-0001-aa', 'ts-0007-ga'])
  })

  test('the next number is one past the highest for the kind, and 1 when there is none', () => {
    const taken = new Set(['ts-0007-ga', 'ts-0012-ua', 'ep-002-va'])
    expect(nextNumber(taken, 'ts')).toBe(13)
    expect(nextNumber(taken, 'ep')).toBe(3)
    expect(nextNumber(new Set(), 'ts')).toBe(1)
  })
})

describe('the seeded suffix', () => {
  // Pinned: a change to the generator, the alphabet or the draw order changes ids that people were told, so this fails.
  test.each([
    [1, 'ra pz zg rt kz mn dk fd'],
    [42, 'qm ws ep gr wm fx uh en'],
  ])('seed %i gives the pinned suffixes', (seed, expected) => {
    const next = seededGenerator(seed)
    expect(Array.from({ length: 8 }, () => drawSuffix(next)).join(' ')).toBe(expected)
  })

  test('a seed gives the same suffixes again, and a suffix is two letters from a to z without i, l and o', () => {
    const first = seededGenerator(7)
    const second = seededGenerator(7)
    for (let draw = 0; draw < 5000; draw++) {
      const suffix = drawSuffix(first)
      expect(suffix).toBe(drawSuffix(second))
      expect(suffix).toMatch(/^[a-hjkmnp-z]{2}$/)
    }
    expect(SUFFIX_LETTERS).toHaveLength(23)
  })
})

describe('create', () => {
  test('declares a create op and the capability', async () => {
    const { result } = await call(repoWith(), 'initialize', { config: {} })
    expect(result?.capabilities.create).toBe(true)
    const op = (result?.ops as Array<Record<string, any>>).find((candidate) => candidate.name === 'create')
    expect(op?.args).toMatchObject({
      required: ['title'],
      properties: { kind: { enum: ['task', 'epic', 'proposal', 'spike', 'question'] }, seed: { type: 'integer' } },
      additionalProperties: false,
    })
  })

  test('writes <id>-<slug>.md into the epic\'s tasks/ folder, returns the file it touched, and the item loads', async () => {
    const root = repoWith('ts-0001-aa-one')
    const { result } = await create(root, { epic: EPIC, title: 'Planning slice: ids, create and reid, and more', seed: 1 })
    const path = `${EPICS}/ep-001-aa-alpha/tasks/ts-0002-ra-planning-slice-ids-create-and-reid.md`
    const version = fileVersion(join(root, path))
    expect(result).toMatchObject({
      outcome: 'ok',
      version,
      files: [path],
      task: { id: 'ts-0002-ra', key: 'plan:ts-0002-ra', kind: 'task', version, path, title: 'Planning slice: ids, create and reid, and more', state: { category: 'ready', status: 'ready' }, owners: [] },
      items: [{ key: 'plan:ts-0002-ra', version, from: null, to: { category: 'ready', status: 'ready' } }],
    })
    expect(result?.task.links).toEqual([{ type: 'part-of', to: EPIC }])
    const listed = await call(root, 'list')
    expect(listed.invalid).toEqual([])
    expect((listed.result?.tasks as Array<{ id: string }>).map((task) => task.id)).toContain('ts-0002-ra')
    expect(git(root, 'status', '--porcelain')).toBe(`?? ${path}\n`)
  })

  test('an epic goes into epics/ep-NNN-xx-<slug>/epic.md, numbered with three digits', async () => {
    const root = repoWith()
    const { result } = await create(root, { kind: 'epic', title: 'Stage 3: headless core', seed: 1 })
    const path = `${EPICS}/ep-002-ra-stage-3-headless-core/epic.md`
    expect(result).toMatchObject({ outcome: 'ok', files: [path], task: { id: 'ep-002-ra', kind: 'epic', path, links: [] } })
    expect(existsSync(join(root, path))).toBe(true)
    // A task can be made in the new epic at once.
    expect(await created(root, { epic: 'ep-002-ra', title: 'First task', seed: 1 })).toBe('ts-0001-ra')
  })

  test('a title with a quote, a colon and non-ASCII text reads back as written', async () => {
    const root = repoWith()
    const title = 'He said "no": a bébé \\ test # not a comment'
    const { result } = await create(root, { epic: EPIC, title, seed: 3 })
    expect(result?.task.title).toBe(title)
  })

  test('leaves no lock file and no temp file', async () => {
    const root = repoWith()
    await created(root, { epic: EPIC, title: 'Tidy', seed: 3 })
    const names = readdirSync(join(root, '.collide/planning'), { recursive: true }) as string[]
    expect(names.filter((name) => /write-lock|collide-tmp/.test(name))).toEqual([])
  })

  test('without a seed the suffix is random: two creates in two repositories seldom match, and every one is valid', async () => {
    const suffixes = new Set<string>()
    for (let run = 0; run < 6; run++) suffixes.add((await created(repoWith(), { epic: EPIC, title: 'Random' })).slice(-2))
    expect(suffixes.size).toBeGreaterThan(1)
    for (const suffix of suffixes) expect(suffix).toMatch(/^[a-hjkmnp-z]{2}$/)
  })

  test.each([
    [{ title: 'No epic' }, 'a task needs epic'],
    [{ epic: 'ep-009-zz', title: 'Unknown epic' }, 'there is no epic ep-009-zz'],
    [{ epic: 'nonsense', title: 'Bad epic id' }, 'a task needs epic'],
    [{ kind: 'epic', epic: EPIC, title: 'An epic in an epic' }, 'an epic is part of no epic'],
    [
      { kind: 'widget', epic: EPIC, title: 'Unknown kind' },
      'kind must be "task", "epic", "proposal", "question" or "spike"',
    ],
    [{ kind: 'spike', epic: EPIC, title: 'A spike in an epic' }, 'a spike is part of no epic'],
    [{ epic: EPIC }, 'title must be text'],
    [{ epic: EPIC, title: ' padded ' }, 'title must be text'],
    [{ epic: EPIC, title: 'two\nlines' }, 'title must be text'],
    [{ epic: EPIC, title: 'x'.repeat(201) }, 'title must be text'],
    [{ epic: EPIC, title: 'Seed', seed: 1.5 }, 'seed must be a whole number'],
    [{ epic: EPIC, title: 'Seed', speed: 1 }, 'unknown argument "speed"'],
  ])('refuses %j and writes nothing', async (args, message) => {
    const root = repoWith()
    const { result } = await create(root, args)
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining(message) })
    expect(git(root, 'status', '--porcelain')).toBe('')
  })

  test('refuses an id, a version and a path that is not a checkout of the repository', async () => {
    const root = repoWith()
    const args = { epic: EPIC, title: 'Extra' }
    expect((await call(root, 'apply', { op: 'create', id: 'ts-0001-aa', args })).result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('leave id out') })
    expect((await call(root, 'apply', { op: 'create', version: 'abc', args })).result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('takes no version') })
    expect((await call(root, 'apply', { op: 'create', checkout: 'wt', args })).result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('checkout') })
    expect(git(root, 'status', '--porcelain')).toBe('')
  })

  test('refuses outside a git repository, because other branches cannot be seen', async () => {
    const root = makeRepo({ 'ep-001-aa-alpha/epic.md': EPIC_FILE })
    const { result } = await create(root, { epic: EPIC, title: 'No git', seed: 1 })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('could not read the refs') })
    expect(existsSync(join(root, EPICS, 'ep-001-aa-alpha/tasks'))).toBe(false)
  })
})

describe('the number across refs', () => {
  /** A bare remote, a clone `work` that has main, and a second clone that pushed branch `other` with ts-0007-zz. */
  function withRemoteOnlyBranch(): { work: string; other: string } {
    const remote = tempDir()
    git(remote, 'init', '--bare', '--quiet')
    const seedClone = tempDir()
    git(seedClone, 'init', '--quiet')
    git(seedClone, 'remote', 'add', 'origin', remote)
    mkdirSync(join(seedClone, EPICS, 'ep-001-aa-alpha/tasks'), { recursive: true })
    writeFileSync(join(seedClone, EPICS, 'ep-001-aa-alpha/epic.md'), EPIC_FILE)
    writeFileSync(join(seedClone, EPICS, 'ep-001-aa-alpha/tasks/ts-0002-bb-two.md'), item('ts-0002-bb'))
    commitAll(seedClone, 'main')
    git(seedClone, 'push', '--quiet', 'origin', 'HEAD:main')

    const work = tempDir()
    git(work, 'clone', '--quiet', '--branch', 'main', remote, '.')

    git(seedClone, 'checkout', '--quiet', '-b', 'other')
    writeFileSync(join(seedClone, EPICS, 'ep-001-aa-alpha/tasks/ts-0007-zz-seven.md'), item('ts-0007-zz'))
    commitAll(seedClone, 'other')
    git(seedClone, 'push', '--quiet', 'origin', 'other')
    git(work, 'fetch', '--quiet', 'origin')
    return { work, other: seedClone }
  }

  test('counts a branch that exists only as a fetched remote ref', async () => {
    const { work } = withRemoteOnlyBranch()
    expect(git(work, 'branch', '--list', 'other')).toBe('')
    expect(git(work, 'branch', '-r', '--list', 'origin/other').trim()).toBe('origin/other')
    expect(await created(work, { epic: EPIC, title: 'After the remote branch', seed: 1 })).toBe('ts-0008-ra')
  })

  test('without the fetch the remote branch is not seen, and with it the next number moves past it', async () => {
    const { work, other } = withRemoteOnlyBranch()
    // A second branch pushed after the fetch is invisible until the next fetch.
    writeFileSync(join(other, EPICS, 'ep-001-aa-alpha/tasks/ts-0011-yy-eleven.md'), item('ts-0011-yy'))
    commitAll(other, 'later')
    git(other, 'push', '--quiet', 'origin', 'other')
    expect((await create(work, { epic: EPIC, title: 'Before the fetch', seed: 1 })).result?.task.id).toBe('ts-0008-ra')
    git(work, 'fetch', '--quiet', 'origin')
    expect((await create(work, { epic: EPIC, title: 'After the fetch', seed: 2 })).result?.task.id).toMatch(/^ts-0012-/)
  })

  test('counts a local branch that is not checked out, and an item in the working tree that no ref has', async () => {
    const root = repoWith('ts-0001-aa-one')
    git(root, 'checkout', '--quiet', '-b', 'side')
    writeFileSync(join(root, EPICS, 'ep-001-aa-alpha/tasks/ts-0009-cc-nine.md'), item('ts-0009-cc'))
    commitAll(root, 'side')
    git(root, 'checkout', '--quiet', 'main')
    expect(await created(root, { epic: EPIC, title: 'After the local branch', seed: 1 })).toBe('ts-0010-ra')
    // The new file is in the working tree only; the next number counts it.
    expect(await created(root, { epic: EPIC, title: 'After the working tree', seed: 2 })).toMatch(/^ts-0011-/)
  })

  test('numbers each kind on its own', async () => {
    const root = repoWith('ts-0001-aa-one')
    expect(await created(root, { kind: 'epic', title: 'Second epic', seed: 1 })).toBe('ep-002-ra')
    expect(await created(root, { epic: EPIC, title: 'Second task', seed: 1 })).toBe('ts-0002-ra')
  })
})

describe('two writers at once', () => {
  test('creates from two provider processes under one lock get different numbers, never one file written over the other', async () => {
    const root = repoWith()
    const [a, b] = await Promise.all([spawnProvider(root), spawnProvider(root)])
    const replies = await Promise.all(
      [a, b, a, b].map((provider, index) => provider.request('apply', { op: 'create', args: { epic: EPIC, title: `Item ${index}`, seed: 1 } })),
    )
    await Promise.all([a.close(), b.close()])
    const ids = replies.map((reply) => reply.task.id as string)
    expect(replies.map((reply) => reply.outcome)).toEqual(['ok', 'ok', 'ok', 'ok'])
    expect(new Set(ids.map((id) => id.slice(0, 7))).size).toBe(4)
    expect(readdirSync(join(root, EPICS, 'ep-001-aa-alpha/tasks'))).toHaveLength(4)
  })
})

describe('a taken id is redrawn', () => {
  /** A generator that returns the given letters' draws in order: each letter is the middle of its slot. */
  const rigged = (...suffixes: string[]) => {
    const letters = suffixes.join('').split('')
    return () => () => (SUFFIX_LETTERS.indexOf(letters.shift() ?? 'a') + 0.5) / SUFFIX_LETTERS.length
  }

  test('the generator returns a taken id first, and the draw is repeated until the id is free', async () => {
    const root = repoWith()
    let looks = 0
    const result = await applyOp(
      root,
      { op: 'create', args: { epic: EPIC, title: 'Collide', seed: 1 } },
      {},
      {
        generator: rigged('ab', 'ab', 'cd'),
        // The first look finds number 1 free; a ref that arrives after it holds ts-0001-ab.
        look: async (path) => {
          looks++
          const taken = await takenIds(path)
          if (looks > 1) taken.add('ts-0001-ab')
          return taken
        },
      },
    )
    expect(result).toMatchObject({ outcome: 'ok', task: { id: 'ts-0001-cd' } })
    expect(looks).toBe(4)
  })

  test('a number with every suffix taken stops with an error instead of drawing for ever', async () => {
    const root = repoWith()
    const everySuffix = SUFFIX_LETTERS.split('').flatMap((a) => SUFFIX_LETTERS.split('').map((b) => `ts-0001-${a}${b}`))
    let looks = 0
    // Number 1 is free at the first look; by the draws, a fetch has brought every suffix of it.
    const look = async () => new Set(looks++ === 0 ? [] : everySuffix)
    await expect(applyOp(root, { op: 'create', args: { epic: EPIC, title: 'Full', seed: 1 } }, {}, { look })).rejects.toThrow('no free suffix for ts-0001')
    expect(git(root, 'status', '--porcelain')).toBe('')
  })
})

describe('the same number made on two branches', () => {
  test('both items load after the merge, and neither is invalid', async () => {
    // Two clones that have not fetched from each other, so neither can see the other's item.
    const remote = tempDir()
    git(remote, 'init', '--bare', '--quiet')
    const first = tempDir()
    git(first, 'init', '--quiet')
    git(first, 'remote', 'add', 'origin', remote)
    mkdirSync(join(first, EPICS, 'ep-001-aa-alpha/tasks'), { recursive: true })
    writeFileSync(join(first, EPICS, 'ep-001-aa-alpha/epic.md'), EPIC_FILE)
    commitAll(first, 'main')
    git(first, 'push', '--quiet', 'origin', 'HEAD:main')
    const second = tempDir()
    git(second, 'clone', '--quiet', '--branch', 'main', remote, '.')

    git(first, 'checkout', '--quiet', '-b', 'branch-a')
    const a = await created(first, { epic: EPIC, title: 'Made on branch a', seed: 1 })
    commitAll(first, 'a')
    git(first, 'push', '--quiet', 'origin', 'branch-a')
    git(second, 'checkout', '--quiet', '-b', 'branch-b')
    const b = await created(second, { epic: EPIC, title: 'Made on branch b', seed: 2 })
    commitAll(second, 'b')
    git(second, 'fetch', '--quiet', 'origin')
    git(second, 'merge', '--quiet', '--no-edit', 'origin/branch-a')

    expect(a.slice(0, 7)).toBe(b.slice(0, 7))
    expect(a).not.toBe(b)
    const listed = await call(second, 'list')
    expect(listed.invalid).toEqual([])
    expect((listed.result?.tasks as Array<{ id: string }>).map((task) => task.id)).toEqual(expect.arrayContaining([a, b]))
    // The next create counts both.
    expect(await created(second, { epic: EPIC, title: 'After the merge', seed: 1 })).toMatch(/^ts-0002-/)
  })
})
