import { spawn, spawnSync } from 'node:child_process'
import { existsSync, readdirSync, readFileSync, utimesSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { withLock } from '../src/lock.ts'
import { tempDir } from './helpers.ts'

const worker = join(import.meta.dirname, 'lock-worker.ts')
const reader = join(import.meta.dirname, 'lock-reader.ts')
const churner = join(import.meta.dirname, 'lock-churn.ts')

/** A pid that belonged to a process that has exited. */
function deadPid(): number {
  const child = spawnSync(process.execPath, ['-e', '0'])
  return child.pid
}

const lockIn = (dir: string): string => join(dir, '.write-lock')

describe('the lock file', () => {
  test('is created holding the pid while the work runs, and removed after, also when the work throws', async () => {
    const lock = lockIn(tempDir())
    await withLock(lock, async () => {
      expect(readFileSync(lock, 'utf8')).toBe(`${process.pid}\n`)
    })
    expect(existsSync(lock)).toBe(false)
    await expect(withLock(lock, () => Promise.reject(new Error('boom')))).rejects.toThrow('boom')
    expect(existsSync(lock)).toBe(false)
  })

  test('a waiter that times out names the lock file and the pid that holds it, and leaves the lock alone', async () => {
    const lock = lockIn(tempDir())
    writeFileSync(lock, `${process.pid}\n`)
    await expect(withLock(lock, async () => {}, { timeoutMs: 150 })).rejects.toThrow(`timed out after 0.15 s waiting for the lock file ${lock}, held by pid ${process.pid}`)
    expect(readFileSync(lock, 'utf8')).toBe(`${process.pid}\n`)
  })

  test('two holders in one process take turns', async () => {
    const lock = lockIn(tempDir())
    const events: string[] = []
    const hold = (name: string) => withLock(lock, async () => {
      events.push(`enter ${name}`)
      await new Promise((resolve) => setTimeout(resolve, 30))
      events.push(`exit ${name}`)
    })
    await Promise.all([hold('a'), hold('b'), hold('c')])
    expect(events.map((event) => event.split(' ')[0])).toEqual(['enter', 'exit', 'enter', 'exit', 'enter', 'exit'])
  })

  test('a lock whose pid is dead is taken over, and the work runs', async () => {
    const dir = tempDir()
    const lock = lockIn(dir)
    const stale = deadPid()
    writeFileSync(lock, `${stale}\n`)
    const taken: Array<number | null> = []
    await withLock(lock, async () => {}, { onTakeover: (pid) => taken.push(pid) })
    expect(taken).toEqual([stale])
    expect(readdirSync(dir)).toEqual([])
  })

  test.each([['empty', ''], ['a pid without its newline', '123'], ['text', 'abc\n'], ['two lines', '123\n456\n']])(
    'a lock that is %s is never taken over, however old, and the timeout says it holds no pid',
    async (_name, content) => {
      const lock = lockIn(tempDir())
      writeFileSync(lock, content)
      const old = new Date(Date.now() - 3_600_000)
      utimesSync(lock, old, old)
      await expect(withLock(lock, async () => {}, { timeoutMs: 150 })).rejects.toThrow(`waiting for the lock file ${lock}, which holds no pid`)
      expect(readFileSync(lock, 'utf8')).toBe(content)
    },
  )

  test('temp files of dead writers beside the lock are removed by the next holder, and those of live ones are not', async () => {
    const dir = tempDir()
    const dead = deadPid()
    const names = [`.write-lock.${dead}.0a1b2c3d.tmp`, `.write-lock.takeover.${dead}.0a1b2c3d.tmp`]
    const live = `.write-lock.${process.pid}.0a1b2c3d.tmp`
    const other = `.write-lock.notes.${dead}.tmp`
    for (const name of [...names, live, other]) writeFileSync(join(dir, name), `${dead}\n`)
    await withLock(lockIn(dir), async () => {
      expect(readdirSync(dir).sort()).toEqual([live, other, '.write-lock'].sort())
    })
    expect(readdirSync(dir).sort()).toEqual([live, other].sort())
  })

  test('a claim file left by a dead waiter does not block a takeover', async () => {
    const dir = tempDir()
    const lock = lockIn(dir)
    writeFileSync(lock, `${deadPid()}\n`)
    writeFileSync(`${lock}.takeover`, `${deadPid()}\n`)
    await withLock(lock, async () => {})
    expect(readdirSync(dir)).toEqual([])
  })

  test('the lock file, read at any moment while writers contend for it, always holds a whole pid', async () => {
    const dir = tempDir()
    const lock = lockIn(dir)
    const endAt = Date.now() + 1_000
    const finished = (script: string, ...args: string[]) =>
      new Promise<string>((resolve, reject) => {
        const child = spawn(process.execPath, [script, ...args], { stdio: ['ignore', 'pipe', 'inherit'] })
        let out = ''
        child.stdout.setEncoding('utf8').on('data', (chunk: string) => (out += chunk))
        child.on('close', (code) => (code === 0 ? resolve(out) : reject(new Error(`${script} exited with ${code}`))))
      })
    const [seen] = await Promise.all([finished(reader, lock, String(endAt)), ...[1, 2, 3].map(() => finished(churner, lock, '12'))])
    const { held, bad } = JSON.parse(seen) as { held: number; bad: string[] }
    expect(held).toBeGreaterThan(0)
    expect(bad).toEqual([])
    expect(readdirSync(dir)).toEqual([])
  })

  test('of two waiters on one stale lock, exactly one takes it over, and they never hold the lock together', async () => {
    const rounds = 6
    const results = await Promise.all(Array.from({ length: rounds }, () => staleRound()))
    for (const { log, dir } of results) {
      expect(log.filter((line) => line.startsWith('takeover'))).toHaveLength(1)
      expect(log.map((line) => line.split(' ')[0])).toContain('enter')
      // enter and exit alternate: one holder at a time
      expect(log.filter((line) => !line.startsWith('takeover')).map((line) => line.split(' ')[0])).toEqual(['enter', 'exit', 'enter', 'exit'])
      expect(readdirSync(dir).filter((name) => name.startsWith('.write-lock'))).toEqual([])
    }
  })
})

/** One dead-pid lock, two waiter processes released at the same moment; what they logged. */
async function staleRound(): Promise<{ log: string[]; dir: string }> {
  const dir = tempDir()
  const lock = lockIn(dir)
  const logPath = join(dir, 'log.txt')
  writeFileSync(lock, `${deadPid()}\n`)
  writeFileSync(logPath, '')
  const startAt = Date.now() + 700
  const run = () =>
    new Promise<void>((resolve, reject) => {
      const child = spawn(process.execPath, [worker, lock, logPath, String(startAt), '100'], { stdio: ['ignore', 'ignore', 'inherit'] })
      child.on('close', (code) => (code === 0 ? resolve() : reject(new Error(`a waiter exited with ${code}`))))
    })
  await Promise.all([run(), run()])
  return { log: readFileSync(logPath, 'utf8').trim().split('\n'), dir }
}
