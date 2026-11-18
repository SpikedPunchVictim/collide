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
      expect(run.stderr).toContain(`${from} cannot move to ${to}`)
      expect(statusLine(repo, path)).toBe(from)
    }
  })

  test('a refusal says where the task can go', () => {
    repo.write(path, repo.read(path).replace(/^status: .*$/m, 'status: ready'))
    expect(set(repo, 'T-03-002', 'done').stderr).toContain('it can move to doing, dropped')
    repo.write(path, repo.read(path).replace(/^status: .*$/m, 'status: dropped'))
    expect(set(repo, 'T-03-002', 'ready').stderr).toContain('it is final')
  })
})

describe('set', () => {
  test('changes the status line of the task file and nothing else in it', () => {
    const repo = makeRepo()
    const path = repo.taskPath('T-03-003')
    const before = repo.read(path)
    expect(set(repo, 'T-03-003', 'doing').code).toBe(0)
    expect(repo.read(path)).toBe(before.replace('status: ready', 'status: doing'))
  })

  test('takes the file stem as the id as well as the short id', () => {
    const repo = makeRepo()
    expect(set(repo, 'T-03-003-price-the-berths', 'doing').code).toBe(0)
    expect(statusLine(repo, repo.taskPath('T-03-003'))).toBe('doing')
    expect(set(repo, 'T-03-002-price-the-berths', 'in_review').code).toBe(2)
  })

  test('refreshes the generated table of the epic that holds the task, and no other file', () => {
    const repo = makeRepo()
    const before = repo.snapshot()
    expect(set(repo, 'T-03-003', 'doing').code).toBe(0)
    const after = repo.snapshot()
    const changed = Object.keys(after).filter((path) => after[path] !== before[path])
    expect(changed.sort()).toEqual([repo.taskPath('T-03-003'), 'arch/work/epics/E-07-harbor-plan.md'].sort())
    const table = repo.read('arch/work/epics/E-07-harbor-plan.md')
    expect(table).toContain('proposals/03-harbor-index/tasks/T-03-003-price-the-berths.md) | doing |')
    expect(table).toContain('T-03-002-map-the-moorings.md) | doing |')
  })

  test('refreshes a table that is out of step, but leaves the prose and the other tables of the file', () => {
    const repo = makeRepo()
    const epic = 'arch/work/epics/E-07-harbor-plan.md'
    const original = repo.read(epic)
    repo.write(epic, original.replace('| ready |', '| specified |'))
    expect(set(repo, 'T-03-003', 'doing').code).toBe(0)
    expect(repo.read(epic)).toBe(
      original.replace('T-03-003-price-the-berths.md) | ready |', 'T-03-003-price-the-berths.md) | doing |'),
    )
  })

  test('a move to the status the task has changes nothing and refreshes nothing', () => {
    const repo = makeRepo()
    const before = repo.snapshot()
    const run = set(repo, 'T-03-003', 'ready', '--format', 'json')
    expect(run.code).toBe(0)
    expect(json(run).data.regenerated).toEqual([])
    expect(repo.snapshot()).toEqual(before)
  })

  test('a task goes to done only with a review on record', () => {
    const repo = makeRepo()
    const path = repo.taskPath('T-05-001')
    const before = repo.snapshot()
    const refused = set(repo, 'T-05-001', 'done')
    expect(refused.code).toBe(2)
    expect(refused.stderr).toContain('reviewed_by or self_review')
    expect(repo.snapshot()).toEqual(before)
    repo.write(path, repo.read(path).replace('status: in_review', 'status: in_review\nreviewed_by: "a made-up person"'))
    expect(set(repo, 'T-05-001', 'done').code).toBe(0)
    expect(statusLine(repo, path)).toBe('done')
  })

  test('says what it did in text: the changes on stdout, which files on stderr', () => {
    const repo = makeRepo()
    const run = plan(repo, ['set', 'T-03-003', 'status=doing', ...ARCH])
    expect(run.code).toBe(0)
    expect(run.stdout).toBe(
      'proposals/03-harbor-index/tasks/T-03-003-price-the-berths.md  status: "ready" -> "doing"\n',
    )
    expect(run.stderr).toContain(
      'wrote: proposals/03-harbor-index/tasks/T-03-003-price-the-berths.md, work/epics/E-07-harbor-plan.md',
    )
  })
})

describe('the JSON envelope', () => {
  test('is schema_version 1 with the command, the corpus and the data, as the only thing on stdout', () => {
    const repo = makeRepo()
    const run = set(repo, 'T-03-003', 'doing', '--format', 'json')
    expect(run.code).toBe(0)
    expect(Object.keys(json(run))).toEqual(['schema_version', 'command', 'corpus', 'data'])
    expect(json(run)).toMatchObject({
      schema_version: 1,
      command: 'set',
      corpus: 'arch',
    })
    expect(json(run).data).toEqual({
      results: [
        {
          relPath: 'proposals/03-harbor-index/tasks/T-03-003-price-the-berths.md',
          changes: [{ field: 'status', oldValue: 'ready', newValue: 'doing' }],
          written: true,
        },
      ],
      dryRun: false,
      errors: [],
      regenerated: ['work/epics/E-07-harbor-plan.md'],
    })
  })

  test('PLAN_FORMAT and PLAN_ROOT stand in for the flags', () => {
    const repo = makeRepo()
    const run = plan(repo, ['set', 'T-03-003', 'status=doing'], {
      PLAN_FORMAT: 'json',
      PLAN_ROOT: join(repo.root, 'arch'),
    })
    expect(run.code, run.stderr).toBe(0)
    expect(json(run).command).toBe('set')
  })
})

describe('--dry-run', () => {
  test('writes nothing, says written is false, and lists the files the write changes', () => {
    const repo = makeRepo()
    const before = repo.snapshot()
    const run = set(repo, 'T-03-003', 'doing', '--format', 'json', '--dry-run')
    expect(run.code).toBe(0)
    expect(repo.snapshot()).toEqual(before)
    expect(json(run).data).toMatchObject({
      dryRun: true,
      results: [{ written: false }],
    })
    expect(json(run).data.regenerated).toEqual(['work/epics/E-07-harbor-plan.md'])
  })

  test('reports what the real write reports, but for written and dryRun', () => {
    const real = makeRepo()
    const dry = makeRepo()
    const written = json(set(real, 'T-03-003', 'doing', '--format', 'json')).data
    const reported = json(set(dry, 'T-03-003', 'doing', '--format', 'json', '--dry-run')).data
    expect({
      ...reported,
      dryRun: false,
      results: [{ ...reported.results[0], written: true }],
    }).toEqual(written)
  })

  test('refuses a move the real write would refuse, in the same way', () => {
    const repo = makeRepo()
    const run = set(repo, 'T-03-003', 'done', '--dry-run')
    expect(run.code).toBe(2)
    expect(run.stderr).toContain('ready cannot move to done')
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
    ['a flag it has not got', ['set', 'T-03-003', 'status=doing', '--force', ...ARCH]],
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
      id: {
        kind: 'task',
        num: '006',
        suffix: '',
        scope: '03',
        slug: 'chart-the-tides',
      },
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
    expect(json(run).data.id).toMatchObject({ scope: '05', num: '005' })
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
