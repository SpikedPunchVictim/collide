import { chmodSync, existsSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { afterEach, describe, expect, test } from 'vitest'
import { fakeClock, fakeFs } from './fakes.ts'
import { tree } from './helpers.ts'
import { makeRepo, FAKE_PLAN, versionOf } from './repo.ts'
import type { Repo } from './repo.ts'

// `get` for a checkout that `watch` holds answers from the watcher's last read of it, with no scan and no plan CLI run,
// as long as the task's own file is what that read saw. The tests count the runs of the plan CLI with a stand-in that
// logs each call, and use a fake clock and a fake file watcher, so none waits for time.

const QUESTIONS = 'arch/proposals/03-harbor-index/questions'
const HOW_DEEP = `${QUESTIONS}/Q-03-02-how-deep-is-the-quay.md`
const SURVEY = 'T-03-001'
const EPIC_07 = 'arch/work/epics/E-07-harbor-plan.md'

type Client = Awaited<ReturnType<Repo['provider']>>
const open: Client[] = []
afterEach(async () => {
  await Promise.all(open.splice(0).map((client) => client.close()))
})

/** The fake plan CLI, which writes a line for each call it gets to `plan-calls.log` in the home. */
const COUNTING = `#!/bin/sh\necho "$*" >> "$HOME/plan-calls.log"\nexec "${FAKE_PLAN}" "$@"\n`

/** A plan CLI whose `set` also removes the epic file of E-07, a file that is not the task's own. */
const REMOVES_EPIC = [
  '#!/bin/sh',
  `"${FAKE_PLAN}" "$@"`,
  'code=$?',
  `if [ "$1" = set ]; then rm -f ${EPIC_07}; fi`,
  'exit $code',
  '',
].join('\n')

/** A plan CLI whose first `status` fails and every later one works: the watcher's first read has no queue. */
const STATUS_FAILS_ONCE = [
  '#!/bin/sh',
  'echo "$*" >> "$HOME/plan-calls.log"',
  'if [ "$1" = status ] && [ ! -e "$HOME/status-failed" ]; then',
  '  touch "$HOME/status-failed"',
  '  echo "made-up trouble" >&2',
  '  exit 1',
  'fi',
  `exec "${FAKE_PLAN}" "$@"`,
  '',
].join('\n')

const callsOf = (repo: Repo): number => {
  const log = join(repo.env.HOME as string, 'plan-calls.log')
  return existsSync(log) ? readFileSync(log, 'utf8').split('\n').filter(Boolean).length : 0
}

/** A provider told to watch the checkout, on a fake clock and a fake file watcher. */
async function watching(repo: Repo) {
  const clock = fakeClock()
  const fs = fakeFs()
  const client = await repo.provider({ watch: { timers: clock.timers, watchFs: fs.watchFs } })
  open.push(client)
  expect((await client.request('watch', { checkouts: [repo.root] })).error).toBeUndefined()
  return {
    client,
    /** What the watcher would see: an event in `arch/`, and then the quiet spell ending. */
    changed(): void {
      fs.at(join(repo.root, 'arch')).emit('somewhere.md')
      clock.runWaits()
    },
    /** Waits for `count` task.changed notifications, then a round trip shows whether more came behind them. */
    async heard(count: number): Promise<void> {
      await client.notifications('task.changed', count)
      await client.request('no.such.method')
    },
  }
}

const get = async (client: Client, id: string, checkout?: string) =>
  (await client.request('get', { id, ...(checkout === undefined ? {} : { checkout }) })).result as Record<string, any>

const setLine = (repo: Repo, path: string, status: string): void =>
  repo.write(path, repo.read(path).replace(/^status: .*$/m, `status: ${status}`))

/** New task files that copy T-03-001, one for each of `numbers`. */
const addTasks = (repo: Repo, numbers: string[]): string[] => {
  const sample = repo.taskPath(SURVEY)
  return numbers.map((number) => {
    const path = sample.replace(`${SURVEY}-`, `T-03-${number}-`)
    repo.write(path, repo.read(sample))
    return `T-03-${number}`
  })
}

describe('get from the read of a watched checkout', () => {
  test('a burst of edits, then a get of each: the plan CLI runs once for the read and none for the gets', async () => {
    const repo = makeRepo('kinds', COUNTING)
    const watch = await watching(repo)
    expect(callsOf(repo)).toBe(1)
    const added = addTasks(repo, ['091', '092', '093'])
    repo.write(HOW_DEEP, repo.read(HOW_DEEP).replace('needs: evidence', 'needs: human'))
    watch.changed()
    await watch.heard(added.length)
    expect(callsOf(repo)).toBe(2)

    const answers = await Promise.all(added.map((id) => get(watch.client, id)))

    expect(callsOf(repo)).toBe(2)
    expect(answers.map((answer) => answer.task.key)).toEqual(added.map((id) => `plan:${id}`))
  })

  test('the answer is what a get of an unwatched provider gives: the same task, body and collections', async () => {
    const repo = makeRepo('kinds', COUNTING)
    const watch = await watching(repo)
    const [added] = addTasks(repo, ['091'])
    repo.write(HOW_DEEP, repo.read(HOW_DEEP).replace('needs: evidence', 'needs: human'))
    watch.changed()
    await watch.heard(1)
    const fresh = await repo.provider()
    open.push(fresh)

    for (const id of [added as string, SURVEY, 'Q-03-02', 'Q-03-03']) {
      expect(await get(watch.client, id)).toEqual(await get(fresh, id))
    }
  })

  test('a task that is edited and not yet read by the watcher is read from its file', async () => {
    const repo = makeRepo('kinds', COUNTING)
    const watch = await watching(repo)
    setLine(repo, repo.taskPath(SURVEY), 'doing')

    const answer = await get(watch.client, SURVEY)

    expect(answer.task.state.status).toBe('doing')
    expect(answer.task.version).toBe(versionOf(repo, repo.taskPath(SURVEY)))
  })

  test('a task whose file is gone and not yet missed by the watcher is no task', async () => {
    const repo = makeRepo('kinds', COUNTING)
    const watch = await watching(repo)
    rmSync(join(repo.root, repo.taskPath(SURVEY)))

    expect(await get(watch.client, SURVEY)).toEqual({})
  })

  test('a task whose file became a link, and is not yet missed by the watcher, is no task', async () => {
    const repo = makeRepo('kinds', COUNTING)
    const watch = await watching(repo)
    const path = join(repo.root, repo.taskPath(SURVEY))
    const copy = `${path}.copy`
    writeFileSync(copy, readFileSync(path))
    rmSync(path)
    symlinkSync(copy, path)

    expect(await get(watch.client, SURVEY)).toEqual({})
  })

  test('a checkout that the watcher could not read is read from the files', async () => {
    const repo = makeRepo('kinds', COUNTING)
    const watch = await watching(repo)
    const unreadable = join(repo.root, repo.taskPath(SURVEY))
    chmodSync(unreadable, 0o000)
    watch.changed()
    await watch.client.notifications('source.stale')

    try {
      const reply = await watch.client.request('get', { id: 'Q-03-03' })
      expect(reply.error?.message).toMatch(/EACCES/)
    } finally {
      chmodSync(unreadable, 0o644)
    }
  })

  test('a read whose queue could not be read does not answer: the get reads the queue again', async () => {
    const repo = makeRepo('kinds', STATUS_FAILS_ONCE)
    const watch = await watching(repo)
    expect(callsOf(repo)).toBe(1)

    const answer = await get(watch.client, 'Q-03-03')

    expect(answer.task.collections).toContain('plan:queue')
    expect(callsOf(repo)).toBe(2)
  })

  test('a read with no open question answers: a change of an epic file not yet read is not in it', async () => {
    const repo = makeRepo('basic', COUNTING)
    const watch = await watching(repo)
    rmSync(join(repo.root, EPIC_07))

    const answer = await get(watch.client, SURVEY)

    expect(answer.task.collections).toEqual(['plan:E-07'])
    expect(callsOf(repo)).toBe(0)
  })

  test('a task the read does not hold is read from the files', async () => {
    const repo = makeRepo('kinds', COUNTING)
    const watch = await watching(repo)
    const before = callsOf(repo)
    const [added] = addTasks(repo, ['094'])

    expect((await get(watch.client, added as string)).task.key).toBe(`plan:${added}`)
    expect(callsOf(repo)).toBe(before + 1)
  })

  test('a checkout that is not watched is read from the files each time', async () => {
    const repo = makeRepo('kinds', COUNTING)
    const other = repo.worktree('beside')
    const watch = await watching(repo)
    const before = callsOf(repo)
    setLine(other, other.taskPath(SURVEY), 'doing')

    const answer = await get(watch.client, SURVEY, other.root)

    expect(answer.task.state.status).toBe('doing')
    expect(callsOf(repo)).toBe(before + 1)
  })

  test('a get that names the watched checkout is answered from the read as one that names none', async () => {
    const repo = makeRepo('kinds', COUNTING)
    const watch = await watching(repo)
    const before = callsOf(repo)

    expect((await get(watch.client, 'Q-03-03', repo.root)).task.key).toBe('plan:Q-03-03')
    expect(callsOf(repo)).toBe(before)
  })

  test('of two watched checkouts, a get that names one is answered from that one read', async () => {
    const repo = makeRepo('basic', COUNTING)
    const other = repo.worktree('beside')
    // The task's own file is the same in both; only the other checkout lacks the epic it is in.
    rmSync(join(other.root, EPIC_07))
    const clock = fakeClock()
    const fs = fakeFs()
    const client = await repo.provider({ watch: { timers: clock.timers, watchFs: fs.watchFs } })
    open.push(client)
    expect((await client.request('watch', { checkouts: [repo.root, other.root] })).error).toBeUndefined()

    expect((await get(client, SURVEY, repo.root)).task.collections).toEqual(['plan:E-07'])
    expect((await get(client, SURVEY, other.root)).task.collections ?? []).not.toContain('plan:E-07')
    expect(callsOf(repo)).toBe(0)
  })
})

describe('get after a write of this provider', () => {
  test('is not answered from a read made before it: a file the write changed besides the task shows', async () => {
    const repo = makeRepo('basic', REMOVES_EPIC)
    const watch = await watching(repo)
    expect((await get(watch.client, SURVEY)).task.collections).toEqual(['plan:E-07'])
    const written = await watch.client.request('apply', {
      op: 'setStatus',
      id: 'T-03-003',
      version: versionOf(repo, repo.taskPath('T-03-003')),
      args: { status: 'doing' },
    })
    expect(written.result).toMatchObject({ outcome: 'ok' })

    expect((await get(watch.client, SURVEY)).task.collections).toEqual([])
  })

  test('is answered from the read again once the watcher has read the checkout after the write', async () => {
    const repo = makeRepo('kinds', COUNTING)
    // The kinds tree has no epic, which the fake plan CLI wants a task it writes to have.
    mkdirSync(join(repo.root, dirname(EPIC_07)), { recursive: true })
    repo.write(EPIC_07, readFileSync(join(tree('basic'), EPIC_07), 'utf8'))
    const survey = repo.taskPath(SURVEY)
    repo.write(survey, repo.read(survey).replace('status: ready', 'status: ready\nepic: E-07-harbor-plan'))
    const watch = await watching(repo)
    const written = await watch.client.request('apply', {
      op: 'setStatus',
      id: SURVEY,
      version: versionOf(repo, repo.taskPath(SURVEY)),
      args: { status: 'doing' },
    })
    expect(written.result).toMatchObject({ outcome: 'ok' })
    await get(watch.client, 'Q-03-03')
    const before = callsOf(repo)

    watch.changed()
    await watch.heard(1)
    await get(watch.client, 'Q-03-03')

    expect(callsOf(repo)).toBe(before)
  })

  test('a task that the write changed shows its new state', async () => {
    const repo = makeRepo('basic')
    const watch = await watching(repo)
    const written = await watch.client.request('apply', {
      op: 'setStatus',
      id: 'T-03-003',
      version: versionOf(repo, repo.taskPath('T-03-003')),
      args: { status: 'doing' },
    })
    expect(written.result).toMatchObject({ outcome: 'ok' })

    expect((await get(watch.client, 'T-03-003')).task.state.status).toBe('doing')
  })
})
