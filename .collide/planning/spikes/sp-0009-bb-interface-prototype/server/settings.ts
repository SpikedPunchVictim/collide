// Agent settings: a prototype stand-in for Collide's real settings layers. One JSON file under out/ (git-ignored).
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from 'node:fs'
import { dirname, isAbsolute, join, resolve } from 'node:path'
import type { AgentConfig, AgentKind, AgentSettings } from '../src/types'

export const SETTINGS_PATH = join('out', 'agents.json')
export const KINDS: AgentKind[] = ['claude-code', 'acp', 'external']

export function defaultSettings(): AgentSettings {
  return {
    agents: [
      {
        id: 'claude', name: 'Claude Code (haiku)', kind: 'claude-code', command: 'claude', args: '', model: 'haiku', folder: 'fixtures/repo',
        budgetUsd: 0.25, timeoutSec: 180, env: '',
      },
      {
        id: 'external', name: 'My own agent', kind: 'external', command: '', args: '', model: '', folder: 'fixtures/repo',
        budgetUsd: 0, timeoutSec: 0, env: '',
      },
    ],
    askAgent: 'claude',
  }
}

export const resolveFolder = (root: string, folder: string): string => (isAbsolute(folder) ? folder : resolve(root, folder))

export type Validated = { ok: true; settings: AgentSettings } | { ok: false; error: string }

export function validateSettings(x: unknown, root: string): Validated {
  const bad = (error: string): Validated => ({ ok: false, error })
  if (typeof x !== 'object' || x === null) return bad('settings must be an object')
  const o = x as Record<string, unknown>
  if (!Array.isArray(o.agents)) return bad('agents must be a list')
  const ids = new Set<string>()
  const agents: AgentConfig[] = []
  for (const [i, raw] of o.agents.entries()) {
    const r = (raw ?? {}) as Record<string, unknown>
    const at = `agent ${i + 1}`
    const str = (k: string) => (typeof r[k] === 'string' ? (r[k] as string).trim() : '')
    const id = str('id')
    if (!/^[a-z0-9][a-z0-9-]*$/i.test(id)) return bad(`${at}: id must be letters, digits and dashes`)
    if (ids.has(id)) return bad(`${at}: id "${id}" is used twice`)
    ids.add(id)
    const name = str('name')
    if (!name) return bad(`${at}: name is required`)
    const kind = str('kind') as AgentKind
    if (!KINDS.includes(kind)) return bad(`${name}: kind must be one of ${KINDS.join(', ')}`)
    const c: AgentConfig = {
      id, name, kind, command: str('command'), args: str('args'), model: str('model'), folder: str('folder'),
      budgetUsd: Number(r.budgetUsd ?? 0), timeoutSec: Number(r.timeoutSec ?? 0), env: typeof r.env === 'string' ? r.env : '',
    }
    if (kind !== 'external') {
      if (!c.command) return bad(`${name}: command is required`)
      if (!c.folder) return bad(`${name}: the folder the agent may read is required`)
      const dir = resolveFolder(root, c.folder)
      if (!existsSync(dir) || !statSync(dir).isDirectory()) return bad(`${name}: folder "${c.folder}" does not exist`)
      if (!(c.budgetUsd >= 0.01 && c.budgetUsd <= 5)) return bad(`${name}: budget must be between $0.01 and $5 per request`)
      if (!(c.timeoutSec >= 5 && c.timeoutSec <= 1800)) return bad(`${name}: timeout must be between 5 and 1800 seconds`)
      for (const line of c.env.split('\n').map((l) => l.trim()).filter(Boolean)) {
        if (!/^[A-Za-z_][A-Za-z0-9_]*=/.test(line)) return bad(`${name}: env line "${line}" is not KEY=VALUE`)
      }
    }
    agents.push(c)
  }
  const ask = typeof o.askAgent === 'string' ? o.askAgent : null
  if (ask !== null) {
    const a = agents.find((x) => x.id === ask)
    if (!a) return bad(`the ask box uses "${ask}", which is not in the list`)
    if (a.kind === 'external') return bad(`"${a.name}" is external: Collide launches nothing for it, so the ask box cannot use it`)
  }
  return { ok: true, settings: { agents, askAgent: ask } }
}

export function loadSettings(root: string): AgentSettings {
  const path = join(root, SETTINGS_PATH)
  if (!existsSync(path)) return defaultSettings()
  try {
    const v = validateSettings(JSON.parse(readFileSync(path, 'utf8')), root)
    return v.ok ? v.settings : defaultSettings()
  } catch {
    return defaultSettings()
  }
}

export function saveSettings(root: string, x: unknown): Validated {
  const v = validateSettings(x, root)
  if (!v.ok) return v
  const path = join(root, SETTINGS_PATH)
  mkdirSync(dirname(path), { recursive: true })
  writeFileSync(path, JSON.stringify(v.settings, null, 2))
  return v
}
