import { execFile } from 'node:child_process'
import { realpath } from 'node:fs/promises'
import { isAbsolute } from 'node:path'
import { promisify } from 'node:util'

const run = promisify(execFile)

/** The named checkout cannot be used. The message says why, and is what a refusal or an error reply carries. */
export class CheckoutError extends Error {}

/** The checkouts of the repository that `root` is in, as `git worktree list` gives them, with symbolic links resolved. A listed checkout that is gone from disk is left out. */
async function checkoutsOf(root: string): Promise<string[]> {
  let stdout: string
  try {
    ;({ stdout } = await run('git', ['-C', root, 'worktree', 'list', '--porcelain'], { encoding: 'utf8', env: { ...process.env, GIT_OPTIONAL_LOCKS: '0' } }))
  } catch (error) {
    const detail = (error as { stderr?: string }).stderr?.trim() || (error as Error).message
    throw new CheckoutError(`could not list the checkouts of ${root} with git (worktree list): ${detail}`)
  }
  const found: string[] = []
  for (const line of stdout.split('\n')) {
    if (!line.startsWith('worktree ')) continue
    try {
      found.push(await realpath(line.slice('worktree '.length)))
    } catch {
      continue
    }
  }
  return found
}

/**
 * The path to write in for `checkout`: it must be an absolute path that is an entry of `git worktree list` for the
 * repository at `root` (compared with symbolic links resolved), so a write never lands in a folder that is not a
 * checkout of this repository. Returns the resolved path; throws a `CheckoutError` otherwise.
 */
export async function resolveCheckout(root: string, checkout: string): Promise<string> {
  if (!isAbsolute(checkout)) throw new CheckoutError(`checkout must be an absolute path; got ${JSON.stringify(checkout)}`)
  let resolved: string
  try {
    resolved = await realpath(checkout)
  } catch (error) {
    throw new CheckoutError(`checkout ${JSON.stringify(checkout)} cannot be read: ${(error as Error).message}`)
  }
  const listed = await checkoutsOf(root)
  if (!listed.includes(resolved)) {
    throw new CheckoutError(`checkout ${JSON.stringify(checkout)} is not a checkout of this repository (git worktree list shows ${listed.join(', ')})`)
  }
  return resolved
}
