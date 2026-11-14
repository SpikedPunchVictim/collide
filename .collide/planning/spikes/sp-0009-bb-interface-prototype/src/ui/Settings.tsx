// "Agents": which agent the user brings. A prototype stand-in for Collide's settings; see record.md for what a real one needs.
import { loadAgentSettings, saveAgentSettings } from '../agent/page'
import { useApp } from './store'
import type { AgentConfig, AgentKind } from '../types'

const KIND_LABEL: Record<AgentKind, string> = { 'claude-code': 'Claude Code', acp: 'ACP agent', external: 'External' }

export function mcpUrl(): string {
  return `${location.origin}/mcp`
}

export const claudeSnippet = (url: string): string => `claude mcp add --transport http collide ${url}`
export const jsonSnippet = (url: string): string => JSON.stringify({ mcpServers: { collide: { type: 'http', url } } }, null, 2)
export const opencodeSnippet = (url: string): string => JSON.stringify({ mcp: { collide: { type: 'remote', url, enabled: true } } }, null, 2)

function Snippet({ label, text }: { label: string; text: string }) {
  return (
    <div className="snippet">
      <span className="lbl">{label}</span>
      <pre className="mono" tabIndex={0}>{text}</pre>
    </div>
  )
}

function AgentCard({ a }: { a: AgentConfig }) {
  const { s, d } = useApp()
  const st = s.agentSettings.settings!
  const set = (patch: Partial<Omit<AgentConfig, 'id'>>) => d({ type: 'settingsEdit', id: a.id, patch })
  const launchable = a.kind !== 'external'
  const url = mcpUrl()
  return (
    <article className="agentcard" aria-label={a.name}>
      <header>
        <input className="aname" aria-label="Name" value={a.name} onChange={(e) => set({ name: e.target.value })} />
        <label className="kind">Kind{' '}
          <select aria-label="Kind" value={a.kind} onChange={(e) => set({ kind: e.target.value as AgentKind })}>
            {(Object.keys(KIND_LABEL) as AgentKind[]).map((k) => <option key={k} value={k}>{KIND_LABEL[k]}</option>)}
          </select>
        </label>
        <span className="spacer" />
        {launchable && (
          <label className="usedby">
            <input type="radio" name="askagent" checked={st.askAgent === a.id} onChange={() => d({ type: 'settingsAskAgent', id: a.id })} />
            {' '}Used by the ask box
          </label>
        )}
        <button type="button" className="btn sm" onClick={() => d({ type: 'settingsRemove', id: a.id })}>Remove</button>
      </header>
      {launchable ? (
        <div className="agentgrid">
          <label>{a.kind === 'claude-code' ? 'Command or path' : 'Command'}
            <input className="mono" aria-label="Command" value={a.command} placeholder={a.kind === 'claude-code' ? 'claude' : '~/.opencode/bin/opencode'} onChange={(e) => set({ command: e.target.value })} />
          </label>
          {a.kind === 'acp' && (
            <label>Arguments
              <input className="mono" aria-label="Arguments" value={a.args} placeholder="acp" onChange={(e) => set({ args: e.target.value })} />
            </label>
          )}
          <label>Model
            <input className="mono" aria-label="Model" value={a.model} placeholder={a.kind === 'claude-code' ? 'haiku' : "the agent's own default"} onChange={(e) => set({ model: e.target.value })} />
          </label>
          <label>Folder the agent may read
            <input className="mono" aria-label="Folder" value={a.folder} placeholder="fixtures/repo" onChange={(e) => set({ folder: e.target.value })} />
          </label>
          <label>Budget per request (USD)
            <input type="number" min="0.01" max="5" step="0.05" aria-label="Budget" value={a.budgetUsd} onChange={(e) => set({ budgetUsd: Number(e.target.value) })} />
          </label>
          <label>Timeout (seconds)
            <input type="number" min="5" max="1800" step="5" aria-label="Timeout" value={a.timeoutSec} onChange={(e) => set({ timeoutSec: Number(e.target.value) })} />
          </label>
          {a.kind === 'acp' && (
            <label className="wide">Environment (KEY=VALUE per line; the agent starts with only PATH and HOME otherwise)
              <textarea className="mono" rows={3} aria-label="Environment" value={a.env} onChange={(e) => set({ env: e.target.value })} />
            </label>
          )}
          <p className="note wide">
            {a.kind === 'claude-code'
              ? 'Runs claude -p with only the Collide tools plus Read, Grep and Glob in that folder, and your own MCP servers off. The budget is enforced by Claude Code.'
              : 'Speaks ACP over stdio and hands the agent the Collide endpoint. The agent chooses its own tools: the folder is its working directory, not a sandbox, and the budget is not enforced (ACP carries no cost); only the timeout is.'}
          </p>
        </div>
      ) : (
        <div className="external">
          <p>Collide launches nothing for this agent. Connect the agent you already run to the endpoint below; what it does through the tools shows up here as chips, the same as an ask.</p>
          <p><span className="lbl">MCP endpoint</span> <code className="mono endpoint">{url}</code> <span className="note">streamable HTTP, this machine only, no sign-in</span></p>
          <Snippet label="Claude Code" text={claudeSnippet(url)} />
          <Snippet label="Any MCP client (JSON)" text={jsonSnippet(url)} />
          <Snippet label="opencode (opencode.json)" text={opencodeSnippet(url)} />
          <p className="note">Shown as text only; Collide does not write to your agent's configuration.</p>
        </div>
      )}
    </article>
  )
}

export function Settings() {
  const { s, d } = useApp()
  const as = s.agentSettings
  const st = as.settings
  return (
    <div className="settings">
      <header className="sethead">
        <h2>Agents</h2>
        <span className="note">The agents you bring. Collide assumes none of them.</span>
        <span className="spacer" />
        {as.dirty && <span className="note" role="status">Unsaved changes</span>}
        {!as.dirty && as.savedAt && <span className="note" role="status">Saved</span>}
        <button type="button" className="btn primary" disabled={!st || !as.dirty} onClick={() => st && void saveAgentSettings(st, d)}>Save</button>
      </header>
      {as.error && (
        <p className="bad" role="alert">
          {as.error} {!as.loaded && <button type="button" className="btn sm" onClick={() => void loadAgentSettings(d)}>Retry</button>}
        </p>
      )}
      {!st && !as.error && <p className="note">Loading...</p>}
      {st && (
        <>
          <p className="note">
            The ask box uses: <b>{st.agents.find((a) => a.id === st.askAgent)?.name ?? 'nothing yet'}</b>. A saved change takes effect on the next ask.
            An agent you run yourself needs no setup here: add an External entry for the snippets.
          </p>
          {st.agents.map((a) => <AgentCard key={a.id} a={a} />)}
          <div className="addrow">
            <span className="lbl">Add</span>
            {(Object.keys(KIND_LABEL) as AgentKind[]).map((k) => <button key={k} type="button" className="btn sm" onClick={() => d({ type: 'settingsAdd', kind: k })}>{KIND_LABEL[k]}</button>)}
          </div>
        </>
      )}
    </div>
  )
}
