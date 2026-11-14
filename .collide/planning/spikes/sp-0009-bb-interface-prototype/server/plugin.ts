// Vite plugin: the MCP endpoint, the ask endpoint and the agent settings endpoint, all on the dev server, local only.
import { appendFileSync, mkdirSync } from 'node:fs'
import type { IncomingMessage, ServerResponse } from 'node:http'
import { join } from 'node:path'
import type { Plugin } from 'vite'
import { Asks } from './ask.ts'
import type { AgentResult, RunCtx } from './agents.ts'
import { createPageLink } from './bridge.ts'
import { guard, handleMcp } from './mcp.ts'
import { loadSettings, saveSettings } from './settings.ts'

async function json(req: IncomingMessage): Promise<unknown> {
  let body = ''
  for await (const c of req) {
    body += c
    if (body.length > 200_000) throw new Error('body too large')
  }
  return JSON.parse(body || 'null')
}

function reply(res: ServerResponse, code: number, body: unknown): void {
  res.statusCode = code
  res.setHeader('content-type', 'application/json')
  res.end(JSON.stringify(body))
}

/** A browser page on another origin can send a simple cross-site POST; requiring JSON content makes it need a preflight. */
const jsonOnly = (req: IncomingMessage): boolean => /^application\/json\b/.test(req.headers['content-type'] ?? '')

export function collideAgent(root = process.cwd(), runners?: Record<string, (c: RunCtx) => Promise<AgentResult>>): Plugin {
  return {
    name: 'collide-agent',
    configureServer(server) {
      const link = createPageLink(server.ws)
      const asks = new Asks(runners)
      const hooks = { onTool: asks.onTool, sourceFor: asks.sourceFor }

      server.middlewares.use('/mcp', (req, res) => {
        handleMcp(req, res, link, hooks).catch((e) => {
          if (!res.headersSent) reply(res, 500, { error: String(e) })
          else res.end()
        })
      })

      server.middlewares.use('/__settings/agents', async (req, res) => {
        const refused = guard(req)
        if (refused) return reply(res, 403, { error: refused })
        if (req.method === 'GET') return reply(res, 200, loadSettings(root))
        if (req.method !== 'PUT') return reply(res, 405, { error: 'GET or PUT' })
        if (!jsonOnly(req)) return reply(res, 415, { error: 'send application/json' })
        try {
          const v = saveSettings(root, await json(req))
          if (!v.ok) return reply(res, 400, { error: v.error })
          reply(res, 200, v.settings)
        } catch (e) {
          reply(res, 400, { error: String(e) })
        }
      })

      server.middlewares.use('/__agent/ask', async (req, res) => {
        const refused = guard(req)
        if (refused) return reply(res, 403, { error: refused })
        if (req.method !== 'POST') return reply(res, 405, { error: 'POST' })
        if (!jsonOnly(req)) return reply(res, 415, { error: 'send application/json' })
        let body: { request?: unknown; agentId?: unknown }
        try {
          body = (await json(req)) as typeof body
        } catch (e) {
          return reply(res, 400, { error: String(e) })
        }
        const request = typeof body?.request === 'string' ? body.request.trim() : ''
        if (!request || request.length > 2000) return reply(res, 400, { error: 'request must be 1 to 2000 characters' })
        res.statusCode = 200
        res.setHeader('content-type', 'application/x-ndjson')
        const ac = new AbortController()
        let finished = false
        res.on('close', () => {
          if (!finished) ac.abort()
        })
        // every ask is also logged with timestamps under out/asks/ (git-ignored), for the record
        const t0 = Date.now()
        mkdirSync(join(root, 'out', 'asks'), { recursive: true })
        const log = join(root, 'out', 'asks', `ask-${new Date(t0).toISOString().replace(/[:.]/g, '-')}.jsonl`)
        appendFileSync(log, `${JSON.stringify({ t: 0, type: 'request', request })}\n`)
        const emit = (e: unknown) => {
          appendFileSync(log, `${JSON.stringify({ t: Date.now() - t0, ...(e as object) })}\n`)
          if (!res.writableEnded) res.write(`${JSON.stringify(e)}\n`)
        }
        const base = `http://${req.headers.host}`
        await asks.run(loadSettings(root), typeof body.agentId === 'string' ? body.agentId : null, request, root, base, emit, ac.signal)
        finished = true
        res.end()
      })
    },
  }
}
