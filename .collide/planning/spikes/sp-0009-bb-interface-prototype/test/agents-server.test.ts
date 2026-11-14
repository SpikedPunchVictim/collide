// @vitest-environment node
// How Collide launches the configured agents: the Claude Code argv and environment, and the ACP launcher's environment.
import { describe, expect, it } from 'vitest'
import { acpArgv, acpEnv, claudeArgv, cleanEnv, UI_TOOLS } from '../server/agents'
import { defaultSettings } from '../server/settings'

const cfg = defaultSettings().agents[0]

describe('Claude Code launch', () => {
  const argv = claudeArgv(cfg, 'http://127.0.0.1:5173/mcp?ask=abc')
  const after = (flag: string) => argv[argv.indexOf(flag) + 1]
  it('uses only the Collide MCP server, and the cheap model, with the budget from settings', () => {
    expect(argv).toContain('--strict-mcp-config')
    expect(JSON.parse(after('--mcp-config'))).toEqual({ mcpServers: { collide: { type: 'http', url: 'http://127.0.0.1:5173/mcp?ask=abc' } } })
    expect(after('--model')).toBe('haiku')
    expect(after('--max-budget-usd')).toBe('0.25')
    expect(argv).toContain('--no-session-persistence')
    expect(after('--output-format')).toBe('stream-json')
  })
  it('gives the agent reading tools and the UI tools, and no way to run or write', () => {
    expect(after('--tools')).toBe('Read,Grep,Glob')
    const allowed = after('--allowedTools').split(',')
    expect(allowed).toEqual(['Read', 'Grep', 'Glob', ...UI_TOOLS.map((t) => `mcp__collide__${t}`)])
    expect(allowed.some((t) => /Bash|Edit|Write/.test(t))).toBe(false)
  })
  it('keeps the request off the command line (it goes in on stdin)', () => {
    expect(argv.every((a) => !/larder/i.test(a) || a === argv[argv.indexOf('--append-system-prompt') + 1])).toBe(true)
  })
})

describe('environments', () => {
  it('cleanEnv drops CLAUDE* and OTEL_* and keeps the rest', () => {
    const e = cleanEnv({ PATH: '/bin', HOME: '/h', CLAUDECODE: '1', CLAUDE_CODE_ENTRYPOINT: 'cli', OTEL_EXPORTER_OTLP_ENDPOINT: 'x', claude_lower: 'y', KEEP: 'z' })
    expect(Object.keys(e).sort()).toEqual(['HOME', 'KEEP', 'PATH'])
  })
  it('an ACP agent starts with PATH, HOME and what the settings add, nothing else', () => {
    const c = { ...cfg, kind: 'acp' as const, args: 'acp --x', env: 'A=1\n\nB=two=2' }
    expect(acpArgv(c)).toEqual(['acp', '--x'])
    expect(acpEnv(c, { PATH: '/bin', HOME: '/h', SECRET: 's', ANTHROPIC_API_KEY: 'k' })).toEqual({ PATH: '/bin', HOME: '/h', A: '1', B: 'two=2' })
  })
})
