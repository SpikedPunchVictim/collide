import { execFile } from 'node:child_process'

/** A git call may run this long. The calls here read a few refs and one tree diff. */
const GIT_TIMEOUT_MS = 20_000

/** A path that git says moved between two commits, from the root of the repository. */
export interface GitRename {
  from: string
  to: string
}

/** Runs git in `cwd` and returns what it printed, or null when it fails, times out or is not installed. */
function git(cwd: string, args: string[], env: NodeJS.ProcessEnv): Promise<string | null> {
  return new Promise((resolve) => {
    execFile(
      'git',
      args,
      {
        cwd,
        env: { ...env, LC_ALL: 'C', GIT_TERMINAL_PROMPT: '0' },
        timeout: GIT_TIMEOUT_MS,
        maxBuffer: 16 * 1024 * 1024,
        encoding: 'utf8',
      },
      (error, stdout) => resolve(error === null ? stdout : null),
    )
  })
}

/** The commit that the checkout's HEAD names, or null when git cannot say (not a repository, no commit yet). */
export async function gitHead(cwd: string, env: NodeJS.ProcessEnv): Promise<string | null> {
  const out = await git(cwd, ['rev-parse', '--verify', '--quiet', 'HEAD'], env)
  const head = out?.trim() ?? ''
  return head === '' ? null : head
}

/** The folder that holds this checkout's own HEAD file: `.git` for a main checkout, one of its own for a worktree. */
export async function gitDir(cwd: string, env: NodeJS.ProcessEnv): Promise<string | null> {
  const out = await git(cwd, ['rev-parse', '--absolute-git-dir'], env)
  const dir = out?.trim() ?? ''
  return dir === '' ? null : dir
}

/**
 * The files under `arch/` that git sees renamed between two commits. Git decides, by its own similarity rule; nothing
 * here guesses. Empty when git cannot say.
 */
export async function gitRenames(cwd: string, from: string, to: string, env: NodeJS.ProcessEnv): Promise<GitRename[]> {
  const out = await git(cwd, ['diff', '--name-status', '-z', '-M', '--diff-filter=R', from, to, '--', 'arch'], env)
  if (out === null) return []
  // `-z` output: a status, then the old path and the new path, each ended by NUL. The filter leaves renames only.
  const parts = out.split('\0')
  const renames: GitRename[] = []
  for (let at = 0; parts[at]?.startsWith('R') === true; at += 3) {
    const [oldPath, newPath] = [parts[at + 1], parts[at + 2]]
    if (oldPath !== undefined && newPath !== undefined) renames.push({ from: oldPath, to: newPath })
  }
  return renames
}
