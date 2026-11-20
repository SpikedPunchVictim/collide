import { spawnSync } from 'node:child_process'
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from 'node:fs'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { afterAll, beforeAll, describe, expect, test } from 'vitest'
import { CASES, runCase } from '../../orchard-arch/contract/cases.ts'
import { FAKE_PARAMS, makeFakeRoot } from '../../orchard-arch/contract/fake-root.ts'
import { diffRuns, runShape } from '../../orchard-arch/contract/shape.ts'
import type { RunShape, Shape } from '../../orchard-arch/contract/shape.ts'
import { REPO } from './helpers.ts'

// The fake plan CLI (fixtures/orchard-arch/plan) is held to the shapes recorded in fixtures/orchard-arch/contract
// (ts-0153-kp). The shape files are the real CLI's, once it has been recorded; until then they are the fake's own.

const CONTRACT = join(REPO, 'fixtures', 'orchard-arch', 'contract')
const SHAPES = join(CONTRACT, 'shapes')
const FAKE = join(REPO, 'fixtures', 'orchard-arch', 'plan')
const RECORD = join(CONTRACT, 'record.ts')
const DEADLINE_MS = 120_000

interface Recorded {
  case: string
  args: string[]
  source: string
  recorded: string
  shape: RunShape
}

const readShape = (dir: string, name: string): Recorded =>
  JSON.parse(readFileSync(join(dir, `${name}.json`), 'utf8')) as Recorded

const temps: string[] = []
afterAll(() => temps.forEach((dir) => rmSync(dir, { recursive: true, force: true })))

/** A temp folder with a `home`, an empty git config and the fake's root in `root`. */
function scratch(): { base: string; root: string; env: NodeJS.ProcessEnv } {
  const base = mkdtempSync(join(tmpdir(), 'arch-contract-'))
  temps.push(base)
  const home = join(base, 'home')
  mkdirSync(home)
  writeFileSync(join(base, 'empty.gitconfig'), '')
  makeFakeRoot(join(base, 'root'))
  const env = {
    PATH: process.env.PATH ?? '',
    HOME: home,
    GIT_CONFIG_GLOBAL: join(base, 'empty.gitconfig'),
    GIT_CONFIG_SYSTEM: join(base, 'empty.gitconfig'),
  }
  return { base, root: join(base, 'root'), env }
}

type Obj = { [key: string]: Shape }

/** The keys down to the first leaf of an object shape, going through objects only. */
function firstLeafPath(shape: Shape | null): string[] {
  if (shape === null || typeof shape !== 'object' || Array.isArray(shape)) return []
  for (const [key, inner] of Object.entries(shape)) {
    if (typeof inner === 'string') return [key]
    const deeper = firstLeafPath(inner)
    if (deeper.length > 0) return [key, ...deeper]
  }
  return []
}

const named = (name: string) => CASES.find((one) => one.name === name)!

/** The problems of one case, each naming the case and the path in the envelope. */
const problems = (name: string, recorded: RunShape, actual: RunShape): string[] =>
  diffRuns(recorded, actual).map((problem) => `case ${name}: ${problem}`)

describe('the recorded shapes', () => {
  test('are one file for each case, and no other', () => {
    const files = readdirSync(SHAPES).filter((name) => name.endsWith('.json'))
    expect(files.sort()).toEqual(CASES.map((one) => `${one.name}.json`).sort())
  })

  test.each(CASES)('$name is a file that names its case and its arguments', (one) => {
    const file = readShape(SHAPES, one.name)
    expect(file.case).toBe(one.name)
    expect(file.args).toEqual([...one.args, ...(one.dryRun === true ? ['--dry-run'] : [])])
    expect(file.recorded).toMatch(/^\d{4}-\d{2}-\d{2}$/)
  })
})

