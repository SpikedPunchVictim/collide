// Deterministic fake agent streams: a run is a script of timed items in run-local seconds.
import { between, chance, mulberry32, pick, type Rng } from './rng'
import type { DiffFile, Finding, ItemSpec, ScriptItem, ScriptSpan } from './types'

export interface PhaseSpec {
  key: string
  label: string
  dur: number
  kind: 'provision' | 'agent' | 'close'
  reviewer?: 'A' | 'B'
}

export const FULL_WORKFLOW: PhaseSpec[] = [
  { key: 'provision', label: 'provision', dur: 180, kind: 'provision' },
  { key: 'build', label: 'build', dur: 720, kind: 'agent' },
  { key: 'brief', label: 'brief', dur: 90, kind: 'agent' },
  { key: 'review:A', label: 'review', dur: 200, kind: 'agent', reviewer: 'A' },
  { key: 'review:B', label: 'review', dur: 230, kind: 'agent', reviewer: 'B' },
  { key: 'adjudicate', label: 'adjudicate', dur: 70, kind: 'agent' },
  { key: 'fix', label: 'fix', dur: 300, kind: 'agent' },
  { key: 'close', label: 'close', dur: 40, kind: 'close' },
]

export const SHORT_WORKFLOW: PhaseSpec[] = [
  { key: 'provision', label: 'provision', dur: 60, kind: 'provision' },
  { key: 'build', label: 'build', dur: 300, kind: 'agent' },
  { key: 'review:A', label: 'review', dur: 100, kind: 'agent', reviewer: 'A' },
  { key: 'review:B', label: 'review', dur: 120, kind: 'agent', reviewer: 'B' },
  { key: 'close', label: 'close', dur: 30, kind: 'close' },
]

export interface RaiseSpec {
  phase: string
  frac: number
  spec: ItemSpec
}

export interface WaitSpec {
  phase: string
  frac: number
  dur: number
  label: string
}

export interface GenOpts {
  seed: number
  phases?: PhaseSpec[]
  /** multiply every phase duration */
  scale?: number
  raises?: RaiseSpec[]
  waits?: WaitSpec[]
  /** verdict of the build gate */
  buildGate?: 'pass' | 'unverified'
  /** what the Queue shows when the build gate is unverified */
  gateSpec?: ItemSpec
  markers?: { phase: string; frac: number; text: string }[]
}

const FILES = [
  'src/cache/entry.rs', 'src/cache/mod.rs', 'src/importer/csv.rs', 'src/importer/mapper.rs', 'src/queue/lanes.rs',
  'src/queue/retry.rs', 'src/export/columns.rs', 'src/config/env.rs', 'src/cli/init.rs', 'tests/queue_retry.rs',
  'tests/importer_csv.rs', 'tests/export_columns.rs', 'docs/notes/importers.md',
]
const SAY = [
  'Reading the module first, then the nearest tests.',
  'The change touches one function and two call sites.',
  'Tests pass locally. Moving on to the edge cases.',
  'Found the cause: the value is read before the default applies.',
  'I will keep the public signature and change the body only.',
  'Running the narrow test first, then the whole crate.',
]

interface Call {
  tool: string
  exe?: string
  input: string
  dur: number
  exit?: number
  error?: boolean
}

