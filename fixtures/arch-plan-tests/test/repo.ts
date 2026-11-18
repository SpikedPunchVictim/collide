import { execFileSync, spawnSync } from 'node:child_process'
import { createHash } from 'node:crypto'
import {
  chmodSync,
  cpSync,
  copyFileSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { tmpdir } from 'node:os'
import { dirname, join } from 'node:path'
import { afterAll } from 'vitest'
import { REPO, start, tree } from './helpers.ts'
import type { ProviderOptions } from './helpers.ts'

/** The fake plan CLI: one executable, which a test repository holds at its root as `plan`. */
export const FAKE_PLAN = join(REPO, 'fixtures', 'orchard-arch', 'plan')

const temps: string[] = []
afterAll(() => temps.forEach((dir) => rmSync(dir, { recursive: true, force: true })))

const sha256 = (content: Buffer | string): string => createHash('sha256').update(content).digest('hex')

export interface Repo {
  /** The checkout's root: it holds `arch/` and `plan`. */
  root: string
  /** The environment for git and for the plan CLI: a temp home, no global or system git configuration. */
  env: NodeJS.ProcessEnv
  git(...args: string[]): string
  head(): string
  read(path: string): string
  write(path: string, text: string): void
  /** Every file under `arch/`, by its path from the root, with the sha256 of its content. */
  snapshot(): Record<string, string>
  /** The path from the root of the task file with this short id. */
  taskPath(shortId: string): string
  /** A provider that `initialize` has given this repository as its root, running the plan CLI with `env`. */
  provider(options?: ProviderOptions): ReturnType<typeof start>
  /** A second checkout of the repository, made by `git worktree add` on a new branch, beside this one. */
  worktree(name: string): Repo
}

/**
 * A temp git repository with one commit: a copy of a fixture tree and the fake plan CLI at its root (or the script
 * text given as `script`). Git reads no configuration of the user's or the system's, and the home is a temp folder.
 */
export function makeRepo(treeName = 'basic', script: string | null = null): Repo {
  const base = mkdtempSync(join(tmpdir(), 'arch-plan-repo-'))
  temps.push(base)
  const home = join(base, 'home')
  const root = join(base, 'repo')
  const emptyConfig = join(base, 'empty.gitconfig')
  mkdirSync(home)
  writeFileSync(emptyConfig, '')
  cpSync(tree(treeName), root, { recursive: true })
  if (script === null) copyFileSync(FAKE_PLAN, join(root, 'plan'))
  else writeFileSync(join(root, 'plan'), script)
  chmodSync(join(root, 'plan'), 0o755)

  const env: NodeJS.ProcessEnv = {
    PATH: process.env.PATH ?? '',
    HOME: home,
    GIT_CONFIG_GLOBAL: emptyConfig,
    GIT_CONFIG_SYSTEM: emptyConfig,
  }
  const first = repoAt(root, env)
  first.git('init', '--quiet', '--initial-branch=main')
  first.git('add', '-A')
  first.git('commit', '--quiet', '-m', 'fixture')
  return first
}

/** The checkout at `root`: git reads no configuration of the user's or the system's, and the home is a temp folder. */
function repoAt(root: string, env: NodeJS.ProcessEnv): Repo {
  const git = (...args: string[]): string =>
    execFileSync(
      'git',
      ['-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', '-c', 'commit.gpgsign=false', ...args],
      { cwd: root, env, encoding: 'utf8' },
    )

  const snapshot = (): Record<string, string> => {
    const files: Record<string, string> = {}
    for (const entry of readdirSync(join(root, 'arch'), {
      recursive: true,
      withFileTypes: true,
    })) {
      if (!entry.isFile()) continue
      const path = join(entry.parentPath, entry.name).slice(root.length + 1)
      files[path] = sha256(readFileSync(join(root, path)))
    }
    return files
  }

  return {
    root,
    env,
    git,
    head: () => git('rev-parse', 'HEAD').trim(),
    read: (path) => readFileSync(join(root, path), 'utf8'),
    write: (path, text) => writeFileSync(join(root, path), text),
    snapshot,
    taskPath: (shortId) => {
      const folders = readdirSync(join(root, 'arch', 'proposals'))
      for (const folder of folders) {
        const dir = join('arch', 'proposals', folder, 'tasks')
        const name = readdirSync(join(root, dir)).find((file) => file.startsWith(`${shortId}-`))
        if (name !== undefined) return join(dir, name)
      }
      throw new Error(`no task file for ${shortId}`)
    },
    provider: (options = {}) => start(root, { env, ...options }),
    worktree: (name) => {
      const path = join(dirname(root), name)
      git('worktree', 'add', '--quiet', '-b', name, path)
      return repoAt(path, env)
    },
  }
}

/** The version a task has: the sha256 of its file's content. */
export const versionOf = (repo: Repo, path: string): string => sha256(readFileSync(join(repo.root, path)))

export interface Run {
  code: number | null
  stdout: string
  stderr: string
}

/** Runs the repository's own `plan` with the repository's environment. */
export function plan(repo: Repo, args: string[], env: NodeJS.ProcessEnv = {}): Run {
  const done = spawnSync(join(repo.root, 'plan'), args, {
    cwd: repo.root,
    env: { ...repo.env, ...env },
    encoding: 'utf8',
  })
  return { code: done.status, stdout: done.stdout, stderr: done.stderr }
}
