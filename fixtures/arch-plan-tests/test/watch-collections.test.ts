import { existsSync, readFileSync, rmSync } from 'node:fs'
import { join } from 'node:path'
import { afterEach, describe, expect, test } from 'vitest'
import { fakeClock, fakeFs } from './fakes.ts'
import { makeRepo, FAKE_PLAN } from './repo.ts'
import type { Repo } from './repo.ts'

// `collections.changed`: what the collections of a checkout, and what a task is in, changed to when the item itself did
// not change. An epic file that comes or goes, or is edited, changes the epics with no task file touched. The human
// queue follows the question files: the plan CLI is asked again only when the open questions are not the ones it last
// answered for. The tests use a fake clock and a fake file watcher, so none waits for time.

const QUEUE = 'plan:queue'
const QUESTIONS = 'arch/proposals/03-harbor-index/questions'
const WHO_RINGS = `${QUESTIONS}/Q-03-03-who-rings-the-bell.md`
const HOW_DEEP = `${QUESTIONS}/Q-03-02-how-deep-is-the-quay.md`
/** The `basic` tree has two epics: E-07 holds four tasks, and E-08 four. */
const EPIC_07 = 'arch/work/epics/E-07-harbor-plan.md'
const SURVEY = 'T-03-001'
const SENTINEL = '099'

type Client = Awaited<ReturnType<Repo['provider']>>
const open: Client[] = []
afterEach(async () => {
  await Promise.all(open.splice(0).map((client) => client.close()))
})

/** The fake plan CLI, which writes a line for each call it gets to `plan-calls.log` in the home. */
const COUNTING = `#!/bin/sh\necho "$*" >> "$HOME/plan-calls.log"\nexec "${FAKE_PLAN}" "$@"\n`

/** A provider told to watch the checkout, on a fake clock and a fake file watcher. */
async function watching(repo: Repo) {
  const clock = fakeClock()
  const fs = fakeFs()
  const client = await repo.provider({ watch: { timers: clock.timers, watchFs: fs.watchFs } })
  open.push(client)
  const reply = await client.request('watch', { checkouts: [repo.root] })
  expect(reply.error).toBeUndefined()
  const logged: string[] = []
  return {
    client,
    /** What the provider logged in the round trips of `until`. */
    logged,
    /** What the watcher would see: an event in `arch/`, and then the quiet spell ending. */
    changed(): void {
      fs.at(join(repo.root, 'arch')).emit('somewhere.md')
      clock.runWaits()
    },
    /**
     * Every notification up to the first of `last`, and any that came right behind it. The watcher sends one read's
     * notifications together, so after the last that a read must send, a round trip (of a method that asks nothing)
     * shows whether it sent more.
     */
    async until(last: string) {
      const seen: Array<{ method: string; params: Record<string, any> }> = []
      do {
        const [next] = await client.notifications(null)
        seen.push({ method: next?.method ?? '', params: next?.params ?? {} })
      } while (seen.at(-1)?.method !== last)
      logged.push(...(await client.request('no.such.method')).logs)
      seen.push(...client.pending().map(({ method, params }) => ({ method, params })))
      return seen
    },
    /** A new task, and the read that finds it: all that a read sends when nothing else changed. */
    async sentinel() {
      const sample = repo.taskPath(SURVEY)
      repo.write(sample.replace(`${SURVEY}-`, `T-03-${SENTINEL}-`), repo.read(sample))
      this.changed()
      const seen = await this.until('task.changed')
      expect(seen.map(({ method }) => method)).toEqual(['task.changed'])
      expect(seen[0]?.params.key).toBe(`plan:T-03-${SENTINEL}`)
    },
  }
}

const edit = (repo: Repo, path: string, from: string, to: string): void => {
  const text = repo.read(path)
  expect(text).toContain(from)
  repo.write(path, text.replace(from, to))
}

const callsOf = (repo: Repo): string[] => {
  const log = join(repo.env.HOME as string, 'plan-calls.log')
  return existsSync(log) ? readFileSync(log, 'utf8').split('\n').filter(Boolean) : []
}

const idsOf = (params: Record<string, any>): string[] => params.collections.map((one: { id: string }) => one.id)