function callFor(r: Rng): Call {
  const x = r()
  if (x < 0.3) return { tool: 'Read', input: pick(r, FILES), dur: between(r, 0.05, 0.4) }
  if (x < 0.45) return { tool: 'Grep', input: `"${pick(r, ['retry', 'ttl', 'lane', 'mapper', 'env'])}" src/`, dur: between(r, 0.1, 0.9) }
  if (x < 0.62) return { tool: 'Edit', input: pick(r, FILES), dur: between(r, 0.05, 0.3) }
  if (x < 0.68) return { tool: 'Write', input: pick(r, FILES), dur: between(r, 0.05, 0.3) }
  if (x < 0.78) {
    const c = pick(r, ['git status', 'git diff --stat', 'git log -n 5'])
    return { tool: 'Bash', exe: 'git', input: c, dur: between(r, 0.1, 0.8) }
  }
  if (x < 0.9) {
    const c = pick(r, [
      ['cargo', 'cargo test -p queue'], ['cargo', 'cargo build'], ['pnpm', 'pnpm test --run'], ['pnpm', 'pnpm lint'],
      ['cargo', 'cargo clippy'],
    ])
    const fail = chance(r, 0.12)
    return { tool: 'Bash', exe: c[0], input: c[1], dur: between(r, 1.2, 22), exit: fail ? 101 : 0, error: fail }
  }
  const server = pick(r, ['docs', 'tracker'])
  return { tool: `mcp:${server}`, input: `${server}.${pick(r, ['search', 'get_page', 'list'])}`, dur: between(r, 0.4, 3.2) }
}

let sidCounter = 0

function fillAgent(r: Rng, p: PhaseSpec, start: number, items: ScriptItem[], waits: WaitSpec[]): number {
  let cur = start
  let end = start + p.dur
  const myWaits = waits.filter((w) => w.phase === p.key).map((w) => ({ ...w, at: start + w.frac * p.dur, done: false }))
  while (cur < end - 4) {
    for (const w of myWaits) {
      if (!w.done && cur >= w.at) {
        w.done = true
        items.push({ at: cur, t: 'span', span: { kind: 'wait', tool: 'permission wait', input: w.label, dur: w.dur, phase: p.key } })
        cur += w.dur
        end += w.dur
      }
    }
    cur += between(r, 2, 18)
    if (chance(r, 0.12)) items.push({ at: cur, t: 'span', span: { kind: 'text', tool: 'text', input: pick(r, SAY), dur: 0, phase: p.key } })
    if (chance(r, 0.1)) {
      const sid = `sa${++sidCounter}`
      const total = between(r, 18, 60)
      items.push({ at: cur, t: 'span', span: { kind: 'subagent', tool: 'Task', input: 'explore callers of the changed function', dur: total, phase: p.key, sid } })
      let o = 1
      while (o < total - 2) {
        const c = callFor(r)
        const d = Math.min(c.dur, total - o - 0.5)
        items.push({ at: cur + o, t: 'span', span: { kind: 'tool', tool: c.tool, exe: c.exe, input: c.input, dur: d, phase: p.key, parent: sid } })
        o += d + between(r, 1, 5)
      }
      cur += total
      continue
    }
    if (chance(r, 0.25)) {
      items.push({ at: cur, t: 'span', span: { kind: 'hook', tool: 'hook', input: 'PreToolUse guard/lint', dur: between(r, 0.05, 0.4), phase: p.key } })
    }
    const c = callFor(r)
    items.push({ at: cur, t: 'span', span: { kind: 'tool', tool: c.tool, exe: c.exe, input: c.input, dur: c.dur, phase: p.key, exit: c.exit, error: c.error } })
    cur += c.dur
  }
  return end
}

