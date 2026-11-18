import { spawn } from 'node:child_process'
import { join } from 'node:path'

/** What `./plan` printed and how it ended. A process that ended by a signal has no `code`. */
export interface PlanRun {
  code: number | null
  signal: NodeJS.Signals | null
  stdout: string
  stderr: string
}

/** The most that the plan CLI may print on either stream. A write prints a few lines. */
const OUTPUT_CAP = 8 * 1024 * 1024

/**
 * Runs the checkout's own `./plan` with `args`, from `root` as its working directory, and waits for it to end. The
 * child is killed and this rejects when it runs past `timeoutMs` or prints more than the cap, and when it cannot be
 * started at all.
 */
export function runPlan(root: string, args: string[], env: NodeJS.ProcessEnv, timeoutMs: number): Promise<PlanRun> {
  return new Promise((resolve, reject) => {
    const child = spawn(join(root, 'plan'), args, {
      cwd: root,
      env,
      stdio: ['ignore', 'pipe', 'pipe'],
    })
    let stdout = ''
    let stderr = ''
    let failure: Error | null = null
    const stop = (error: Error): void => {
      if (failure === null) failure = error
      child.kill('SIGKILL')
    }
    const timer = setTimeout(() => stop(new Error(`./plan did not finish within ${timeoutMs} ms`)), timeoutMs)
    const collect = (add: (text: string) => string) => (chunk: string) => {
      if (add(chunk).length > OUTPUT_CAP) stop(new Error(`./plan printed more than ${OUTPUT_CAP} bytes`))
    }
    child.stdout.setEncoding('utf8').on(
      'data',
      collect((text) => (stdout += text)),
    )
    child.stderr.setEncoding('utf8').on(
      'data',
      collect((text) => (stderr += text)),
    )
    child.on('error', (error) => {
      clearTimeout(timer)
      reject(new Error(`cannot run ./plan in ${root}: ${error.message}`))
    })
    child.on('close', (code, signal) => {
      clearTimeout(timer)
      if (failure === null) resolve({ code, signal, stdout, stderr })
      else reject(failure)
    })
  })
}
