import { existsSync } from 'node:fs'
import { rename, rm, writeFile } from 'node:fs/promises'
import { basename, dirname, join } from 'node:path'
import { createHookContext } from './context.ts'
import type { HookContext } from './context.ts'
import type { HookInputOutput, HookResultInput } from './generated/index.ts'

/**
 * A hook: reads the input, and ends with a result. It returns a result (`pending` and `unverified` need this, as
 * `ctx` has no method for them), or `void` after `ctx.ok`, `ctx.fail` or `ctx.refuse` has written one.
 */
export type HookHandler = (input: HookInputOutput, ctx: HookContext) => Promise<HookResultInput | void>

type HookStatus = HookResultInput['status']

const STATUSES: readonly HookStatus[] = ['ok', 'fail', 'unverified', 'pending', 'refused', 'error']

const isObject = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

const errorText = (error: unknown): string => (error instanceof Error ? error.message : String(error))

/**
 * Runs `handler` as the hook: reads stdin, builds the context, writes the result file (0.5, 0.6). It resolves when
 * the hook is done. A hook that cannot finish writes its reason to stderr and sets a non-zero exit code, with no
 * result file: a missing file is `error` to the engine (0.5).
 */
export async function defineHook(handler: HookHandler): Promise<void> {
  try {
    const input = parseInput(await readStdin())
    const resultPath = process.env.COLLIDE_RESULT
    if (resultPath === undefined || resultPath === '') {
      throw new Error('the COLLIDE_RESULT environment variable is not set, so there is nowhere to write the result')
    }
    const result = await handler(input, createHookContext(input))
    if (result === undefined) {
      // `ctx.ok`, `ctx.fail` and `ctx.refuse` write the file themselves and the handler may return nothing after them.
      if (!existsSync(resultPath)) throw new Error('the hook returned no result and wrote no result file')
      return
    }
    await writeResult(resultPath, checkResult(result))
  } catch (error) {
    process.stderr.write(`${errorText(error)}\n`)
    process.exitCode = 1
  }
}

/**
 * All of stdin, with no cap: the input is Collide's own, and the task text in it was capped when the corpus was built
 * (0.9).
 */
async function readStdin(): Promise<string> {
  const chunks: Buffer[] = []
  for await (const chunk of process.stdin) chunks.push(chunk as Buffer)
  return Buffer.concat(chunks).toString('utf8')
}

/** The input is one JSON object. It is not checked against the generated schema: a hook that needs more checks it. */
function parseInput(text: string): HookInputOutput {
  let value: unknown
  try {
    value = JSON.parse(text)
  } catch (error) {
    throw new Error(`the hook input is not JSON: ${errorText(error)}`)
  }
  if (!isObject(value)) throw new Error('the hook input is not one JSON object')
  return value as unknown as HookInputOutput
}

function checkResult(result: unknown): HookResultInput {
  if (!isObject(result)) throw new Error('the hook result is not an object')
  if (!STATUSES.includes(result.status as HookStatus)) {
    throw new Error(`the hook result has status ${JSON.stringify(result.status)}, not one of ${STATUSES.join(', ')}`)
  }
  return result as unknown as HookResultInput
}

/** Writes to a temp file in the result's folder and renames it into place, so a reader sees the whole file or none. */
async function writeResult(path: string, result: HookResultInput): Promise<void> {
  const temp = join(dirname(path), `.${basename(path)}.${process.pid}.tmp`)
  try {
    await writeFile(temp, `${JSON.stringify(result)}\n`, { flag: 'wx' })
    await rename(temp, path)
  } catch (error) {
    await rm(temp, { force: true })
    throw error
  }
}