describe('the fake plan CLI', () => {
  const { root, env } = scratch()

  // The cases change the root, in this order: each test follows the one before.
  test.each(CASES)('keeps the recorded shape of $name', (one) => {
    const recorded = readShape(SHAPES, one.name).shape
    const actual = runShape(runCase(FAKE, root, one, FAKE_PARAMS, env))
    expect(problems(one.name, recorded, actual)).toEqual([])
  })

  // The edits are built from whatever set is committed, the fake's own or the real CLI's.
  test('is told apart from a shape that was edited by hand: a leaf of another type', () => {
    const recorded = structuredClone(readShape(SHAPES, 'new-task').shape)
    const path = firstLeafPath(recorded.envelope)
    const parent = path.slice(0, -1).reduce((node, key) => node[key] as Obj, recorded.envelope as Obj)
    const original = parent[path.at(-1)!] as string
    parent[path.at(-1)!] = '=never'
    const actual = runShape(runCase(FAKE, scratch().root, named('new-task'), FAKE_PARAMS, env))
    const where = path.map((key) => key.replace(/\?$/, '')).join('.')
    expect(problems('new-task', recorded, actual)).toEqual([
      `case new-task: envelope.${where}: recorded =never, got ${original}`,
    ])
  })

  test('is told apart from a shape that was edited by hand: a key the fake lacks', () => {
    const recorded = structuredClone(readShape(SHAPES, 'status').shape)
    ;(recorded.envelope as Obj).tally = 'number'
    const actual = runShape(runCase(FAKE, scratch().root, named('status'), FAKE_PARAMS, env))
    expect(problems('status', recorded, actual)).toEqual(['case status: envelope.tally: recorded, missing'])
  })

  test('is told apart from a shape that was edited by hand: an exit code', () => {
    const recorded = readShape(SHAPES, 'set-no-task').shape
    const actual = runShape(runCase(FAKE, scratch().root, named('set-no-task'), FAKE_PARAMS, env))
    expect(problems('set-no-task', { ...recorded, exit: recorded.exit! + 1 }, actual)).toEqual([
      `case set-no-task: exit: recorded ${recorded.exit! + 1}, got ${recorded.exit}`,
    ])
  })
})

describe('the recorder', () => {
  const record = (args: string[], env: NodeJS.ProcessEnv) =>
    spawnSync(process.execPath, [RECORD, ...args], { env, encoding: 'utf8', timeout: DEADLINE_MS })

  const argsFor = (base: string, root: string): string[] => [
    '--cli',
    FAKE,
    '--root',
    root,
    '--out',
    join(base, 'out'),
    '--source',
    'fake',
    '--task',
    FAKE_PARAMS.task,
    '--proposal',
    FAKE_PARAMS.proposal,
    '--epic',
    FAKE_PARAMS.epic,
  ]

  test('prints the path of each file it wrote and nothing else', () => {
    const { base, root, env } = scratch()
    const run = record(argsFor(base, root), env)
    expect(run.stderr).toBe('')
    expect(run.status).toBe(0)
    expect(run.stdout.trim().split('\n')).toEqual(CASES.map((one) => join(base, 'out', `${one.name}.json`)))
  })

  test('writes only the shape files under its folder, and leaves no home behind', () => {
    const { base, root, env } = scratch()
    record(argsFor(base, root), env)
    expect(readdirSync(join(base, 'out')).sort()).toEqual(CASES.map((one) => `${one.name}.json`).sort())
    expect(readdirSync(base).sort()).toEqual(['empty.gitconfig', 'home', 'out', 'root'])
  })

  test('records shapes that hold against the committed ones, when it runs over the fake', () => {
    const { base, root, env } = scratch()
    record(argsFor(base, root), env)
    for (const one of CASES) {
      const written = readShape(join(base, 'out'), one.name)
      const kept = readShape(SHAPES, one.name)
      expect(problems(one.name, kept.shape, written.shape), one.name).toEqual([])
      expect(written.args, one.name).toEqual(kept.args)
      expect(written.source).toBe('fake')
    }
  })

  test('refuses a task that is not ready, and runs nothing', () => {
    const { base, root, env } = scratch()
    const args = argsFor(base, root)
    args[args.indexOf('--task') + 1] = 'T-03-001'
    const run = record(args, env)
    expect(run.status).toBe(2)
    expect(run.stdout).toBe('')
    expect(run.stderr).toBe('record: --task is not a task of the root in status ready\n')
    expect(existsSync(join(base, 'out'))).toBe(false)
  })

  test('refuses a CLI that cannot be run, in its own words', () => {
    const { base, root, env } = scratch()
    const args = argsFor(base, root)
    args[args.indexOf('--cli') + 1] = join(base, 'no-such-cli')
    const run = record(args, env)
    expect(run.status).toBe(2)
    expect(run.stdout).toBe('')
    expect(run.stderr).toBe('record: the CLI could not be run for the case status (ENOENT)\n')
  })
})
