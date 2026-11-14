// Launching the agent the user configured. Collide assumes no vendor: `claude-code` and `acp` are two launchers behind
// one function; `external` launches nothing. Each launcher gets the same things: a prompt, the MCP endpoint, a folder
// to read, a time limit, and a way to report what it sees.
import { spawn, type ChildProcess } from 'node:child_process'
import type { AgentConfig } from '../src/types'
import { resolveFolder } from './settings.ts'

export type AskEvent =
  | { type: 'start'; agent: string; argv: string[] }
  | { type: 'line'; text: string }
  | { type: 'tool'; name: string; summary: string; ok: boolean; error?: string; ms: number }
  | { type: 'end'; ok: boolean; text: string; error: string; usage: Usage | null; calls: number; changes: number; ms: number }
export interface Usage {
  costUsd: number | null
  inputTokens: number | null
  outputTokens: number | null
}
export interface AgentResult {
  ok: boolean
  text: string
  error: string
  usage: Usage | null
}
export interface RunCtx {
  cfg: AgentConfig
  root: string
  /** the MCP endpoint, with the ask id */
  endpoint: string
  request: string
  emit: (e: AskEvent) => void
  signal: AbortSignal
}

export const UI_TOOLS = ['query', 'set_view', 'show_groups', 'highlight', 'open']

export const SYSTEM_PROMPT = [
  'You operate the screen of Collide, a task-oriented agent IDE, for the user, through the "collide" tools. The screen only presents what you tell it to; it does not know why. You do the analysis.',
  'Use the collide query tool to read tasks and runs, and Read, Grep and Glob to read the code folder you are in (read only).',
  'When the request is about code, files or what a change would touch, you must read the code folder (Glob its layout, then Read or Grep the files a task would change) before you group or rank anything. Task text rarely names files; work out where each change would land from the code. Only use tasks that are about the code in this folder, and say which tasks you left out. Tasks that are ready are the ones that can still be launched.',
  'Then change the screen: set_view when the answer is a filter, sort or grouping of data the screen already has; show_groups when the answer is your own judgement (named groups of ids, each with a reason); highlight to point at ids; open to open a surface.',
  'You cannot launch, accept, edit or delete anything. Never ask the user a question: make the best change, then say in one or two sentences what you did and what you were unsure about.',
  'Take ids from query, never guess them. Keep the number of calls small and finish by calling a screen-changing tool.',
].join('\n')

/** The parent is itself often a Claude Code session; its variables must not leak into the child. */
export function cleanEnv(env: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
  const out: NodeJS.ProcessEnv = {}
  for (const [k, v] of Object.entries(env)) if (!/^(CLAUDE|OTEL_)/i.test(k)) out[k] = v
  return out
}

export function claudeArgv(cfg: AgentConfig, endpoint: string): string[] {
  return [
    '-p',
    '--model', cfg.model || 'haiku',
    '--output-format', 'stream-json', '--verbose',
    '--mcp-config', JSON.stringify({ mcpServers: { collide: { type: 'http', url: endpoint } } }), '--strict-mcp-config',
    '--tools', 'Read,Grep,Glob',
    '--allowedTools', ['Read', 'Grep', 'Glob', ...UI_TOOLS.map((t) => `mcp__collide__${t}`)].join(','),
    '--max-budget-usd', String(cfg.budgetUsd),
    '--no-session-persistence', '--disable-slash-commands',
    '--append-system-prompt', SYSTEM_PROMPT,
  ]
}

const short = (x: unknown, n = 90): string => {
  const t = typeof x === 'string' ? x : JSON.stringify(x)
  return t.length > n ? `${t.slice(0, n)}...` : t
}

function lineFor(name: string, input: Record<string, unknown>): string {
  if (name === 'Read') return `Read ${short(input.file_path, 70)}`
  if (name === 'Grep') return `Grep ${short(input.pattern, 40)} ${short(input.path ?? '', 40)}`.trim()
  if (name === 'Glob') return `Glob ${short(input.pattern, 60)}`
  return `${name} ${short(input)}`
}

function watchdog(child: ChildProcess, ctx: RunCtx, flag: { timedOut: boolean }): () => void {
  const kill = () => {
    child.kill('SIGTERM')
    setTimeout(() => child.kill('SIGKILL'), 2000).unref()
  }
  const timer = setTimeout(() => {
    flag.timedOut = true
    kill()
  }, ctx.cfg.timeoutSec * 1000)
  const onAbort = () => kill()
  ctx.signal.addEventListener('abort', onAbort)
  return () => {
    clearTimeout(timer)
    ctx.signal.removeEventListener('abort', onAbort)
  }
}

