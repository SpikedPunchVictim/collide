// Stages the built-in packs for the app bundle (0.4, 0.6): `pnpm build:packs [--out <dir>]`.
//
// Each pack in `packs/` is copied to `<out>/<pack>/`, its `src/` is compiled to JavaScript in place (tsc, with
// `rewriteRelativeImportExtensions` turning the `.ts` imports into `.js`), and its npm dependencies are copied into
// a bundle-local `node_modules`. `@collide/pack-sdk` is its built `dist/` and `package.json`; any other dependency
// is the installed package, which must have no dependencies of its own. No `.ts` file is staged, declarations
// included, because the runtime never needs them and Node refuses to strip types under `node_modules`.
// The result has no write bits. A rebuild unlocks and replaces the previous output.
import { execFileSync } from 'node:child_process'
import {
  chmodSync,
  cpSync,
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  realpathSync,
  renameSync,
  rmSync,
  writeFileSync,
} from 'node:fs'
import { basename, dirname, join, resolve } from 'node:path'

const root = join(import.meta.dirname, '..')
const SDK = '@collide/pack-sdk'
const NOT_COPIED = new Set(['node_modules', 'package.json', 'tsconfig.json', 'tsconfig.build.json', 'src', 'test', 'dist'])

const outFlag = process.argv.indexOf('--out')
const out = resolve(outFlag > 0 ? (process.argv[outFlag + 1] ?? '') : join(root, 'app/src-tauri/resources/builtin-packs'))
if (outFlag > 0 && process.argv[outFlag + 1] === undefined) throw new Error('--out needs a directory')

/** Gives write permission back to a tree this script locked, so that it can be removed. */
function unlock(path: string) {
  const stat = lstatSync(path, { throwIfNoEntry: false })
  if (stat === undefined || stat.isSymbolicLink()) return
  chmodSync(path, stat.mode | 0o200)
  if (stat.isDirectory()) for (const name of readdirSync(path)) unlock(join(path, name))
}

function remove(path: string) {
  unlock(path)
  rmSync(path, { recursive: true, force: true })
}

/** Takes the write bits off every file and directory under `path`, children first. */
function lock(path: string) {
  const stat = lstatSync(path)
  if (stat.isDirectory()) for (const name of readdirSync(path)) lock(join(path, name))
  chmodSync(path, stat.mode & ~0o222)
}

const run = (cwd: string, command: string, args: string[]) => execFileSync(command, args, { cwd, stdio: 'inherit' })

// Skipped: sources and declarations, source maps, and the `node_modules/.bin` shims that pnpm leaves inside a package.
const skipped = (path: string) => path.endsWith('.ts') || path.endsWith('.map') || basename(path) === 'node_modules'
const copyTree = (from: string, to: string) =>
  cpSync(from, to, { recursive: true, dereference: true, filter: (path) => !skipped(path) })

/** Copies the installed package `name` of `from`'s `node_modules` into `<to>/node_modules/<name>`. */
function copyDependency(from: string, name: string, to: string) {
  const installed = realpathSync(join(from, 'node_modules', name))
  const target = join(to, 'node_modules', name)
  if (name === SDK) {
    mkdirSync(target, { recursive: true })
    cpSync(join(installed, 'package.json'), join(target, 'package.json'))
    copyTree(join(installed, 'dist'), join(target, 'dist'))
    // The SDK's own dependencies (`yaml`) go in its own `node_modules`, where `@collide/pack-sdk/yaml` finds them from
    // the SDK's real path, which is how a materialized pack reaches it through the SDK link (0.6).
    copyOwnDependencies(installed, target)
    return
  }
  const own = JSON.parse(readFileSync(join(installed, 'package.json'), 'utf8')).dependencies ?? {}
  if (Object.keys(own).length > 0) {
    const names = Object.keys(own).join(', ')
    throw new Error(`${name} has dependencies of its own (${names}); the bundle copies none of them`)
  }
  copyTree(installed, target)
}

/** Copies each dependency named in `from`'s `package.json` into `<to>/node_modules`. */
function copyOwnDependencies(from: string, to: string) {
  const dependencies = JSON.parse(readFileSync(join(from, 'package.json'), 'utf8')).dependencies ?? {}
  for (const name of Object.keys(dependencies)) copyDependency(from, name, to)
}

function stagePack(name: string, to: string) {
  const packDir = join(root, 'packs', name)
  mkdirSync(to)
  for (const entry of readdirSync(packDir).filter((entry) => !NOT_COPIED.has(entry))) {
    copyTree(join(packDir, entry), join(to, entry))
  }
  if (!existsSync(join(packDir, 'package.json'))) return
  run(packDir, 'pnpm', ['exec', 'tsc', '-p', 'tsconfig.build.json', '--outDir', join(to, 'src')])
  writeFileSync(join(to, 'package.json'), '{"private": true, "type": "module"}\n')
  copyOwnDependencies(packDir, to)
}

run(root, 'pnpm', ['--filter', SDK, 'build'])

const staging = `${out}.staging`
remove(staging)
mkdirSync(dirname(out), { recursive: true })
mkdirSync(staging)
for (const pack of readdirSync(join(root, 'packs')).sort()) stagePack(pack, join(staging, pack))
remove(out)
renameSync(staging, out) // before locking: macOS refuses to rename a directory without write permission on it
lock(out)
console.log(`built-in packs staged in ${out}`)
