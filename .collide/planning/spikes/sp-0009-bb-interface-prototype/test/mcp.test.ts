// @vitest-environment node
// The MCP server, the page bridge, the ask endpoint and the settings endpoint, on a real Vite dev server. A fake page
// connects to the real HMR websocket and answers tool calls with the same code the browser runs.
import { mkdirSync, mkdtempSync, existsSync, readFileSync } from 'node:fs'
import { request as httpRequest } from 'node:http'
import type { AddressInfo } from 'node:net'
import { tmpdir } from 'node:os'
import { join } from 'node:path'
import { Client } from '@modelcontextprotocol/sdk/client/index.js'
import { StreamableHTTPClientTransport } from '@modelcontextprotocol/sdk/client/streamableHttp.js'
import { createServer, type ViteDevServer } from 'vite'
import { afterAll, beforeAll, describe, expect, it } from 'vitest'
import { agentDemoState } from '../src/agentDemo'
import { execTool } from '../src/agent/exec'
import { reduce } from '../src/reducer'
import type { State } from '../src/types'
import type { AgentResult, RunCtx } from '../server/agents'
import { collideAgent } from '../server/plugin'

let server: ViteDevServer
let base: string
let root: string
let page: { state: State; ws: WebSocket } | null = null

const fakeAgent = (script: (c: Client, ctx: RunCtx) => Promise<AgentResult>) => async (ctx: RunCtx): Promise<AgentResult> => {
  const client = new Client({ name: 'fake-agent', version: '0' })
  await client.connect(new StreamableHTTPClientTransport(new URL(ctx.endpoint)))
  try {
    return await script(client, ctx)
  } finally {
    await client.close()
  }
}
const text = (r: unknown): string => ((r as { content: { text: string }[] }).content[0]?.text ?? '')

beforeAll(async () => {
  root = mkdtempSync(join(tmpdir(), 'collide-mcp-test-'))
  mkdirSync(join(root, 'fixtures', 'repo'), { recursive: true })
  server = await createServer({
    configFile: false, root, logLevel: 'silent', server: { host: '127.0.0.1', port: 0 },
    plugins: [collideAgent(root, {
      'claude-code': fakeAgent(async (c) => {
        await c.callTool({ name: 'query', arguments: { subject: 'tasks', filter: { epic: ['E-09'] } } })
        await c.callTool({ name: 'set_view', arguments: { surface: 'field', title: 'Larder', filter: { epic: ['E-09'] } } })
        return { ok: true, text: 'Showed the Larder tasks.', error: '', usage: { costUsd: 0.01, inputTokens: 10, outputTokens: 2 } }
      }),
      acp: fakeAgent(async (c, ctx) => {
        if (/nothing/.test(ctx.request)) return { ok: true, text: 'I looked and changed nothing.', error: '', usage: null }
        if (/boom/.test(ctx.request)) return { ok: false, text: '', error: 'timed out after 5 s', usage: null }
        await c.callTool({ name: 'query', arguments: { subject: 'tasks' } })
        return { ok: true, text: 'only read', error: '', usage: null }
      }),
    })],
  })
  await server.listen()
  base = `http://127.0.0.1:${(server.httpServer!.address() as AddressInfo).port}`
})

afterAll(async () => {
  page?.ws.close()
  await server.close()
})

function connectPage(): Promise<void> {
  const state = { state: agentDemoState(), ws: new WebSocket(`${base.replace('http', 'ws')}/`, 'vite-hmr') }
  state.ws.addEventListener('message', (m) => {
    const msg = JSON.parse(String(m.data))
    if (msg.type !== 'custom' || msg.event !== 'collide:tool-call') return
    const c = msg.data as { id: string; name: string; args: unknown; source: string }
    const out = execTool(state.state, c.name, c.args, c.source)
    if (out.ok && out.action) state.state = reduce(state.state, { ...out.action, t: Date.now() })
    state.ws.send(JSON.stringify({ type: 'custom', event: 'collide:tool-result', data: out.ok ? { id: c.id, ok: true, result: out.result } : { id: c.id, ok: false, error: out.error } }))
  })
  page = state
  return new Promise((res) => state.ws.addEventListener('open', () => setTimeout(res, 150)))
}

async function mcp(path = '/mcp'): Promise<Client> {
  const c = new Client({ name: 'test-client', version: '0' })
  await c.connect(new StreamableHTTPClientTransport(new URL(`${base}${path}`)))
  return c
}