export async function runClaude(ctx: RunCtx): Promise<AgentResult> {
  const argv = claudeArgv(ctx.cfg, ctx.endpoint)
  ctx.emit({ type: 'start', agent: ctx.cfg.name, argv: [ctx.cfg.command, ...argv] })
  const child = spawn(ctx.cfg.command, argv, { cwd: resolveFolder(ctx.root, ctx.cfg.folder), env: cleanEnv(process.env), stdio: ['pipe', 'pipe', 'pipe'] })
  child.stdin.end(ctx.request)
  const flag = { timedOut: false }
  const stop = watchdog(child, ctx, flag)
  let result: Record<string, any> | null = null
  let lastText = ''
  let err = ''
  let buf = ''
  child.stderr.on('data', (d) => (err += d))
  child.stdout.on('data', (d) => {
    buf += d
    let i: number
    while ((i = buf.indexOf('\n')) >= 0) {
      const line = buf.slice(0, i).trim()
      buf = buf.slice(i + 1)
      if (!line) continue
      let m: any
      try {
        m = JSON.parse(line)
      } catch {
        continue
      }
      if (m.type === 'result') result = m
      if (m.type === 'assistant') {
        for (const b of m.message?.content ?? []) {
          if (b.type === 'text' && b.text) lastText = b.text
          // Collide's own tools are reported from the MCP side, for every kind of agent; show the others here
          if (b.type === 'tool_use' && !String(b.name).startsWith('mcp__collide__')) ctx.emit({ type: 'line', text: lineFor(b.name, b.input ?? {}) })
        }
      }
    }
  })
  const code = await new Promise<number | null>((res) => {
    child.on('error', (e) => {
      err += String(e)
      res(-1)
    })
    child.on('close', (c) => res(c))
  })
  stop()
  if (ctx.signal.aborted) return { ok: false, text: lastText, error: 'cancelled', usage: null }
  if (flag.timedOut) return { ok: false, text: lastText, error: `timed out after ${ctx.cfg.timeoutSec} s`, usage: usageOf(result) }
  const r = result as Record<string, any> | null
  if (!r) return { ok: false, text: lastText, error: `claude exited ${code} with no result. ${err.trim().slice(0, 300)}`.trim(), usage: null }
  if (r.is_error || r.subtype !== 'success') {
    const why = r.subtype === 'error_max_budget_usd' ? `stopped at the $${ctx.cfg.budgetUsd} budget` : `${r.subtype ?? 'error'}: ${short(r.result ?? err, 200)}`
    return { ok: false, text: String(r.result ?? lastText), error: why, usage: usageOf(r) }
  }
  return { ok: true, text: String(r.result ?? lastText), error: '', usage: usageOf(r) }
}

function usageOf(r: Record<string, any> | null): Usage | null {
  if (!r) return null
  const u = r.usage ?? {}
  const input = (u.input_tokens ?? 0) + (u.cache_creation_input_tokens ?? 0) + (u.cache_read_input_tokens ?? 0)
  return { costUsd: typeof r.total_cost_usd === 'number' ? r.total_cost_usd : null, inputTokens: input || null, outputTokens: u.output_tokens ?? null }
}

// ---- ACP: a minimal JSON-RPC client over stdio (the shape S7 measured against opencode) ----

export function acpArgv(cfg: AgentConfig): string[] {
  return cfg.args.split(/\s+/).filter(Boolean)
}

export function acpEnv(cfg: AgentConfig, parent: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
  const env: NodeJS.ProcessEnv = { PATH: parent.PATH, HOME: parent.HOME }
  for (const line of cfg.env.split('\n').map((l) => l.trim()).filter(Boolean)) {
    const i = line.indexOf('=')
    env[line.slice(0, i)] = line.slice(i + 1)
  }
  return env
}

