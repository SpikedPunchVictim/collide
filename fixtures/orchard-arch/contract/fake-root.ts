import { cpSync, mkdirSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'

const FIXTURES = join(dirname(fileURLToPath(import.meta.url)), '..')

/** The parameters of the cases on a root made by `makeFakeRoot`. */
export const FAKE_PARAMS = { task: 'T-03-003', proposal: 'P03', epic: 'E-07' }

/**
 * Makes the root that the fake plan CLI is recorded on, in `dir`: the `basic` tree, with the `kinds` tree's open
 * questions added so that `status` has a queue. `T-03-003` of that tree is `ready`.
 */
export function makeFakeRoot(dir: string): void {
  mkdirSync(dir, { recursive: true })
  cpSync(join(FIXTURES, 'basic'), dir, { recursive: true })
  for (const proposal of ['03-harbor-index', '05-lamp-works']) {
    const from = join(FIXTURES, 'kinds', 'arch', 'proposals', proposal, 'questions')
    cpSync(from, join(dir, 'arch', 'proposals', proposal, 'questions'), { recursive: true })
  }
}

// By hand: `node fixtures/orchard-arch/contract/fake-root.ts <new folder>`.
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const [dir] = process.argv.slice(2)
  if (dir === undefined) {
    process.stderr.write('usage: fake-root.ts <folder to make>\n')
    process.exitCode = 2
  } else makeFakeRoot(dir)
}