describe('collections.changed: epics', () => {
  test('a task whose epic line changes is announced with its new place, and the epics as list gives them', async () => {
    const repo = makeRepo()
    const watch = await watching(repo)
    const path = repo.taskPath(SURVEY)
    repo.write(path, repo.read(path).replace('epic: E-07-harbor-plan', 'epic: E-08-signal-lamps'))
    watch.changed()

    const seen = await watch.until('collections.changed')

    expect(seen.map(({ method }) => method)).toEqual(['task.changed', 'collections.changed'])
    const listed = (await watch.client.request('list')).result?.collections
    expect(seen[1]?.params).toEqual({
      checkout: repo.root,
      collections: listed,
      members: [{ key: `plan:${SURVEY}`, collections: ['plan:E-08'] }],
    })
  })

  test('an epic file that goes takes the epic out of the collections and its tasks out of it', async () => {
    const repo = makeRepo()
    const watch = await watching(repo)
    rmSync(join(repo.root, EPIC_07))
    watch.changed()

    const seen = await watch.until('collections.changed')

    expect(seen.map(({ method }) => method)).toEqual(['collections.changed'])
    expect(idsOf(seen[0]?.params ?? {})).toEqual(['plan:E-08'])
    expect(seen[0]?.params.members).toEqual(
      ['T-03-001', 'T-03-002', 'T-03-003', 'T-03-005'].map((short) => ({ key: `plan:${short}`, collections: [] })),
    )
  })

  test('an epic file that changes its title is the epic as it now is, and no task is named', async () => {
    const repo = makeRepo()
    const watch = await watching(repo)
    edit(repo, EPIC_07, 'title: "Harbor plan"', 'title: "Harbor plan, renamed"')
    watch.changed()

    const seen = await watch.until('collections.changed')

    expect(seen.map(({ method }) => method)).toEqual(['collections.changed'])
    const epic = seen[0]?.params.collections.find((one: { id: string }) => one.id === 'plan:E-07')
    expect(epic.title).toBe('Harbor plan, renamed')
    expect(seen[0]?.params.members).toEqual([])
  })

  test('a change that touches no collection sends no collections.changed', async () => {
    const repo = makeRepo()
    const watch = await watching(repo)
    const path = repo.taskPath(SURVEY)
    repo.write(path, repo.read(path).replace(/^status: .*$/m, 'status: doing'))
    watch.changed()

    expect((await watch.until('task.changed')).map(({ method }) => method)).toEqual(['task.changed'])
    await watch.sentinel()
  })

  test('a new task is left to its own task.changed: it is not named in members', async () => {
    const repo = makeRepo()
    const watch = await watching(repo)

    await watch.sentinel()
  })
})

describe('collections.changed: the human queue', () => {
  test('a question that is resolved leaves the queue, after its own task.changed', async () => {
    const repo = makeRepo('kinds', COUNTING)
    const watch = await watching(repo)
    edit(repo, WHO_RINGS, 'status: open', 'status: resolved')
    watch.changed()

    const seen = await watch.until('collections.changed')

    expect(seen.map(({ method }) => method)).toEqual(['task.changed', 'collections.changed'])
    const queue = seen[1]?.params.collections.find((one: { id: string }) => one.id === QUEUE)
    expect(queue.groups[0].tasks).toEqual(['plan:Q-03-01', 'plan:Q-05-01'])
    expect(seen[1]?.params.members).toEqual([{ key: 'plan:Q-03-03', collections: [] }])
  })

  test('a question that is set to need a person joins it, in the order the plan CLI gives', async () => {
    const repo = makeRepo('kinds', COUNTING)
    const watch = await watching(repo)
    edit(repo, HOW_DEEP, 'needs: evidence', 'needs: human')
    watch.changed()

    const seen = await watch.until('collections.changed')

    expect(seen.map(({ method }) => method)).toEqual(['task.changed', 'collections.changed'])
    const queue = seen[1]?.params.collections.find((one: { id: string }) => one.id === QUEUE)
    expect(queue.groups[0].tasks).toEqual(['plan:Q-03-01', 'plan:Q-03-02', 'plan:Q-03-03', 'plan:Q-05-01'])
    expect(seen[1]?.params.members).toEqual([{ key: 'plan:Q-03-02', collections: [QUEUE] }])
  })

  test('the last question to be resolved takes the queue away, as list shows it', async () => {
    const repo = makeRepo('kinds', COUNTING)
    const watch = await watching(repo)
    for (const path of [
      `${QUESTIONS}/Q-03-01-which-berth-first.md`,
      WHO_RINGS,
      `${QUESTIONS}/Q-03-02-how-deep-is-the-quay.md`,
      'arch/proposals/05-lamp-works/questions/Q-05-01-which-lamp-first.md',
    ]) {
      edit(repo, path, 'status: open', 'status: resolved')
    }
    watch.changed()

    const seen = await watch.until('collections.changed')

    const last = seen.at(-1)
    expect(last?.method).toBe('collections.changed')
    expect(idsOf(last?.params ?? {})).toEqual([])
    expect(last?.params.members.map((one: { key: string }) => one.key)).toEqual([
      'plan:Q-03-01',
      'plan:Q-03-03',
      'plan:Q-05-01',
    ])
  })

  test('the plan CLI is asked at the start, and again only when the open questions have changed', async () => {
    const repo = makeRepo('kinds', COUNTING)
    const watch = await watching(repo)
    const asked = () => callsOf(repo).length
    expect(asked()).toBe(1)

    // A task changes and no question does: the queue is not asked again.
    const task = repo.taskPath(SURVEY)
    repo.write(task, repo.read(task).replace(/^status: .*$/m, 'status: doing'))
    watch.changed()
    await watch.until('task.changed')
    expect(asked()).toBe(1)

    edit(repo, WHO_RINGS, 'status: open', 'status: resolved')
    watch.changed()
    await watch.until('collections.changed')
    expect(asked()).toBe(2)

    // A read in which no question changed: the sentinel is its only news.
    await watch.sentinel()
    expect(asked()).toBe(2)
  })

  test('a plan CLI that cannot answer leaves the queue out of the notice, with the reason logged', async () => {
    const repo = makeRepo('kinds', COUNTING)
    const watch = await watching(repo)
    repo.write('plan', '#!/bin/sh\necho "made-up trouble" >&2\nexit 1\n')
    edit(repo, WHO_RINGS, 'status: open', 'status: resolved')
    watch.changed()

    const seen = await watch.until('collections.changed')

    expect(seen.map(({ method }) => method)).toEqual(['task.changed', 'collections.changed'])
    expect(idsOf(seen[1]?.params ?? {})).toEqual([])
    expect(watch.logged.join('\n')).toMatch(/the queue is not read/)
  })
})
