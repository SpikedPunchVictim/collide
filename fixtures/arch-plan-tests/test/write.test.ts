import { readFileSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { Ajv2020 } from 'ajv/dist/2020.js'
import { describe, expect, test } from 'vitest'
import { REPO } from './helpers.ts'
import { makeRepo, plan, versionOf } from './repo.ts'
import type { Repo } from './repo.ts'

// The write side of the adapter: `apply` and `preview` for `setStatus` and `create`, run through the repository's own
// `./plan`, which here is the fake plan CLI (or, in a few tests, a script that misbehaves on purpose).

type Client = Awaited<ReturnType<Repo['provider']>>
type Message = Record<string, any>

const E07 = 'arch/work/epics/E-07-harbor-plan.md'
const E08 = 'arch/work/epics/E-08-signal-lamps.md'
const ILLEGAL = { op: 'setStatus', id: 'T-03-003', args: { status: 'done' } }

/** Runs `test` with a provider for the repository, and closes it. */
async function using<T>(repo: Repo, test: (client: Client) => Promise<T>): Promise<T> {
  const client = await repo.provider()
  try {
    return await test(client)
  } finally {
    await client.close()
  }
}

/** `apply` or `preview` of a call on one task, which sends the version the task's file has now. */
const callOn = async (client: Client, repo: Repo, method: string, params: Message) => {
  const shortId = /T-\d{2}-\d{3}/.exec(String(params.id))?.[0]
  const version = shortId === undefined ? undefined : versionOf(repo, repo.taskPath(shortId))
  return client.request(method, {
    ...(version === undefined ? {} : { version }),
    ...params,
  })
}
const apply = (client: Client, repo: Repo, params: Message) => callOn(client, repo, 'apply', params)
const preview = (client: Client, repo: Repo, params: Message) => callOn(client, repo, 'preview', params)

const statusLine = (repo: Repo, path: string): string => /^status: (.*)$/m.exec(repo.read(path))?.[1] ?? ''
const porcelain = (repo: Repo): string[] => repo.git('status', '--porcelain').split('\n').filter(Boolean).sort()
const state = (status: string, category: string) => ({ status, category })

describe('setStatus', () => {
  test('by status: ok, the task as written, the two files touched, and nothing committed', async () => {
    const repo = makeRepo()
    const path = repo.taskPath('T-03-003')
    const head = repo.head()
    const reply = await using(repo, (client) =>
      apply(client, repo, {
        op: 'setStatus',
        id: 'T-03-003-price-the-berths',
        args: { status: 'doing' },
      }),
    )
    expect(reply.error).toBeUndefined()
    const version = versionOf(repo, path)
    expect(reply.result).toMatchObject({
      outcome: 'ok',
      version,
      task: {
        key: 'plan:T-03-003',
        state: state('doing', 'active'),
        version,
        path,
      },
      files: [path, E07].sort(),
      items: [
        {
          key: 'plan:T-03-003',
          version,
          from: state('ready', 'ready'),
          to: state('doing', 'active'),
        },
      ],
    })
    expect(statusLine(repo, path)).toBe('doing')
    expect(repo.read(E07)).toContain('T-03-003-price-the-berths.md) | doing |')
    expect(repo.head()).toBe(head)
    expect(porcelain(repo)).toEqual([` M ${path}`, ` M ${E07}`].sort())
  })

  test('the id may be the long id, the short id or the key', async () => {
    for (const id of ['T-03-003-price-the-berths', 'T-03-003', 'plan:T-03-003']) {
      const repo = makeRepo()
      const reply = await using(repo, (client) =>
        callOn(client, repo, 'apply', {
          op: 'setStatus',
          id,
          args: { status: 'doing' },
        }),
      )
      expect(reply.result?.outcome, id).toBe('ok')
    }
  })

  const CATEGORIES: Array<[string, string, string]> = [
    ['ready', 'T-03-004', 'ready'],
    ['active', 'T-03-003', 'doing'],
    ['review', 'T-03-002', 'in_review'],
    ['dropped', 'T-03-003', 'dropped'],
    ['done', 'T-05-001', 'done'],
  ]
  test.each(CATEGORIES)(
    'by category %s it writes the status the category names, %s becomes %s',
    async (category, shortId, status) => {
      const repo = makeRepo()
      const path = repo.taskPath(shortId)
      // A task is done only with a review on record (the plan CLI's rule).
      if (status === 'done')
        repo.write(
          path,
          repo.read(path).replace('status: in_review', 'status: in_review\nreviewed_by: "a made-up person"'),
        )
      const reply = await using(repo, (client) =>
        apply(client, repo, {
          op: 'setStatus',
          id: shortId,
          args: { category },
        }),
      )
      expect(reply.result).toMatchObject({
        outcome: 'ok',
        task: { state: { status, category } },
      })
      expect(statusLine(repo, path)).toBe(status)
    },
  )

  test('by category backlog on a task that is already specified changes nothing and lists no file', async () => {
    const repo = makeRepo()
    const before = repo.snapshot()
    const version = versionOf(repo, repo.taskPath('T-03-004'))
    const reply = await using(repo, (client) =>
      apply(client, repo, {
        op: 'setStatus',
        id: 'T-03-004',
        args: { category: 'backlog' },
      }),
    )
    expect(reply.result).toMatchObject({
      outcome: 'ok',
      version,
      files: [],
      items: [
        {
          key: 'plan:T-03-004',
          version,
          from: state('specified', 'backlog'),
          to: state('specified', 'backlog'),
        },
      ],
    })
    expect(repo.snapshot()).toEqual(before)
  })

  test('an unrelated edit under arch/ is not listed, and is left as it was', async () => {
    const repo = makeRepo()
    const other = repo.taskPath('T-05-002')
    repo.write(other, `${repo.read(other)}\nAn edit that is nothing to do with the write.\n`)
    repo.write('arch/README.md', `${repo.read('arch/README.md')}\nAnother.\n`)
    const edited = [other, 'arch/README.md'].map((path) => [path, repo.read(path)] as const)
    const reply = await using(repo, (client) =>
      apply(client, repo, {
        op: 'setStatus',
        id: 'T-03-003',
        args: { status: 'doing' },
      }),
    )
    expect(reply.result?.files).toEqual([repo.taskPath('T-03-003'), E07].sort())
    for (const [path, text] of edited) expect(repo.read(path)).toBe(text)
    expect(porcelain(repo)).toContain(` M ${other}`)
  })

  test("an illegal transition is refused with the plan CLI's own message, and nothing is written", async () => {
    const repo = makeRepo()
    const head = repo.head()
    const before = repo.snapshot()
    const reply = await using(repo, (client) => apply(client, repo, ILLEGAL))
    const own = plan(makeRepo(), ['set', 'T-03-003', 'status=done', '--root', 'arch'])
    expect(own.code).toBe(2)
    expect(reply.result).toEqual({
      outcome: 'refused',
      message: own.stderr.trim(),
    })
    expect(reply.result?.message).toContain('ready cannot move to done')
    expect(repo.snapshot()).toEqual(before)
    expect(repo.head()).toBe(head)
  })

  test("a category with no legal move from the task's status is refused in the same way", async () => {
    const repo = makeRepo()
    const before = repo.snapshot()
    const reply = await using(repo, (client) =>
      apply(client, repo, {
        op: 'setStatus',
        id: 'T-03-003',
        args: { category: 'backlog' },
      }),
    )
    expect(reply.result).toMatchObject({
      outcome: 'refused',
      message: expect.stringContaining('ready cannot move to specified'),
    })
    expect(repo.snapshot()).toEqual(before)
  })

  test('a category that names no status is refused with a message', async () => {
    const repo = makeRepo()
    const before = repo.snapshot()
    const reply = await using(repo, (client) =>
      apply(client, repo, {
        op: 'setStatus',
        id: 'T-03-003',
        args: { category: 'blocked' },
      }),
    )
    expect(reply.result).toMatchObject({
      outcome: 'refused',
      message: expect.stringContaining('blocked'),
    })
    expect(repo.snapshot()).toEqual(before)
  })

  test('a task that is not there is refused, and the CLI is not run', async () => {
    const repo = makeRepo()
    const reply = await using(repo, (client) =>
      client.request('apply', {
        op: 'setStatus',
        id: 'T-03-099',
        args: { status: 'doing' },
        version: 'x',
      }),
    )
    expect(reply.result).toMatchObject({
      outcome: 'refused',
      message: expect.stringContaining('T-03-099'),
    })
  })

  test("a task that went to done without a review is refused with the CLI's message", async () => {
    const repo = makeRepo()
    const reply = await using(repo, (client) =>
      apply(client, repo, {
        op: 'setStatus',
        id: 'T-05-001',
        args: { status: 'done' },
      }),
    )
    expect(reply.result).toMatchObject({
      outcome: 'refused',
      message: expect.stringContaining('reviewed_by or self_review'),
    })
  })

  test('an old version is a conflict with the current task, and nothing is written', async () => {
    const repo = makeRepo()
    const path = repo.taskPath('T-03-003')
    const old = versionOf(repo, path)
    repo.write(path, `${repo.read(path)}\nEdited after the caller read it.\n`)
    const before = repo.snapshot()
    const reply = await using(repo, (client) =>
      client.request('apply', {
        op: 'setStatus',
        id: 'T-03-003',
        args: { status: 'doing' },
        version: old,
      }),
    )
    expect(reply.result).toMatchObject({
      outcome: 'conflict',
      current: {
        key: 'plan:T-03-003',
        state: state('ready', 'ready'),
        version: versionOf(repo, path),
      },
    })
    expect(repo.snapshot()).toEqual(before)
  })

  const BAD_CALLS: Array<[string, Message, RegExp]> = [
    ['no id', { op: 'setStatus', args: { status: 'doing' }, version: 'x' }, /needs the id/],
    ['no version', { op: 'setStatus', id: 'T-03-003', args: { status: 'doing' } }, /needs the version/],
    [
      'neither status nor category',
      { op: 'setStatus', id: 'T-03-003', args: {}, version: 'x' },
      /one of status or category/,
    ],
    [
      'both status and category',
      {
        op: 'setStatus',
        id: 'T-03-003',
        args: { status: 'doing', category: 'active' },
        version: 'x',
      },
      /one of status or category/,
    ],
    [
      'an argument it does not take',
      {
        op: 'setStatus',
        id: 'T-03-003',
        args: { status: 'doing', force: true },
        version: 'x',
      },
      /unknown argument "force"/,
    ],
    [
      'a status that is not declared',
      {
        op: 'setStatus',
        id: 'T-03-003',
        args: { status: 'started' },
        version: 'x',
      },
      /not declared/,
    ],
    [
      'a status that is not text',
      { op: 'setStatus', id: 'T-03-003', args: { status: 3 }, version: 'x' },
      /status must be a non-empty string/,
    ],
    ['an op it has not got', { op: 'rename', id: 'T-03-003', args: {}, version: 'x' }, /no op "rename"/],
  ]
  test.each(BAD_CALLS)(
    'a call the adapter cannot build is an error reply, not a refusal: %s',
    async (_name, params, message) => {
      const repo = makeRepo()
      const before = repo.snapshot()
      const reply = await using(repo, (client) => client.request('apply', params))
      expect(reply.result).toBeUndefined()
      expect(reply.error).toMatchObject({
        code: -32603,
        message: expect.stringMatching(message),
      })
      expect(repo.snapshot()).toEqual(before)
    },
  )

  test('a checkout other than the one it was given is an error reply', async () => {
    const repo = makeRepo()
    const reply = await using(repo, (client) =>
      client.request('apply', {
        ...ILLEGAL,
        version: 'x',
        checkout: join(repo.root, 'elsewhere'),
      }),
    )
    expect(reply.error?.message).toMatch(/checkout/)
  })
})

describe('create', () => {
  const CHART = {
    op: 'create',
    args: { proposal: 'P03', title: 'Chart the tides', epic: 'E-07' },
  }

  test('ok with the new task, the id the CLI gave it and the new file and its epic as the files touched', async () => {
    const repo = makeRepo()
    const head = repo.head()
    const path = 'arch/proposals/03-harbor-index/tasks/T-03-006-chart-the-tides.md'
    const reply = await using(repo, (client) => client.request('apply', CHART))
    expect(reply.error).toBeUndefined()
    const version = versionOf(repo, path)
    expect(reply.result).toMatchObject({
      outcome: 'ok',
      version,
      task: {
        key: 'plan:T-03-006',
        id: 'T-03-006-chart-the-tides',
        title: 'Chart the tides',
        state: state('specified', 'backlog'),
        version,
        path,
        fields: { proposal: '03', epic: 'E-07' },
        collections: ['plan:E-07'],
      },
      files: [path, E07].sort(),
      items: [
        {
          key: 'plan:T-03-006',
          version,
          from: null,
          to: state('specified', 'backlog'),
        },
      ],
    })
    expect(repo.read(E07)).toContain('T-03-006-chart-the-tides.md) | specified |')
    expect(repo.head()).toBe(head)
    expect(porcelain(repo)).toEqual([` M ${E07}`, `?? ${path}`].sort())
  })

  test('takes dependencies, which show as links, and the new task can be read with get', async () => {
    const repo = makeRepo()
    const args = {
      proposal: 'P05',
      title: 'Hang the lamps',
      epic: 'E-08',
      dependsOn: ['T-05-001', 'T-05-003'],
    }
    const { created, found } = await using(repo, async (client) => ({
      created: await client.request('apply', { op: 'create', args }),
      found: await client.request('get', { id: 'T-05-004' }),
    }))
    expect(created.result?.task.links).toEqual([
      { type: 'blocked-by', to: 'T-05-001' },
      { type: 'blocked-by', to: 'T-05-003' },
    ])
    expect(created.result?.files).toEqual(['arch/proposals/05-lamp-works/tasks/T-05-004-hang-the-lamps.md', E08].sort())
    expect(found.result?.task).toMatchObject({
      key: 'plan:T-05-004',
      title: 'Hang the lamps',
    })
  })

  test('a refusal of the write gate is a refusal with its message, and nothing is written', async () => {
    const repo = makeRepo()
    const before = repo.snapshot()
    const reply = await using(repo, (client) =>
      client.request('apply', {
        op: 'create',
        args: { ...CHART.args, epic: 'E-99' },
      }),
    )
    expect(reply.result).toMatchObject({
      outcome: 'refused',
      message: expect.stringContaining('epic E-99 has no epic file'),
    })
    expect(repo.snapshot()).toEqual(before)
  })

  test("a proposal the CLI does not know is a refusal with the CLI's message", async () => {
    const repo = makeRepo()
    const reply = await using(repo, (client) =>
      client.request('apply', {
        op: 'create',
        args: { ...CHART.args, proposal: 'P99' },
      }),
    )
    expect(reply.result).toMatchObject({
      outcome: 'refused',
      message: expect.stringContaining('P99'),
    })
  })

  const BAD_CALLS: Array<[string, Message, RegExp]> = [
    ['an id', { ...CHART, id: 'T-03-001' }, /leave id out/],
    ['a version', { ...CHART, version: 'x' }, /no version/],
    ['no title', { op: 'create', args: { proposal: 'P03', epic: 'E-07' } }, /title must be a non-empty/],
    ['no epic', { op: 'create', args: { proposal: 'P03', title: 'Chart' } }, /epic must be a non-empty/],
    ['no proposal', { op: 'create', args: { title: 'Chart', epic: 'E-07' } }, /proposal must be a non-empty/],
    [
      'a proposal that is not P and two digits',
      { op: 'create', args: { ...CHART.args, proposal: '3' } },
      /P and two digits/,
    ],
    ['an epic that is not an epic id', { op: 'create', args: { ...CHART.args, epic: 'harbor' } }, /epic id/],
    [
      'a title that starts with a dash',
      { op: 'create', args: { ...CHART.args, title: '--force' } },
      /not starting with a dash/,
    ],
    ['a title with a control character', { op: 'create', args: { ...CHART.args, title: 'Two\nlines' } }, /one line/],
    ['a title that is empty', { op: 'create', args: { ...CHART.args, title: '' } }, /title must be a non-empty/],
    [
      'a dependency that is not a task id',
      { op: 'create', args: { ...CHART.args, dependsOn: ['harbor'] } },
      /dependsOn/,
    ],
    [
      'an argument it does not take',
      { op: 'create', args: { ...CHART.args, owner: 'me' } },
      /unknown argument "owner"/,
    ],
  ]
  test.each(BAD_CALLS)('a call the adapter cannot build is an error reply: %s', async (_name, params, message) => {
    const repo = makeRepo()
    const before = repo.snapshot()
    const reply = await using(repo, (client) => client.request('apply', params))
    expect(reply.result).toBeUndefined()
    expect(reply.error).toMatchObject({
      code: -32603,
      message: expect.stringMatching(message),
    })
    expect(repo.snapshot()).toEqual(before)
  })
})

describe('when the plan CLI misbehaves', () => {
  const script = (body: string) => `#!/bin/sh\n${body}\n`
  const envelope = (data: object, version = 1) =>
    JSON.stringify({
      schema_version: version,
      command: 'set',
      corpus: 'arch',
      data,
    })
  const SET = { op: 'setStatus', id: 'T-03-003', args: { status: 'doing' } }

  test.each([
    ['exits with a code it does not define', 'echo oops >&2\nexit 3', /exited with 3/],
    ['exits 0 and prints something that is not JSON', 'echo not json', /not JSON/],
    ['exits 0 with an envelope of another version', `echo '${envelope({}, 2)}'`, /schema_version/],
    ['exits 0 with errors in the envelope', `echo '${envelope({ errors: [{ message: 'x' }] })}'`, /errors/],
  ])('an error reply when it %s', async (_name, body, message) => {
    const repo = makeRepo('basic', script(body))
    const reply = await using(repo, (client) => apply(client, repo, SET))
    expect(reply.result).toBeUndefined()
    expect(reply.error?.message).toMatch(message)
  })

  test('an error reply when ./plan is not there', async () => {
    const repo = makeRepo()
    rmSync(join(repo.root, 'plan'))
    const reply = await using(repo, (client) => apply(client, repo, SET))
    expect(reply.error?.message).toMatch(/cannot run \.\/plan/)
  })

  test('an error reply, and the child killed, when it does not finish in time', async () => {
    const repo = makeRepo('basic', script('exec sleep 30'))
    const started = Date.now()
    const client = await repo.provider({ timeoutMs: 300 })
    try {
      const reply = await apply(client, repo, SET)
      expect(reply.error?.message).toMatch(/did not finish/)
    } finally {
      await client.close()
    }
    expect(Date.now() - started).toBeLessThan(20_000)
  })

  test('exit 1 with an envelope of errors is a refusal with the first error, with its file', async () => {
    const error = {
      file: 'x.md',
      severity: 'error',
      rule: 'r',
      message: 'made-up reason',
    }
    const repo = makeRepo('basic', script(`echo '${envelope({ errors: [error, error] })}'\nexit 1`))
    const reply = await using(repo, (client) => apply(client, repo, SET))
    expect(reply.result).toEqual({
      outcome: 'refused',
      message: 'x.md: made-up reason (and 1 more)',
    })
  })

  test('exit 1 with no envelope is the CLI failing, not its write gate: an error reply with its message', async () => {
    const repo = makeRepo('basic', script('echo "Internal error: made-up trouble" >&2\nexit 1'))
    const reply = await using(repo, (client) => apply(client, repo, SET))
    expect(reply.result).toBeUndefined()
    expect(reply.error?.message).toContain('made-up trouble')
  })

  test('exit 2 with a message on stderr is a refusal with it', async () => {
    const repo = makeRepo('basic', script('echo "made-up trouble" >&2\nexit 2'))
    const reply = await using(repo, (client) => apply(client, repo, SET))
    expect(reply.result).toEqual({
      outcome: 'refused',
      message: 'made-up trouble',
    })
  })

  test('a failure that left a file changed is an error reply that names the file', async () => {
    const repo = makeRepo('basic', script('echo changed >> arch/README.md\necho "made-up trouble" >&2\nexit 1'))
    const reply = await using(repo, (client) => apply(client, repo, SET))
    expect(reply.result).toBeUndefined()
    expect(reply.error?.message).toContain('arch/README.md')
  })

  test('a dry run that changed a file is an error reply that names the file', async () => {
    const repo = makeRepo('basic', script(`echo changed >> arch/README.md\necho '${envelope({})}'`))
    const reply = await using(repo, (client) => preview(client, repo, SET))
    expect(reply.result).toBeUndefined()
    expect(reply.error?.message).toContain('arch/README.md')
  })

  test('the CLI runs from the root, the command first, and --root names the arch folder of the checkout', async () => {
    const body = 'pwd > arch/cwd.txt\nprintf "%s\\n" "$@" > arch/args.txt\nexit 2'
    const repo = makeRepo('basic', script(body))
    await using(repo, (client) => apply(client, repo, SET))
    expect(repo.read('arch/cwd.txt').trim()).toMatch(/repo$/)
    expect(repo.read('arch/args.txt').trim().split('\n')).toEqual([
      'set',
      'T-03-003',
      'status=doing',
      '--root',
      join(repo.root, 'arch'),
      '--format',
      'json',
    ])
  })
})

describe('preview', () => {
  const CASES: Array<[string, Message, 'set' | 'create']> = [
    ['setStatus by status', { op: 'setStatus', id: 'T-03-003', args: { status: 'doing' } }, 'set'],
    ['setStatus by category', { op: 'setStatus', id: 'T-03-002', args: { category: 'review' } }, 'set'],
    ['setStatus to the same status', { op: 'setStatus', id: 'T-03-004', args: { status: 'specified' } }, 'set'],
    [
      'create',
      {
        op: 'create',
        args: { proposal: 'P03', title: 'Chart the tides', epic: 'E-07' },
      },
      'create',
    ],
    [
      'create with dependencies',
      {
        op: 'create',
        args: {
          proposal: 'P05',
          title: 'Hang',
          epic: 'E-08',
          dependsOn: ['T-05-001'],
        },
      },
      'create',
    ],
  ]

  test.each(CASES)(
    '%s: the same files and items as the write, and the tree is byte for byte as it was',
    async (_name, params, kind) => {
      const dry = makeRepo()
      const real = makeRepo()
      const before = dry.snapshot()
      const head = dry.head()
      const previewed = await using(dry, (client) =>
        kind === 'set' ? preview(client, dry, params) : client.request('preview', params),
      )
      const written = await using(real, (client) =>
        kind === 'set' ? apply(client, real, params) : client.request('apply', params),
      )
      expect(dry.snapshot()).toEqual(before)
      expect(dry.head()).toBe(head)
      expect(porcelain(dry)).toEqual([])
      expect(previewed.result?.outcome).toBe('ok')
      expect(Object.keys(previewed.result ?? {}).sort()).toEqual(['files', 'items', 'outcome'])
      expect(previewed.result?.files).toEqual(written.result?.files)
      const versionless = (items: Message[]) => items.map(({ version: _version, ...item }) => item)
      expect(versionless(previewed.result?.items)).toEqual(versionless(written.result?.items))
    },
  )

  test('the version of an item is the one the task has now: a dry run writes no file to hash', async () => {
    const repo = makeRepo()
    const version = versionOf(repo, repo.taskPath('T-03-003'))
    const reply = await using(repo, (client) =>
      preview(client, repo, {
        op: 'setStatus',
        id: 'T-03-003',
        args: { status: 'doing' },
      }),
    )
    expect(reply.result?.items[0].version).toBe(version)
  })

  test('a task that does not exist yet has the version ""', async () => {
    const repo = makeRepo()
    const reply = await using(repo, (client) =>
      client.request('preview', {
        op: 'create',
        args: { proposal: 'P03', title: 'Chart', epic: 'E-07' },
      }),
    )
    expect(reply.result?.items).toEqual([
      {
        key: 'plan:T-03-006',
        version: '',
        from: null,
        to: state('specified', 'backlog'),
      },
    ])
  })

  test('a refusal comes back as the write would give it, with nothing written', async () => {
    const dry = makeRepo()
    const real = makeRepo()
    const before = dry.snapshot()
    const previewed = await using(dry, (client) => preview(client, dry, ILLEGAL))
    const written = await using(real, (client) => apply(client, real, ILLEGAL))
    expect(previewed.result).toEqual(written.result)
    expect(previewed.result?.outcome).toBe('refused')
    expect(dry.snapshot()).toEqual(before)
  })

  test('a conflict comes back as the write would give it', async () => {
    const repo = makeRepo()
    const path = repo.taskPath('T-03-003')
    const old = versionOf(repo, path)
    repo.write(path, `${repo.read(path)}\nEdited.\n`)
    const params = {
      op: 'setStatus',
      id: 'T-03-003',
      args: { status: 'doing' },
      version: old,
    }
    const { previewed, written } = await using(repo, async (client) => ({
      previewed: await client.request('preview', params),
      written: await client.request('apply', params),
    }))
    expect(previewed.result).toEqual(written.result)
    expect(previewed.result?.outcome).toBe('conflict')
  })

  test('a preview is an error reply for a call the adapter cannot build', async () => {
    const repo = makeRepo()
    const reply = await using(repo, (client) =>
      client.request('preview', {
        op: 'setStatus',
        id: 'T-03-003',
        args: {},
        version: 'x',
      }),
    )
    expect(reply.error).toMatchObject({ code: -32603 })
  })
})

// A provider's result is checked against the input schemas: they leave out what core fills in (empty lists).
describe('the generated schemas', () => {
  const validator = (name: string) => {
    const schema = JSON.parse(readFileSync(join(REPO, 'schemas', `${name}.input.json`), 'utf8'))
    return new Ajv2020({ strict: false, allErrors: true }).compile(schema)
  }

  test('apply: ok, conflict and refused validate against ApplyResult', async () => {
    const validate = validator('ApplyResult')
    const repo = makeRepo()
    const results = await using(repo, async (client) => [
      (
        await apply(client, repo, {
          op: 'setStatus',
          id: 'T-03-003',
          args: { status: 'doing' },
        })
      ).result,
      (await apply(client, repo, ILLEGAL)).result,
      (
        await client.request('apply', {
          op: 'create',
          args: { proposal: 'P03', title: 'Chart', epic: 'E-07' },
        })
      ).result,
      (
        await client.request('apply', {
          op: 'setStatus',
          id: 'T-03-004',
          args: { status: 'ready' },
          version: 'stale',
        })
      ).result,
    ])
    expect(results.map((result) => result?.outcome)).toEqual(['ok', 'refused', 'ok', 'conflict'])
    for (const result of results) expect(validate(result), JSON.stringify(validate.errors)).toBe(true)
  })

  test('preview: ok, conflict and refused validate against PreviewResult', async () => {
    const validate = validator('PreviewResult')
    const repo = makeRepo()
    const results = await using(repo, async (client) => [
      (
        await preview(client, repo, {
          op: 'setStatus',
          id: 'T-03-003',
          args: { status: 'doing' },
        })
      ).result,
      (await preview(client, repo, ILLEGAL)).result,
      (
        await client.request('preview', {
          op: 'create',
          args: { proposal: 'P03', title: 'Chart', epic: 'E-07' },
        })
      ).result,
      (
        await client.request('preview', {
          op: 'setStatus',
          id: 'T-03-004',
          args: { status: 'ready' },
          version: 'stale',
        })
      ).result,
    ])
    expect(results.map((result) => result?.outcome)).toEqual(['ok', 'refused', 'ok', 'conflict'])
    for (const result of results) expect(validate(result), JSON.stringify(validate.errors)).toBe(true)
  })

  test('the check can fail: an ok without its files does not validate', () => {
    expect(validator('ApplyResult')({ outcome: 'ok', version: 'v', items: [] })).toBe(false)
    expect(
      validator('PreviewResult')({
        outcome: 'ok',
        items: [],
        files: [],
        task: {},
      }),
    ).toBe(false)
  })
})

describe('writes in sequence', () => {
  test('eight setStatus calls at once run one after the other, and each answer is true to the disk', async () => {
    const repo = makeRepo()
    const head = repo.head()
    const same = { op: 'setStatus', id: 'T-03-003', args: { status: 'doing' } }
    const others: Message[] = [
      { op: 'setStatus', id: 'T-03-004', args: { status: 'ready' } },
      { op: 'setStatus', id: 'T-03-002', args: { status: 'in_review' } },
      { op: 'setStatus', id: 'T-05-002', args: { status: 'ready' } },
      { op: 'setStatus', id: 'T-05-001', args: { status: 'doing' } },
    ]
    // Every call carries the version its task had before any of them ran, so of the four on one task only one wins.
    const calls = [same, same, others[0], same, others[1], same, others[2], others[3]] as Message[]
    const withVersion = calls.map((call) => ({
      ...call,
      version: versionOf(repo, repo.taskPath(String(call.id))),
    }))
    const replies = await using(repo, (client) => client.requestMany('apply', withVersion))

    expect(replies.map((reply) => reply.result?.outcome)).toEqual([
      'ok',
      'conflict',
      'ok',
      'conflict',
      'ok',
      'conflict',
      'ok',
      'ok',
    ])
    for (const [at, reply] of replies.entries()) {
      const call = calls[at] as Message
      const path = repo.taskPath(String(call.id))
      expect(reply.error).toBeUndefined()
      if (reply.result?.outcome === 'ok') {
        expect(reply.result.version).toBe(versionOf(repo, path))
        expect(reply.result.task.state.status).toBe(statusLine(repo, path))
        expect(reply.result.files).toHaveLength(2)
        expect(reply.result.files).toContain(path)
      } else {
        expect(reply.result?.current.version).toBe(versionOf(repo, path))
        expect(reply.result?.current.state.status).toBe(statusLine(repo, path))
      }
    }
    // The epic tables, regenerated by each write in turn, agree with every task file at the end.
    for (const epic of [E07, E08]) {
      for (const [, shortId, status] of repo.read(epic).matchAll(/\[(T-\d{2}-\d{3})\]\([^)]*\) \| (\w+) \|/g)) {
        expect(status, `${shortId} in ${epic}`).toBe(statusLine(repo, repo.taskPath(shortId as string)))
      }
    }
    expect(repo.head()).toBe(head)
  })
})
