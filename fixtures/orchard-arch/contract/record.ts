import { mkdirSync, mkdtempSync, rmSync, statSync, writeFileSync } from 'node:fs'
import { join, resolve } from 'node:path'
import { parseArgs } from 'node:util'
import { EPIC_ARG, PROPOSAL_ARG, SHORT_ID_PATTERN } from '../../packs/orchard/providers/plan/ids.ts'
import { CASES, CliFailure, shapeOfCase, taskStatus } from './cases.ts'
import type { Params } from './cases.ts'

// Records the shape of a plan CLI's envelopes, one file per case (cases.ts), into a folder.
//
//   node record.ts --cli <plan executable> --root <checkout root> --out <folder> --source fake|real
//                  --task <T-nn-nnn, a ready task> --proposal <Pnn> --epic <E-nn>
//
// It runs only the cases, with the CLI and the root it is given, and the writes change that root. It writes only
// under `--out`: one `<case>.json` per case, and a home folder for the CLI that it removes at the end. It prints the
// path of each file it wrote and nothing from the CLI. A refusal names the argument, never a value from the run.

const OPTIONS = {
  cli: { type: 'string' },
  root: { type: 'string' },
  out: { type: 'string' },
  source: { type: 'string' },
  task: { type: 'string' },
  proposal: { type: 'string' },
  epic: { type: 'string' },
} as const

/** What a shape file holds besides the shape: which case, how it was recorded and when. */
export interface ShapeFile {
  case: string
  args: string[]
  source: string
  recorded: string
  shape: ReturnType<typeof shapeOfCase>
}

class Refusal extends Error {}

function argumentsOf(argv: string[]) {
  const { values } = parseArgs({ args: argv, options: OPTIONS, strict: true })
  const need = (name: keyof typeof OPTIONS): string => {
    const value = values[name]
    if (value === undefined || value === '') throw new Refusal(`the argument --${name} is needed`)
    return value
  }
  const params: Params = { task: need('task'), proposal: need('proposal'), epic: need('epic') }
  if (!new RegExp(SHORT_ID_PATTERN).test(params.task)) throw new Refusal('--task is not a short task id')
  if (!new RegExp(PROPOSAL_ARG).test(params.proposal)) throw new Refusal('--proposal is not a proposal id like P03')
  if (!new RegExp(EPIC_ARG).test(params.epic)) throw new Refusal('--epic is not an epic id like E-07')
  const source = need('source')
  if (!/^[a-z][a-z0-9-]*$/.test(source)) throw new Refusal('--source is a lowercase word, such as fake or real')
  return { cli: resolve(need('cli')), root: resolve(need('root')), out: resolve(need('out')), source, params }
}

function record(argv: string[]): void {
  const { cli, root, out, source, params } = argumentsOf(argv)
  if (!statSync(join(root, 'arch'), { throwIfNoEntry: false })?.isDirectory()) {
    throw new Refusal('--root has no arch folder')
  }
  if (taskStatus(root, params.task) !== 'ready') throw new Refusal('--task is not a task of the root in status ready')
  mkdirSync(out, { recursive: true })
  const home = mkdtempSync(join(out, '.home-'))
  try {
    const gitConfig = join(home, 'gitconfig')
    writeFileSync(gitConfig, '')
    const env = {
      PATH: process.env.PATH ?? '',
      HOME: home,
      GIT_CONFIG_GLOBAL: gitConfig,
      GIT_CONFIG_SYSTEM: gitConfig,
    }
    const recorded = new Date().toISOString().slice(0, 10)
    for (const one of CASES) {
      const file: ShapeFile = {
        case: one.name,
        args: [...one.args, ...(one.dryRun === true ? ['--dry-run'] : [])],
        source,
        recorded,
        shape: shapeOfCase(cli, root, one, params, env),
      }
      const path = join(out, `${one.name}.json`)
      writeFileSync(path, `${JSON.stringify(file, null, 2)}\n`)
      process.stdout.write(`${path}\n`)
    }
  } finally {
    rmSync(home, { recursive: true, force: true })
  }
}

try {
  record(process.argv.slice(2))
} catch (error) {
  // Only this script's own words: a value from the CLI's run is never in an error message.
  const badOption = (error as NodeJS.ErrnoException).code === 'ERR_PARSE_ARGS_UNKNOWN_OPTION'
  const own = error instanceof Refusal || error instanceof CliFailure || badOption
  process.stderr.write(`record: ${own ? (error as Error).message : 'it failed before it could record'}\n`)
  process.exitCode = 2
}
