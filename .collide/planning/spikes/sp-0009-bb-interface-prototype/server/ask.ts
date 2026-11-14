// One ask at a time: launch the configured agent, relay its tool calls and the outcome as events.
import { randomUUID } from 'node:crypto'
import type { AgentSettings } from '../src/types'
import { runAcp, runClaude, type AskEvent, type AgentResult, type RunCtx } from './agents.ts'
import type { ToolEvent } from './mcp.ts'

export interface ActiveAsk {
  id: string
  agent: string
  emit: (e: AskEvent) => void
  calls: number
  changes: number
}

export class Asks {
  private active = new Map<string, ActiveAsk>()
  private runners: Record<string, (c: RunCtx) => Promise<AgentResult>>

  constructor(runners: Record<string, (c: RunCtx) => Promise<AgentResult>> = { 'claude-code': runClaude, acp: runAcp }) {
    this.runners = runners
  }

  busy = (): boolean => this.active.size > 0
  sourceFor = (ask: string | null): string => (ask && this.active.get(ask) ? `ask: ${this.active.get(ask)!.agent}` : 'external agent')

  onTool = (ask: string | null, ev: ToolEvent): void => {
    const a = ask ? this.active.get(ask) : undefined
    if (!a) return
    a.calls += 1
    if (ev.ok && ev.changed) a.changes += 1
    const args = (ev.args ?? {}) as Record<string, any>
    const what = ev.name === 'query' ? `query ${args.subject ?? ''} ${args.filter ? JSON.stringify(args.filter) : ''}`.trim() : `${ev.name} "${args.title ?? args.surface ?? ''}"`
    a.emit({ type: 'tool', name: ev.name, summary: what.slice(0, 140), ok: ev.ok, error: ev.error, ms: ev.ms })
  }

  async run(settings: AgentSettings, agentId: string | null, request: string, root: string, base: string, emit: (e: AskEvent) => void, signal: AbortSignal): Promise<void> {
    const t0 = Date.now()
    const cfg = settings.agents.find((a) => a.id === (agentId ?? settings.askAgent))
    const end = (r: Partial<AskEvent & { type: 'end' }>, a?: ActiveAsk) =>
      emit({ type: 'end', ok: false, text: '', error: '', usage: null, calls: a?.calls ?? 0, changes: a?.changes ?? 0, ms: Date.now() - t0, ...r })
    if (!cfg) return end({ error: 'No agent is set for the ask box. Open Agents (key 5) and choose one.' })
    if (cfg.kind === 'external') return end({ error: `"${cfg.name}" is an external agent: Collide launches nothing. Connect it to the MCP endpoint and ask it there.` })
    const run = this.runners[cfg.kind]
    if (!run) return end({ error: `no launcher for agent kind "${cfg.kind}"` })
    if (this.busy()) return end({ error: 'Another ask is still running.' })
    const id = randomUUID().slice(0, 8)
    const a: ActiveAsk = { id, agent: cfg.name, emit, calls: 0, changes: 0 }
    this.active.set(id, a)
    try {
      const r = await run({ cfg, root, endpoint: `${base}/mcp?ask=${id}`, request, emit, signal })
      end({ ok: r.ok, text: r.text, error: r.error, usage: r.usage }, a)
    } catch (e) {
      end({ error: e instanceof Error ? e.message : String(e) }, a)
    } finally {
      this.active.delete(id)
    }
  }
}
