// Collide's UI tools as an MCP server: streamable HTTP, stateless, JSON responses. Every tool call is forwarded to the
// page (server/bridge.ts) and its outcome comes back as the tool result. Read-only or view-only: no tool launches,
// accepts, edits or deletes anything.
import type { IncomingMessage, ServerResponse } from 'node:http'
import { McpServer } from '@modelcontextprotocol/sdk/server/mcp.js'
import { StreamableHTTPServerTransport } from '@modelcontextprotocol/sdk/server/streamableHttp.js'
import * as z from 'zod'
import type { PageLink } from './bridge.ts'

export interface ToolEvent {
  name: string
  args: unknown
  ok: boolean
  error?: string
  ms: number
  /** a tool that changed the screen (not query) */
  changed: boolean
}

export interface McpHooks {
  /** called after each tool call; ask is the id from `/mcp?ask=<id>` when Collide launched the agent */
  onTool?: (ask: string | null, ev: ToolEvent) => void
  /** who the page should credit the change to */
  sourceFor?: (ask: string | null) => string
}

const surface = z.string().describe('Which surface: "field" (tasks board) or "tracks" (runs on a time axis).')
const taskFilter = z.object({
  epic: z.array(z.string()).optional().describe('Epic ids, e.g. ["E-09"]'),
  status: z.array(z.string()).optional().describe('tasks: ready, blocked, backlog, running, review, done. runs: working, waiting, failed, paused, done, stopped'),
  label: z.array(z.string()).optional().describe('tasks only: labels, any of'),
  source: z.array(z.string()).optional().describe('tasks only: "tasks/" or "github"'),
  text: z.string().optional().describe('tasks only: substring of id or title'),
  ids: z.array(z.string()).optional().describe('exact ids (task ids for tasks, run ids for runs)'),
  task: z.array(z.string()).optional().describe('runs only: task ids the runs belong to'),
  workflow: z.array(z.string()).optional().describe('runs only: build-review or quick-fix'),
})
const sort = z.object({
  by: z.string().describe('field: id, title, epic, labels, status. tracks: total, start, end, spend, wait, or phase:<key> such as phase:build (seconds in that phase)'),
  dir: z.string().optional().describe('"asc" (default) or "desc"'),
})

export function buildMcpServer(link: PageLink, ask: string | null, hooks: McpHooks = {}): McpServer {
  const server = new McpServer({ name: 'collide-ui', version: '0.1.0' })
  const source = hooks.sourceFor?.(ask) ?? (ask ? 'ask' : 'external')

  const reg = (name: string, description: string, inputSchema: z.ZodRawShape, changed: boolean) => {
    server.registerTool(name, { description, inputSchema }, async (args: unknown) => {
      const t0 = Date.now()
      const out = await link.call(name, args, source)
      hooks.onTool?.(ask, { name, args, ok: out.ok, error: out.ok ? undefined : out.error, ms: Date.now() - t0, changed })
      if (!out.ok) return { isError: true, content: [{ type: 'text' as const, text: `Error: ${out.error}` }] }
      return { content: [{ type: 'text' as const, text: JSON.stringify(out.result) }] }
    })
  }

  reg('query',
    'Read-only. Returns the rows Collide shows: tasks (id, title, body, epic, status, labels, source, depends_on) or runs (id, task, status, workflow, started, total_seconds, wait_seconds, spend_usd, phases with seconds). Use it to get real ids before calling any other tool.',
    { subject: z.string().describe('"tasks" or "runs"'), filter: taskFilter.optional(), limit: z.number().optional().describe('default 200, max 500') }, false)
  reg('set_view',
    'Set the filter, sort or grouping of a surface to a view of data the screen already has. Parts you give replace that part; parts you leave out stay. The user sees a chip "Agent: <title>" with the expression, Undo and Save as view. Use this when the answer is a filter or a sort. Example: tracks sorted by build time: {surface:"tracks", title:"Longest builds", sort:{by:"phase:build", dir:"desc"}}.',
    {
      surface, title: z.string().describe('Short label for the chip, under 60 characters'), reason: z.string().optional().describe('One line the user can read: why this view'),
      filter: taskFilter.optional().describe('Pass {} to clear the filter'), sort: sort.optional(),
      group_by: z.string().optional().describe('field only: status (default), epic, label, source'),
      window: z.object({ from: z.string(), to: z.string() }).optional().describe('tracks only: time window as clock times like "09:15", in the clock query shows in "started"'),
    }, true)
  reg('show_groups',
    'Show your own grouping as a lens: named groups of ids, each with a reason, drawn on the surface and stamped with the time. Use this when the answer is your judgement rather than a filter. Each id may be in one group only; leave unsure ids out and they appear as "not grouped". The user may launch a group themselves; you cannot.',
    {
      surface, title: z.string().describe('Short label for the chip, under 60 characters'),
      groups: z.array(z.object({ name: z.string(), ids: z.array(z.string()), reason: z.string().describe('Why these belong together') })).describe('1 to 12 groups'),
      rest: z.string().optional().describe('What to do with visible items you did not group: "dim" (default) or "hide"'),
    }, true)
  reg('highlight',
    'Mark ids on a surface so the user sees them, with an optional note. Nothing is filtered.',
    { surface, ids: z.array(z.string()), note: z.string().optional(), title: z.string().optional().describe('Chip label; default "Highlight N tasks"') }, true)
  reg('open',
    'Open a surface, and optionally one item on it: a task on field, a run on tracks, a queue item on queue, an experiment on experiments.',
    { surface: z.string().describe('field, tracks, queue or experiments'), id: z.string().optional() }, true)
  return server
}

/** The page must be reached from the same machine: refuse a foreign Host (DNS rebinding) or a foreign Origin (a web page). */
export function guard(req: IncomingMessage): string | null {
  const host = req.headers.host ?? ''
  if (!/^(127\.0\.0\.1|localhost|\[::1\])(:\d+)?$/.test(host)) return `refused: Host "${host}"`
  const origin = req.headers.origin
  if (origin) {
    let o: URL
    try {
      o = new URL(origin)
    } catch {
      return 'refused: bad Origin'
    }
    if (o.host !== host) return `refused: Origin "${origin}"`
  }
  return null
}

async function readBody(req: IncomingMessage, limit = 1_000_000): Promise<string> {
  let body = ''
  for await (const chunk of req) {
    body += chunk
    if (body.length > limit) throw new Error('body too large')
  }
  return body
}

export async function handleMcp(req: IncomingMessage, res: ServerResponse, link: PageLink, hooks: McpHooks = {}): Promise<void> {
  const refused = guard(req)
  if (refused) {
    res.statusCode = 403
    res.end(refused)
    return
  }
  if (req.method !== 'POST') {
    res.statusCode = 405
    res.setHeader('allow', 'POST')
    res.end('This endpoint is stateless: POST JSON-RPC messages only.')
    return
  }
  const ask = new URL(req.url ?? '/', 'http://local').searchParams.get('ask')
  let parsed: unknown
  try {
    parsed = JSON.parse(await readBody(req))
  } catch (e) {
    res.statusCode = 400
    res.end(`bad JSON body: ${e instanceof Error ? e.message : String(e)}`)
    return
  }
  const server = buildMcpServer(link, ask, hooks)
  const transport = new StreamableHTTPServerTransport({ sessionIdGenerator: undefined, enableJsonResponse: true })
  res.on('close', () => {
    void transport.close()
    void server.close()
  })
  await server.connect(transport)
  await transport.handleRequest(req, res, parsed)
}