export function genScript(o: GenOpts): ScriptItem[] {
  const r = mulberry32(o.seed)
  const scale = o.scale ?? 1
  const phases = (o.phases ?? FULL_WORKFLOW).map((p) => ({ ...p, dur: p.dur * scale }))
  const items: ScriptItem[] = []
  let t = 0
  let i = 0
  while (i < phases.length) {
    const group = phases[i].reviewer ? phases.filter((p) => p.reviewer) : [phases[i]]
    let groupEnd = t
    for (const p of group) {
      items.push({ at: t, t: 'phase', key: p.key, label: p.label, reviewer: p.reviewer })
      let end: number
      if (p.kind === 'provision') {
        const wt = p.dur * 0.45
        items.push({ at: t, t: 'span', span: { kind: 'provision', tool: 'provision', input: 'git worktree add', dur: wt, phase: p.key } })
        items.push({ at: t + wt, t: 'span', span: { kind: 'provision', tool: 'provision', input: 'map build', dur: p.dur - wt, phase: p.key } })
        end = t + p.dur
      } else if (p.kind === 'close') {
        items.push({ at: t, t: 'span', span: { kind: 'tool', tool: 'Bash', exe: 'git', input: 'git commit', dur: 1.2, phase: p.key } })
        end = t + p.dur
      } else {
        end = fillAgent(r, p, t, items, o.waits ?? [])
      }
      for (const rs of o.raises ?? []) {
        if (rs.phase === p.key) items.push({ at: t + rs.frac * p.dur, t: 'raise', spec: rs.spec, phase: p.key })
      }
      for (const m of o.markers ?? []) {
        if (m.phase === p.key) items.push({ at: t + m.frac * p.dur, t: 'marker', text: m.text })
      }
      if (p.key === 'build') {
        const v = o.buildGate ?? 'pass'
        items.push({ at: end, t: 'gate', phase: p.key, name: 'typecheck', verdict: v, note: v === 'pass' ? 'exit 0' : 'exit 127, tsc not found in worktree', spec: v === 'pass' ? undefined : o.gateSpec })
        items.push({ at: end, t: 'gate', phase: p.key, name: 'tests', verdict: 'pass', note: '38 passed' })
      }
      items.push({ at: end, t: 'phaseEnd', key: p.key })
      groupEnd = Math.max(groupEnd, end)
    }
    t = groupEnd
    i += group.length
  }
  items.push({ at: t, t: 'end' })
  // stable order by time; same-time items keep their insertion order
  return items.map((it, k) => ({ it, k })).sort((a, b) => a.it.at - b.it.at || a.k - b.k).map((x) => x.it)
}

export function genDiff(seed: number): DiffFile[] {
  const r = mulberry32(seed)
  const paths = [pick(r, FILES.filter((f) => f.startsWith('src/'))), pick(r, FILES.filter((f) => f.startsWith('tests/'))), 'docs/notes/importers.md']
  const files: DiffFile[] = paths.map((p, k) => ({
    path: p,
    add: 12 + Math.floor(r() * 60),
    del: Math.floor(r() * 20),
    lines: [`@@ -${10 + k * 7},6 +${10 + k * 7},9 @@`, ' fn existing() {', '-    let value = read();', '+    let value = read().unwrap_or(default());', '+    log_value(&value);', ' }'],
  }))
  files.push({ path: 'Cargo.lock', add: 4, del: 1, dep: true, lines: ['@@ -812,3 +812,6 @@', '+name = "tiny-backoff"', '+version = "0.4.2"'] })
  return files
}

export function genFindings(seed: number): Finding[] {
  const r = mulberry32(seed)
  return [
    { reviewer: 'A', text: 'The default is applied after the value is logged.', adjudicated: 'accepted' },
    { reviewer: 'A', text: 'The new test covers only the empty case.', adjudicated: chance(r, 0.5) ? 'accepted' : 'rejected' },
    { reviewer: 'B', text: 'The change adds a dependency that is already vendored.', adjudicated: 'rejected' },
  ]
}

export function genReport(title: string): string {
  return `${title}. The change reads the value once, applies the default, and logs the result. Two tests were added. ` +
    'The crate builds and the narrow tests pass. One reviewer finding was fixed; one was rejected as already covered.'
}

/** Run-local second at which `frac` of a phase has passed, for placing raises at a known age. */
export function localAt(phases: PhaseSpec[], scale: number, key: string, frac: number): number {
  let t = 0
  let i = 0
  while (i < phases.length) {
    const group = phases[i].reviewer ? phases.filter((p) => p.reviewer) : [phases[i]]
    const hit = group.find((p) => p.key === key)
    if (hit) return t + frac * hit.dur * scale
    t += Math.max(...group.map((p) => p.dur)) * scale
    i += group.length
  }
  throw new Error(`no phase ${key}`)
}