function raw(path: string, opts: { method?: string; headers?: Record<string, string>; body?: string }): Promise<{ status: number; body: string }> {
  const u = new URL(base)
  return new Promise((resolve, reject) => {
    const r = httpRequest({ host: u.hostname, port: u.port, path, method: opts.method ?? 'GET', headers: opts.headers }, (res) => {
      let b = ''
      res.on('data', (d) => (b += d))
      res.on('end', () => resolve({ status: res.statusCode ?? 0, body: b }))
    })
    r.on('error', reject)
    r.end(opts.body)
  })
}

describe('MCP tools', () => {
  it('tools/list names the five generic tools, none of which launches anything', async () => {
    const c = await mcp()
    const names = (await c.listTools()).tools.map((t) => t.name)
    expect(names).toEqual(['query', 'set_view', 'show_groups', 'highlight', 'open'])
    await c.close()
  })

  it('tells the agent plainly when no page is open', async () => {
    const c = await mcp()
    const r = await c.callTool({ name: 'query', arguments: { subject: 'tasks' } })
    expect(r.isError).toBe(true)
    expect(text(r)).toMatch(/No Collide page is open/)
    await c.close()
  })

  it('each tool round-trips to the page and the result comes back', async () => {
    await connectPage()
    const c = await mcp()
    const q = await c.callTool({ name: 'query', arguments: { subject: 'tasks', filter: { epic: ['E-09'] } } })
    expect(q.isError).toBeFalsy()
    expect(JSON.parse(text(q)).count).toBe(12)
    const qr = await c.callTool({ name: 'query', arguments: { subject: 'runs' } })
    expect(JSON.parse(text(qr)).rows.find((r: { id: string }) => r.id === 'R-108').phases.some((p: { key: string }) => p.key === 'build')).toBe(true)

    const v = await c.callTool({ name: 'set_view', arguments: { surface: 'tracks', title: 'Longest builds', sort: { by: 'phase:build', dir: 'desc' } } })
    expect(v.isError).toBeFalsy()
    expect(JSON.parse(text(v)).applied).toBe('true | sort phase_duration("build") desc')
    expect(page!.state.tracksView.sort).toEqual({ by: 'phase:build', dir: 'desc' })

    const g = await c.callTool({ name: 'show_groups', arguments: { surface: 'field', title: 'Groups', groups: [{ name: 'Data file', ids: ['T-09-011', 'T-09-012'], reason: 'both change saving' }] } })
    expect(JSON.parse(text(g)).grouped).toBe(2)
    expect(page!.state.lens?.groups[0].reason).toBe('both change saving')

    const h = await c.callTool({ name: 'highlight', arguments: { surface: 'field', ids: ['T-09-004'], note: 'start here' } })
    expect(JSON.parse(text(h)).highlighted).toBe(1)
    expect(page!.state.highlight?.note).toBe('start here')

    const o = await c.callTool({ name: 'open', arguments: { surface: 'field', id: 'T-09-004' } })
    expect(JSON.parse(text(o)).opened).toBe('field T-09-004')
    expect(page!.state.drawerTask).toBe('T-09-004')
    expect(page!.state.agent.changes.map((x) => x.tool)).toEqual(['set_view', 'show_groups', 'highlight', 'open'])
    expect(page!.state.agent.changes[0].source).toBe('external agent')
    await c.close()
  })

  it('returns the page error for a bad call, and changes nothing', async () => {
    const n = page!.state.agent.changes.length
    const c = await mcp()
    const r = await c.callTool({ name: 'show_groups', arguments: { surface: 'field', title: 't', groups: [{ name: 'a', ids: ['T-00-000'], reason: 'r' }] } })
    expect(r.isError).toBe(true)
    expect(text(r)).toMatch(/unknown task id T-00-000/)
    const s = await c.callTool({ name: 'set_view', arguments: { surface: 'settings', title: 't', sort: { by: 'id' } } })
    expect(text(s)).toMatch(/unknown surface "settings" for set_view; supported: field, tracks/)
    expect(page!.state.agent.changes).toHaveLength(n)
    await c.close()
  })

  it('refuses a foreign Host or Origin, and anything but POST', async () => {
    const body = JSON.stringify({ jsonrpc: '2.0', id: 1, method: 'tools/list' })
    const h = { 'content-type': 'application/json', accept: 'application/json, text/event-stream' }
    expect((await raw('/mcp', { method: 'POST', headers: { ...h, host: 'evil.example' }, body })).status).toBe(403)
    expect((await raw('/mcp', { method: 'POST', headers: { ...h, origin: 'http://evil.example' }, body })).status).toBe(403)
    expect((await raw('/mcp', { method: 'GET' })).status).toBe(405)
  })
})

