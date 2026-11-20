import { existsSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { makeRepo, plan } from './repo.ts'
import type { Repo, Run } from './repo.ts'

// The fake plan CLI (fixtures/orchard-arch/plan) is what the adapter's write tests run, so it has tests of its own:
// its table of legal moves, how it regenerates an epic's table, its flags, its JSON envelope and its exit codes.

const ARCH = ['--root', 'arch']
const set = (repo: Repo, id: string, status: string, ...flags: string[]) =>
  plan(repo, ['set', id, `status=${status}`, ...flags, ...ARCH])
const json = (run: Run) => JSON.parse(run.stdout) as Record<string, any>
const statusLine = (repo: Repo, path: string): string => /^status: (.*)$/m.exec(repo.read(path))?.[1] ?? ''

const MOVES: Record<string, string[]> = {
  specified: ['ready', 'dropped'],
  ready: ['doing', 'dropped'],
  doing: ['in_review', 'dropped'],
  in_review: ['done', 'doing', 'dropped'],
  done: [],
  dropped: [],
}
const STATUSES = Object.keys(MOVES)

describe('the table of legal moves', () => {
  const repo = makeRepo()
  const path = repo.taskPath('T-03-002')
  // A review on record, so that a move to done is judged by the table and not by the review rule.
  repo.write(path, repo.read(path).replace(/^status: .*$/m, 'status: doing\nself_review: true'))

  const pairs = STATUSES.flatMap((from) => STATUSES.map((to) => [from, to] as const))
  test.each(pairs)('%s to %s', (from, to) => {
    repo.write(path, repo.read(path).replace(/^status: .*$/m, `status: ${from}`))
    const run = set(repo, 'T-03-002', to)
    if (from === to || MOVES[from]?.includes(to)) {
      expect(run.code, run.stderr).toBe(0)
      expect(statusLine(repo, path)).toBe(to)
    } else {
      expect(run.code).toBe(2)
      expect(run.stderr).toContain(`${from} -> ${to} is not a legal step`)
      expect(statusLine(repo, path)).toBe(from)
    }
  })

  test('a refusal names the step and then the legal ones, on two lines, with nothing on standard output', () => {
    repo.write(path, repo.read(path).replace(/^status: .*$/m, 'status: ready'))
    const run = set(repo, 'T-03-002', 'done')
    expect(run.code).toBe(2)
    expect(run.stdout).toBe('')
    const lines = run.stderr.trim().split('\n')
    expect(lines).toHaveLength(2)
    expect(lines[0]).toContain('ready -> done is not a legal step')
    expect(lines[1]).toBe('legal steps from ready: doing, dropped')
    repo.write(path, repo.read(path).replace(/^status: .*$/m, 'status: dropped'))
    const final = set(repo, 'T-03-002', 'ready').stderr.trim().split('\n')
    expect(final[1]).toBe('legal steps from dropped: none, it is final')
  })

  test('refuses a move the real write would refuse, in the same way', () => {
    const repo = makeRepo()
    const run = set(repo, 'T-03-003', 'done', '--dry-run')
    expect(run.code).toBe(2)
    expect(run.stderr).toContain('ready -> done is not a legal step')
  })

  test('says in text that nothing was written', () => {
    const repo = makeRepo()
    const run = plan(repo, ['set', 'T-03-003', 'status=doing', '--dry-run', ...ARCH])
    expect(run.stderr).toContain('would write: ')
    expect(run.stderr).not.toContain('wrote: ')
  })
})

describe('exit codes', () => {
  const bad: Array<[string, string[]]> = [
    ['no command', []],
    ['a command it has not got', ['close', 'T-03-003', ...ARCH]],
    ['a flag it has not got', ['set', 'T-03-003', 'status=doing', '--colour', ...ARCH]],
    ['a flag with no value', ['set', 'T-03-003', 'status=doing', '--format', ...ARCH]],
    ['a format it has not got', ['set', 'T-03-003', 'status=doing', '--format', 'xml', ...ARCH]],
    ['a root that is not a folder', ['set', 'T-03-003', 'status=doing', '--root', 'nowhere']],
    // The plan CLI is an oclif CLI: its first argument is the command, and --root is a flag of each command.
    ['a flag before the command', [...ARCH, 'set', 'T-03-003', 'status=doing']],
    ['set with no assignment', ['set', 'T-03-003', ...ARCH]],
    ['an assignment that is not field=value', ['set', 'T-03-003', 'doing', ...ARCH]],
    ['a field it does not set', ['set', 'T-03-003', 'title=Other', ...ARCH]],
    ['an id that is not a task id', ['set', 'task-3', 'status=doing', ...ARCH]],
    ['a task that is not there', ['set', 'T-03-099', 'status=doing', ...ARCH]],
    ['a status it has not got', ['set', 'T-03-003', 'status=started', ...ARCH]],
    ['new with no kind', ['new', ...ARCH]],
    ['new of a kind it cannot make', ['new', 'epic', 'P03', 'Title', '--epic', 'E-07', ...ARCH]],
    ['new task with no epic', ['new', 'task', 'P03', 'Title', ...ARCH]],
    ['new task for a proposal that is not there', ['new', 'task', 'P99', 'Title', '--epic', 'E-07', ...ARCH]],
    ['new task with a title starting with a dash', ['new', 'task', 'P03', '-Title', '--epic', 'E-07', ...ARCH]],
  ]
  test.each(bad)('exit 2, a message on stderr and nothing written: %s', (_name, args) => {
    const repo = makeRepo()
    const before = repo.snapshot()
    const run = plan(repo, args)
    expect(run.code).toBe(2)
    expect(run.stderr.trim()).not.toBe('')
    expect(run.stdout).toBe('')
    expect(repo.snapshot()).toEqual(before)
  })

  test('exit 1 when the write gate refuses, with the reasons in the envelope and nothing written', () => {
    const repo = makeRepo()
    const before = repo.snapshot()
    const run = plan(repo, ['new', 'task', 'P03', 'Title', '--epic', 'E-99', '--format', 'json', ...ARCH])
    expect(run.code).toBe(1)
    expect(json(run).data).toMatchObject({
      written: false,
      errors: [
        {
          rule: 'reference-resolves',
          severity: 'error',
          message: 'epic E-99 has no epic file',
        },
      ],
    })
    expect(repo.snapshot()).toEqual(before)
  })

  test('exit 1 for a dependency that names no task', () => {
    const repo = makeRepo()
    const run = plan(repo, [
      'new',
      'task',
      'P03',
      'Title',
      '--epic',
      'E-07',
      '--depends-on',
      'T-03-001,T-77-001',
      '--format',
      'json',
      ...ARCH,
    ])
    expect(run.code).toBe(1)
    expect(json(run).data.errors).toHaveLength(1)
    expect(json(run).data.errors[0].message).toContain('T-77-001')
  })
})

describe('new task', () => {
  test('makes the next task of the proposal, with its frontmatter, and adds it to the epic table', () => {
    const repo = makeRepo()
    const run = plan(repo, ['new', 'task', 'P03', 'Chart the tides', '--epic', 'E-07', '--format', 'json', ...ARCH])
    expect(run.code, run.stderr).toBe(0)
    const path = 'arch/proposals/03-harbor-index/tasks/T-03-006-chart-the-tides.md'
    expect(json(run).command).toBe('new-task')
    expect(json(run).data).toEqual({
      relPath: 'proposals/03-harbor-index/tasks/T-03-006-chart-the-tides.md',
      id: 'T-03-006-chart-the-tides',
      kind: 'task',
      frontmatter: {
        title: 'Chart the tides',
        status: 'specified',
        epic: 'E-07',
        depends_on: [],
        blocked_by: [],
      },
      written: true,
      dryRun: false,
      errors: [],
      regenerated: ['work/epics/E-07-harbor-plan.md'],
    })
    expect(repo.read(path)).toBe(
      '---\ntitle: "Chart the tides"\nstatus: specified\nepic: E-07\ndepends_on: []\nblocked_by: []\n---\n\n' +
        '## Task\n\nA task made by the fake plan CLI.\n',
    )
    expect(repo.read('arch/work/epics/E-07-harbor-plan.md')).toContain('T-03-006-chart-the-tides.md) | specified |')
  })

  test('writes dependencies as a list, and takes the proposal with or without its P', () => {
    const repo = makeRepo()
    const args = ['new', 'task', '05', 'Hang the lamps', '--epic', 'E-08', '--depends-on', 'T-05-001,T-05-002', ...ARCH]
    expect(plan(repo, args).code).toBe(0)
    expect(repo.read('arch/proposals/05-lamp-works/tasks/T-05-004-hang-the-lamps.md')).toContain(
      'depends_on: [T-05-001, T-05-002]',
    )
    expect(plan(repo, ['new', 'task', 'P05', 'Dim the lamps', '--epic', 'E-08', ...ARCH]).code).toBe(0)
    expect(repo.read('arch/proposals/05-lamp-works/tasks/T-05-005-dim-the-lamps.md')).toContain('Dim the lamps')
  })

  test('skips a number the id map names on either side', () => {
    const repo = makeRepo()
    repo.write('arch/ID-MAP.md', `${repo.read('arch/ID-MAP.md')}| T-05-004 | T-05-006 | made up | none |\n`)
    const run = plan(repo, ['new', 'task', 'P05', 'Skip the numbers', '--epic', 'E-08', '--format', 'json', ...ARCH])
    expect(json(run).data.id).toBe('T-05-005-skip-the-numbers')
  })

  test('with --dry-run reports the file and writes nothing', () => {
    const repo = makeRepo()
    const before = repo.snapshot()
    const run = plan(repo, [
      'new',
      'task',
      'P03',
      'Chart the tides',
      '--epic',
      'E-07',
      '--format',
      'json',
      '--dry-run',
      ...ARCH,
    ])
    expect(run.code).toBe(0)
    expect(json(run).data).toMatchObject({
      relPath: 'proposals/03-harbor-index/tasks/T-03-006-chart-the-tides.md',
      written: false,
      dryRun: true,
      regenerated: ['work/epics/E-07-harbor-plan.md'],
    })
    expect(repo.snapshot()).toEqual(before)
  })

  test('keeps the rows of the table in id order', () => {
    const repo = makeRepo()
    plan(repo, ['new', 'task', 'P03', 'Chart the tides', '--epic', 'E-07-harbor-plan', ...ARCH])
    const rows = repo.read('arch/work/epics/E-07-harbor-plan.md').match(/\[(T-\d{2}-\d{3})\]/g)
    expect(rows).toEqual(['[T-03-001]', '[T-03-002]', '[T-03-003]', '[T-03-005]', '[T-03-006]'])
  })
})

describe('renumber', () => {
  const OLD = 'proposals/03-harbor-index/tasks/T-03-002-map-the-moorings.md'
  const NEW = 'proposals/03-harbor-index/tasks/T-03-050-map-the-moorings.md'
  const EPIC = 'arch/work/epics/E-07-harbor-plan.md'
  const renumber = (repo: Repo, from: string, to: string, ...flags: string[]) =>
    plan(repo, ['renumber', from, to, ...flags, ...ARCH])
  const changedFiles = (before: Record<string, string>, after: Record<string, string>): string[] =>
    [...new Set([...Object.keys(before), ...Object.keys(after)])].filter((path) => before[path] !== after[path]).sort()

  test('moves the file to the new number in its own folder, and keeps the slug', () => {
    const repo = makeRepo()
    const run = renumber(repo, 'T-03-002', 'T-03-050')
    expect(run.code, run.stderr).toBe(0)
    expect(existsSync(join(repo.root, 'arch', OLD))).toBe(false)
    expect(repo.read(`arch/${NEW}`)).toContain('title: "Map the moorings"')
  })

  test('takes the old id with its slug as well as bare', () => {
    const repo = makeRepo()
    expect(renumber(repo, 'T-03-002-map-the-moorings', 'T-03-050').code).toBe(0)
    expect(existsSync(join(repo.root, 'arch', NEW))).toBe(true)
  })

  test('rewrites each citation of the old id, bare or with its slug, in tasks and epics, and in its own file', () => {
    const repo = makeRepo()
    expect(renumber(repo, 'T-03-002', 'T-03-050').code).toBe(0)
    expect(repo.read(repo.taskPath('T-03-003'))).toContain('depends_on: [T-03-001, T-03-050]')
    expect(repo.read(repo.taskPath('T-03-004'))).toContain('  - T-03-050\n')
    expect(repo.read(`arch/${NEW}`)).toContain('Made-up body for T-03-050.')
    const epic = repo.read(EPIC)
    expect(epic).toContain('T-03-001 → T-03-050')
    expect(epic).toContain('T-03-001, T-03-050 (M)')
    expect(epic).not.toContain('T-03-002')
  })

  test('refreshes the generated table of the epic, with the new path, and writes no other file', () => {
    const repo = makeRepo()
    const before = repo.snapshot()
    expect(renumber(repo, 'T-03-002', 'T-03-050').code).toBe(0)
    expect(repo.read(EPIC)).toContain(`](../../${NEW}) | doing |`)
    expect(changedFiles(before, repo.snapshot())).toEqual(
      [
        `arch/${OLD}`,
        `arch/${NEW}`,
        EPIC,
        repo.taskPath('T-03-003'),
        repo.taskPath('T-03-004'),
        'arch/ID-MAP.md',
      ].sort(),
    )
  })

  test('adds a row to the id map: old id, new id, the day, and the path it moved from', () => {
    const repo = makeRepo()
    const before = repo.read('arch/ID-MAP.md')
    expect(renumber(repo, 'T-03-002', 'T-03-050').code).toBe(0)
    const map = repo.read('arch/ID-MAP.md')
    expect(map.startsWith(before)).toBe(true)
    const row = map.slice(before.length).trimEnd()
    expect(row).toMatch(/^\| T-03-002-map-the-moorings \| T-03-050-map-the-moorings \| \d{4}-\d{2}-\d{2} \| moved: /)
    expect(row.endsWith(`moved: ${OLD} |`)).toBe(true)
    expect(row.split('\n')).toHaveLength(1)
  })

  test('makes the id map, with its header row, when there is none', () => {
    const repo = makeRepo()
    rmSync(join(repo.root, 'arch', 'ID-MAP.md'))
    expect(renumber(repo, 'T-03-002', 'T-03-050').code).toBe(0)
    const lines = repo.read('arch/ID-MAP.md').trimEnd().split('\n')
    expect(lines.slice(-3, -1)).toEqual(['| old id | new id | renamed | detail |', '|---|---|---|---|'])
    expect(lines.at(-1)).toContain('| T-03-002-map-the-moorings | T-03-050-map-the-moorings |')
  })

  test('takes no number that the id map names, so that a later task never reuses the old one', () => {
    const repo = makeRepo()
    expect(renumber(repo, 'T-03-005', 'T-03-006').code).toBe(0)
    const made = plan(repo, ['new', 'task', 'P03', 'Chart the tides', '--epic', 'E-07', '--format', 'json', ...ARCH])
    expect(json(made).data.id).toBe('T-03-007-chart-the-tides')
  })

  test('says what it did in JSON, in the shape of the plan CLI: a versioned envelope and a report', () => {
    const repo = makeRepo()
    const run = renumber(repo, 'T-03-002', 'T-03-050', '--format', 'json')
    expect(run.code).toBe(0)
    const envelope = json(run)
    expect(envelope).toMatchObject({ schema_version: 1, command: 'renumber', corpus: 'arch' })
    expect(envelope.data).toEqual({
      oldId: 'T-03-002-map-the-moorings',
      newId: 'T-03-050-map-the-moorings',
      kind: 'task',
      renamed: [{ oldRelPath: OLD, newRelPath: NEW, contentChanged: true }],
      updated: [
        { relPath: 'proposals/03-harbor-index/tasks/T-03-003-price-the-berths.md', occurrences: 1 },
        { relPath: 'proposals/03-harbor-index/tasks/T-03-004-draft-the-tide-table.md', occurrences: 1 },
        { relPath: 'work/epics/E-07-harbor-plan.md', occurrences: 3 },
      ],
      skipped: [],
      keepsOldId: { frozenCiting: [], namedFiles: [] },
      survivors: [],
      regenerated: ['work/epics/E-07-harbor-plan.md'],
      idMapFile: 'ID-MAP.md',
      errors: [],
      written: true,
      dryRun: false,
    })
  })

  test('says what it did in text: the move and the counts on stdout, which files on stderr', () => {
    const repo = makeRepo()
    const run = renumber(repo, 'T-03-002', 'T-03-050')
    expect(run.code).toBe(0)
    expect(run.stdout.split('\n')).toEqual([
      `${OLD} -> ${NEW} (citation rewritten)`,
      'proposals/03-harbor-index/tasks/T-03-003-price-the-berths.md  1 citation(s) rewritten',
      'proposals/03-harbor-index/tasks/T-03-004-draft-the-tide-table.md  1 citation(s) rewritten',
      'work/epics/E-07-harbor-plan.md  3 citation(s) rewritten',
      '',
    ])
    expect(run.stderr).toContain(`wrote: ${NEW}`)
    expect(run.stderr).toContain('ID-MAP.md')
  })

  test('with --dry-run reports the same and writes nothing', () => {
    const repo = makeRepo()
    const before = repo.snapshot()
    const run = renumber(repo, 'T-03-002', 'T-03-050', '--dry-run', '--format', 'json')
    expect(run.code).toBe(0)
    expect(json(run).data).toMatchObject({
      renamed: [{ oldRelPath: OLD, newRelPath: NEW }],
      regenerated: ['work/epics/E-07-harbor-plan.md'],
      written: false,
      dryRun: true,
    })
    expect(repo.snapshot()).toEqual(before)
    expect(renumber(repo, 'T-03-002', 'T-03-050', '--dry-run').stderr).toContain('would write:')
  })

  const REFUSED: Array<[string, string[], RegExp]> = [
    ['a task that is not there', ['T-03-099', 'T-03-098'], /no task T-03-099/],
    ['a new id that is a task already', ['T-03-002', 'T-03-003'], /T-03-003 already exists/],
    ['a new id in another proposal', ['T-03-002', 'T-05-050'], /proposal 03/],
    ['a new id with a slug', ['T-03-002', 'T-03-050-moorings'], /without a slug/],
    ['an id that is not a task', ['E-07', 'E-09'], /task ids only/],
    ['a proposal', ['P03', 'P09'], /task ids only/],
    ['one id', ['T-03-002'], /needs an old id and a new id/],
    ['three ids', ['T-03-002', 'T-03-050', 'T-03-051'], /takes two arguments/],
    ['a --file when the id has one claimant', ['T-03-002', 'T-03-050', '--file', OLD], /only one claimant/],
    ['the --incoming flag, which this fake does not have', ['T-03-002', 'T-03-050', '--incoming', 'main'], /no flag/],
  ]
  test.each(REFUSED)('refuses %s, with exit 2 and nothing written', (_name, args, message) => {
    const repo = makeRepo()
    const before = repo.snapshot()
    const run = plan(repo, ['renumber', ...args, ...ARCH])
    expect(run.code, run.stdout).toBe(2)
    expect(run.stderr).toMatch(message)
    expect(repo.snapshot()).toEqual(before)
  })

  test('refuses a new id that the id map names on either side, even with --force', () => {
    const repo = makeRepo()
    expect(renumber(repo, 'T-03-002', 'T-03-050').code).toBe(0)
    const before = repo.snapshot()
    const back = renumber(repo, 'T-03-050', 'T-03-002', '--force')
    expect(back.code).toBe(2)
    expect(back.stderr).toContain('id map')
    expect(repo.snapshot()).toEqual(before)
  })

  test('refuses a new id that is cited already, unless --force', () => {
    const repo = makeRepo()
    const path = repo.taskPath('T-03-003')
    repo.write(path, repo.read(path).replace('Made-up body', 'It waits on T-03-077. Made-up body'))
    const before = repo.snapshot()
    const refused = renumber(repo, 'T-03-002', 'T-03-077')
    expect(refused.code).toBe(2)
    expect(refused.stderr).toContain('already cited')
    expect(repo.snapshot()).toEqual(before)
    expect(renumber(repo, 'T-03-002', 'T-03-077', '--force').code).toBe(0)
  })

  describe('a merge collision: two files with one short id', () => {
    const OTHER = 'proposals/03-harbor-index/tasks/T-03-002-map-the-berths.md'
    const collide = (): Repo => {
      const repo = makeRepo()
      repo.write(`arch/${OTHER}`, repo.read(`arch/${OLD}`).replace('Map the moorings', 'Map the berths'))
      return repo
    }

    test('is refused without --file, which names the file to move', () => {
      const repo = collide()
      const before = repo.snapshot()
      const run = renumber(repo, 'T-03-002', 'T-03-050')
      expect(run.code).toBe(2)
      expect(run.stderr).toContain('more than one file')
      expect(repo.snapshot()).toEqual(before)
    })

    test('with --file moves that one, leaves the other, and records it as a survivor', () => {
      const repo = collide()
      const run = renumber(repo, 'T-03-002', 'T-03-050', '--file', OLD, '--format', 'json')
      expect(run.code, run.stderr).toBe(0)
      expect(existsSync(join(repo.root, 'arch', OLD))).toBe(false)
      expect(existsSync(join(repo.root, 'arch', NEW))).toBe(true)
      expect(repo.read(`arch/${OTHER}`)).toContain('Map the berths')
      expect(json(run).data.survivors).toEqual([OTHER])
      expect(repo.read('arch/ID-MAP.md').trimEnd().split('\n').at(-1)).toMatch(
        new RegExp(`\\| moved: ${OLD}; survivors: ${OTHER} \\|$`),
      )
    })

    test('takes --file from the root, or from the repository root, and refuses a file that is no claimant', () => {
      expect(renumber(collide(), 'T-03-002', 'T-03-050', '--file', `arch/${OLD}`).code).toBe(0)
      const repo = collide()
      const wrong = renumber(repo, 'T-03-002', 'T-03-050', '--file', repo.taskPath('T-03-003'))
      expect(wrong.code).toBe(2)
      expect(wrong.stderr).toContain('does not match')
    })

    test('leaves a bare citation alone, since it cannot say which file it meant, and reports it', () => {
      const repo = collide()
      const run = renumber(repo, 'T-03-002', 'T-03-050', '--file', OLD, '--format', 'json')
      expect(repo.read(repo.taskPath('T-03-003'))).toContain('depends_on: [T-03-001, T-03-002]')
      expect(json(run).data.skipped).toContainEqual({
        relPath: 'proposals/03-harbor-index/tasks/T-03-003-price-the-berths.md',
        citations: ['T-03-002'],
        reasons: ['ambiguous'],
      })
    })
  })
})