export async function runAcp(ctx: RunCtx): Promise<AgentResult> {
  const argv = acpArgv(ctx.cfg)
  const cwd = resolveFolder(ctx.root, ctx.cfg.folder)
  ctx.emit({ type: 'start', agent: ctx.cfg.name, argv: [ctx.cfg.command, ...argv] })
  const child = spawn(ctx.cfg.command, argv, { cwd, env: acpEnv(ctx.cfg, process.env), stdio: ['pipe', 'pipe', 'pipe'] })
  const flag = { timedOut: false }
  const stop = watchdog(child, ctx, flag)
  const pending = new Map<number, { res: (v: any) => void; rej: (e: Error) => void }>()
  let nextId = 1
  let text = ''
  let err = ''
  const send = (m: unknown) => child.stdin.write(`${JSON.stringify(m)}\n`)
  const request = (method: string, params: unknown) =>
    new Promise<any>((res, rej) => {
      const id = nextId++
      pending.set(id, { res, rej })
      send({ jsonrpc: '2.0', id, method, params })
    })
  child.stderr.on('data', (d) => (err += d))
  child.on('close', () => {
    for (const p of pending.values()) p.rej(new Error('the agent process exited'))
    pending.clear()
  })
  let buf = ''
  child.stdout.on('data', (d) => {
    buf += d
    let i: number
    while ((i = buf.indexOf('\n')) >= 0) {
      const line = buf.slice(0, i).trim()
      buf = buf.slice(i + 1)
      let m: any
      try {
        m = JSON.parse(line)
      } catch {
        continue
      }
      if (m.method && m.id !== undefined) {
        // a request from the agent: only reading may be allowed
        if (m.method === 'session/request_permission') {
          const tc = m.params?.toolCall ?? {}
          const readish = ['read', 'search', 'fetch', 'think'].includes(tc.kind) || /collide/i.test(JSON.stringify(tc.title ?? ''))
          const want = readish ? 'allow_once' : 'reject_once'
          const o = (m.params?.options ?? []).find((x: any) => x.kind === want)
          ctx.emit({ type: 'line', text: `permission ${readish ? 'allowed' : 'refused'}: ${short(tc.title ?? tc.kind ?? '')}` })
          send({ jsonrpc: '2.0', id: m.id, result: { outcome: o ? { outcome: 'selected', optionId: o.optionId } : { outcome: 'cancelled' } } })
        } else send({ jsonrpc: '2.0', id: m.id, error: { code: -32601, message: 'not supported by Collide' } })
      } else if (m.method === 'session/update') {
        const u = m.params?.update ?? {}
        if (u.sessionUpdate === 'agent_message_chunk' && u.content?.type === 'text') text += u.content.text
        if (u.sessionUpdate === 'tool_call' && !/collide/i.test(String(u.title))) ctx.emit({ type: 'line', text: short(u.title ?? u.kind ?? 'tool') })
      } else if (m.id !== undefined && pending.has(m.id)) {
        const p = pending.get(m.id)!
        pending.delete(m.id)
        if (m.error) p.rej(new Error(m.error.message ?? JSON.stringify(m.error)))
        else p.res(m.result)
      }
    }
  })
  try {
    await request('initialize', { protocolVersion: 1, clientCapabilities: {} })
    const created = await request('session/new', { cwd, mcpServers: [{ type: 'http', name: 'collide', url: ctx.endpoint, headers: [] }] })
    const sessionId = created.sessionId as string
    if (ctx.cfg.model) {
      try {
        await request('session/set_model', { sessionId, modelId: ctx.cfg.model })
      } catch (e) {
        ctx.emit({ type: 'line', text: `model not set (${e instanceof Error ? e.message : String(e)}); the agent's own default is used` })
      }
    }
    const done = await request('session/prompt', { sessionId, prompt: [{ type: 'text', text: `${SYSTEM_PROMPT}\n\nRequest: ${ctx.request}` }] })
    stop()
    child.kill('SIGTERM')
    const failed = done.stopReason !== 'end_turn'
    return { ok: !failed, text: text.trim(), error: failed ? `stop reason ${done.stopReason}` : '', usage: null }
  } catch (e) {
    stop()
    child.kill('SIGTERM')
    if (ctx.signal.aborted) return { ok: false, text, error: 'cancelled', usage: null }
    if (flag.timedOut) return { ok: false, text, error: `timed out after ${ctx.cfg.timeoutSec} s`, usage: null }
    return { ok: false, text, error: `${e instanceof Error ? e.message : String(e)}${err.trim() ? `. stderr: ${err.trim().slice(-300)}` : ''}`, usage: null }
  }
}