describe('ask endpoint', () => {
  const put = (settings: unknown) => raw('/__settings/agents', { method: 'PUT', headers: { 'content-type': 'application/json' }, body: JSON.stringify(settings) })
  const agent = (id: string, kind: string) => ({ id, name: `Test ${id}`, kind, command: 'x', args: '', model: '', folder: 'fixtures/repo', budgetUsd: 0.1, timeoutSec: 30, env: '' })
  const ask = async (request: string, agentId?: string) => {
    const r = await raw('/__agent/ask', { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify({ request, agentId }) })
    return { status: r.status, events: r.body.split('\n').filter(Boolean).map((l) => JSON.parse(l)) }
  }

  it('launches the configured agent, relays its tool calls and the end, and the change is credited to it', async () => {
    expect((await put({ agents: [agent('a', 'claude-code'), agent('b', 'acp')], askAgent: 'a' })).status).toBe(200)
    const before = page!.state.agent.changes.length
    const r = await ask('show the larder')
    expect(r.status).toBe(200)
    const tools = r.events.filter((e) => e.type === 'tool')
    expect(tools.map((e) => e.name)).toEqual(['query', 'set_view'])
    const end = r.events[r.events.length - 1]
    expect(end).toMatchObject({ type: 'end', ok: true, calls: 2, changes: 1, text: 'Showed the Larder tasks.' })
    expect(end.usage.costUsd).toBe(0.01)
    expect(page!.state.agent.changes).toHaveLength(before + 1)
    expect(page!.state.agent.changes[before].source).toBe('ask: Test a')
  })

  it('reports an agent that read but changed nothing, and a failure', async () => {
    const none = await ask('nothing please', 'b')
    expect(none.events[none.events.length - 1]).toMatchObject({ ok: true, calls: 0, changes: 0 })
    const read = await ask('just read', 'b')
    expect(read.events[read.events.length - 1]).toMatchObject({ ok: true, calls: 1, changes: 0, text: 'only read' })
    const bad = await ask('boom', 'b')
    expect(bad.events[bad.events.length - 1]).toMatchObject({ ok: false, error: 'timed out after 5 s' })
  })

  it('refuses an external agent and an empty request', async () => {
    await put({ agents: [agent('a', 'claude-code'), { ...agent('e', 'external'), command: '' }], askAgent: 'a' })
    const e = await ask('anything', 'e')
    expect(e.events[e.events.length - 1].error).toMatch(/external agent: Collide launches nothing/)
    expect((await ask('   ')).status).toBe(400)
  })
})

describe('settings endpoint', () => {
  const req = (method: string, body?: unknown, headers: Record<string, string> = { 'content-type': 'application/json' }) =>
    raw('/__settings/agents', { method, headers, body: body === undefined ? undefined : JSON.stringify(body) })
  const good = { agents: [{ id: 'c', name: 'C', kind: 'claude-code', command: 'claude', args: '', model: 'haiku', folder: 'fixtures/repo', budgetUsd: 0.25, timeoutSec: 90, env: '' }], askAgent: 'c' }

  it('PUT writes a file under out/ and GET reads it back', async () => {
    expect((await req('PUT', good)).status).toBe(200)
    expect(existsSync(join(root, 'out', 'agents.json'))).toBe(true)
    expect(JSON.parse(readFileSync(join(root, 'out', 'agents.json'), 'utf8')).agents[0].timeoutSec).toBe(90)
    expect(JSON.parse((await req('GET')).body).askAgent).toBe('c')
  })
  it('says what is wrong: bad budget, missing folder, external as the ask agent, bad env', async () => {
    const withAgent = (patch: object, ask = 'c') => ({ agents: [{ ...good.agents[0], ...patch }], askAgent: ask })
    expect(JSON.parse((await req('PUT', withAgent({ budgetUsd: 50 }))).body).error).toMatch(/budget must be between/)
    expect(JSON.parse((await req('PUT', withAgent({ folder: 'nope' }))).body).error).toMatch(/folder "nope" does not exist/)
    expect(JSON.parse((await req('PUT', withAgent({ kind: 'external' }))).body).error).toMatch(/is external/)
    expect(JSON.parse((await req('PUT', withAgent({ kind: 'acp', env: 'oops' }))).body).error).toMatch(/not KEY=VALUE/)
    expect(JSON.parse((await req('PUT', withAgent({}, 'zz'))).body).error).toMatch(/"zz", which is not in the list/)
  })
  it('refuses a form post and a foreign origin', async () => {
    expect((await req('PUT', good, { 'content-type': 'text/plain' })).status).toBe(415)
    expect((await req('PUT', good, { 'content-type': 'application/json', origin: 'http://evil.example' })).status).toBe(403)
  })
})
