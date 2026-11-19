import { existsSync, readFileSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { makeRepo, plan } from './repo.ts'
import type { Repo } from './repo.ts'

// The human queue: the open questions that need a person. The provider does not decide which they are. It runs the
// repository's own `./plan status --format json` and gives the questions in its `data.queue`, in its order, as the
// collection `plan:queue`. A view reads that collection by name. The fake plan CLI answers from the tree; a test that
// needs another answer puts a script that prints one in its place.

type Message = Record<string, any>

const QUEUE = 'plan:queue'
const NEW_QUESTION = 'arch/proposals/03-harbor-index/questions/Q-03-05-which-rope.md'

const entry = (question: string, needs = 'human') => ({
  question,
  file: `proposals/made-up/questions/${question}.md`,
  needs,
  blocks: [],
})
const envelope = (data: unknown, over: Message = {}) =>
  JSON.stringify({ schema_version: 1, command: 'status', corpus: 'arch', data, ...over })
/** A plan CLI that prints `json` and records its arguments in the home of the repository. */
const printing = (json: string) =>
  `#!/bin/sh\necho "$*" >> "$HOME/plan-calls.log"\nprintf '%s\\n' '${json}'\n`
const script = (body: string) => `#!/bin/sh\n${body}\n`

/** The first list of a provider for the repository, with the log lines it wrote. */
async function listing(repo: Repo, options: { timeoutMs?: number } = {}): Promise<Message> {
  const client = await repo.provider(options)
  try {
    const reply = await client.request('list')
    return { ...(reply.result as Message), logs: reply.logs }
  } finally {
    await client.close()
  }
}

const queueOf = (list: Message): Message | undefined =>
  (list.collections as Message[]).find((collection) => collection.id === QUEUE)
const inQueue = (list: Message): string[] =>
  (list.tasks as Message[]).filter((task) => task.collections?.includes(QUEUE)).map((task) => task.key)
const callsOf = (repo: Repo): string[] => {
  const log = join(repo.env.HOME as string, 'plan-calls.log')
  return existsSync(log) ? readFileSync(log, 'utf8').split('\n').filter(Boolean) : []
}

/** The questions of the fake's own JSON, as keys, in its order. */
function fakeQueue(repo: Repo): string[] {
  const run = plan(repo, ['status', '--root', 'arch', '--format', 'json'])
  expect(run.code, run.stderr).toBe(0)
  const queue: Array<{ question: string }> = JSON.parse(run.stdout).data.queue
  return queue.map(({ question }) => `plan:${/^Q-\d{2}-\d{2}/.exec(question)?.[0]}`)
}

describe('the queue is the plan CLI\'s list', () => {
  test('the collection holds the questions of the CLI\'s JSON, in its order', async () => {
    const repo = makeRepo('kinds')
    const expected = fakeQueue(repo)
    expect(expected).toEqual(['plan:Q-03-01', 'plan:Q-03-03', 'plan:Q-05-01'])
    const queue = queueOf(await listing(repo))
    expect(queue).toMatchObject({ id: QUEUE })
    expect(queue?.groups).toHaveLength(1)
    expect(queue?.groups[0].tasks).toEqual(expected)
  })

  test('each question in it holds the collection, and no other item does', async () => {
    const repo = makeRepo('kinds')
    expect(inQueue(await listing(repo))).toEqual(fakeQueue(repo))
  })

  test('get says the same of a question as list does', async () => {
    const repo = makeRepo('kinds')
    const client = await repo.provider()
    try {
      const inside = await client.request('get', { id: 'Q-03-03' })
      const outside = await client.request('get', { id: 'Q-03-02' })
      expect(inside.result?.task.collections).toEqual([QUEUE])
      expect(outside.result?.task.collections).toEqual([])
    } finally {
      await client.close()
    }
  })

  test('a question that is added to the tree is in the queue the next time, in the CLI\'s order', async () => {
    const repo = makeRepo('kinds')
    repo.write(NEW_QUESTION, '---\ntitle: "Which rope"\nstatus: open\nneeds: human\n---\n')
    const queue = queueOf(await listing(repo))
    expect(queue?.groups[0].tasks).toEqual(['plan:Q-03-01', 'plan:Q-03-03', 'plan:Q-03-05', 'plan:Q-05-01'])
    expect(queue?.groups[0].tasks).toEqual(fakeQueue(repo))
  })

  test('a question that is closed leaves the queue', async () => {
    const repo = makeRepo('kinds')
    const path = 'arch/proposals/03-harbor-index/questions/Q-03-01-which-berth-first.md'
    repo.write(path, repo.read(path).replace('status: open', 'status: dropped'))
    expect(queueOf(await listing(repo))?.groups[0].tasks).toEqual(['plan:Q-03-03', 'plan:Q-05-01'])
  })

  test('the rule is the CLI\'s: what it prints is what is queued, in its order, whatever the files say', async () => {
    const queue = [entry('Q-05-01-which-lamp-first'), entry('Q-03-02-how-deep-is-the-quay', 'evidence')]
    const answer = envelope({ queue })
    const repo = makeRepo('kinds', printing(answer))
    const list = await listing(repo)
    expect(queueOf(list)?.groups[0].tasks).toEqual(['plan:Q-05-01', 'plan:Q-03-02'])
    expect(inQueue(list)).toEqual(['plan:Q-03-02', 'plan:Q-05-01'])
  })

  test('a question the CLI names that the tree lacks is left out, and so is a name that is no question', async () => {
    const answer = envelope({
      queue: [entry('Q-99-99-nowhere'), entry('Q-03-01-which-berth-first'), entry('T-03-001-survey-the-quay')],
    })
    const repo = makeRepo('kinds', printing(answer))
    const list = await listing(repo)
    expect(queueOf(list)?.groups[0].tasks).toEqual(['plan:Q-03-01'])
    expect(list.logs.join('\n')).toMatch(/Q-99-99-nowhere/)
    expect(list.logs.join('\n')).toMatch(/T-03-001-survey-the-quay/)
  })

  test('an empty queue is a collection with no question in it', async () => {
    const repo = makeRepo('kinds', printing(envelope({ queue: [] })))
    const list = await listing(repo)
    expect(queueOf(list)?.groups).toEqual([{ name: expect.any(String), tasks: [] }])
    expect(inQueue(list)).toEqual([])
  })

  test('the CLI is run once with the command first, the arch root and the format', async () => {
    const repo = makeRepo('kinds', printing(envelope({ queue: [] })))
    await listing(repo)
    expect(callsOf(repo)).toEqual([`status --root ${join(repo.root, 'arch')} --format json`])
  })
})

describe('when the plan CLI cannot say', () => {
  const CASES: Array<[string, string]> = [
    ['exits 1', 'echo "made-up trouble" >&2\nexit 1'],
    ['exits 2', 'echo "made-up usage" >&2\nexit 2'],
    ['prints what is not JSON', 'echo not json'],
    ['prints an envelope of another version', `echo '${envelope({ queue: [] }, { schema_version: 2 })}'`],
    ['prints the envelope of another command', `echo '${envelope({ queue: [] }, { command: 'set' })}'`],
    ['prints data with no queue', `echo '${envelope({})}'`],
    ['prints a queue that is not a list', `echo '${envelope({ queue: 'none' })}'`],
    ['prints a queue entry that is not an object', `echo '${envelope({ queue: ['Q-03-01'] })}'`],
    ['prints a queue entry with no question', `echo '${envelope({ queue: [{ needs: 'human' }] })}'`],
  ]
  const KEYS_OF_TREE = 16

  test.each(CASES)('it %s: no queue, every item lists, and a line says why', async (_name, body) => {
    const repo = makeRepo('kinds', script(body))
    const list = await listing(repo)
    expect(queueOf(list)).toBeUndefined()
    expect(inQueue(list)).toEqual([])
    expect(list.tasks).toHaveLength(KEYS_OF_TREE)
    expect(list.invalid).toEqual([])
    expect(list.logs.join('\n')).toMatch(/queue/)
  })

  test('it is not there: no queue, every item lists, and a line says why', async () => {
    const repo = makeRepo('kinds')
    rmSync(join(repo.root, 'plan'))
    const list = await listing(repo)
    expect(queueOf(list)).toBeUndefined()
    expect(list.tasks).toHaveLength(KEYS_OF_TREE)
    expect(list.logs.join('\n')).toMatch(/queue.*cannot run \.\/plan/)
  })

  test('it does not finish in time: it is killed, there is no queue, and the list answers', async () => {
    const repo = makeRepo('kinds', script('exec sleep 30'))
    const started = Date.now()
    const list = await listing(repo, { timeoutMs: 300 })
    expect(Date.now() - started).toBeLessThan(20_000)
    expect(queueOf(list)).toBeUndefined()
    expect(list.tasks).toHaveLength(KEYS_OF_TREE)
    expect(list.logs.join('\n')).toMatch(/did not finish/)
  })
})

describe('when it is not needed', () => {
  test('a tree with no question never runs the plan CLI', async () => {
    const repo = makeRepo('basic', printing(envelope({ queue: [] })))
    const list = await listing(repo)
    expect(callsOf(repo)).toEqual([])
    expect(queueOf(list)).toBeUndefined()
  })

  test('a tree whose questions are all closed never runs the plan CLI', async () => {
    const repo = makeRepo('kinds', printing(envelope({ queue: [] })))
    for (const path of [
      'arch/proposals/03-harbor-index/questions/Q-03-01-which-berth-first.md',
      'arch/proposals/03-harbor-index/questions/Q-03-02-how-deep-is-the-quay.md',
      'arch/proposals/03-harbor-index/questions/Q-03-03-who-rings-the-bell.md',
      'arch/proposals/05-lamp-works/questions/Q-05-01-which-lamp-first.md',
    ]) {
      repo.write(path, repo.read(path).replace('status: open', 'status: dropped'))
    }
    const list = await listing(repo)
    expect(callsOf(repo)).toEqual([])
    expect(queueOf(list)).toBeUndefined()
  })

  test('the epics of the basic tree are still its only collections', async () => {
    const repo = makeRepo('basic')
    const list = await listing(repo)
    expect((list.collections as Message[]).map((collection) => collection.id)).toEqual(['plan:E-07', 'plan:E-08'])
  })
})
