// Agent output (0.11): the agent's stream as text and one row for each tool call, rather than a raw terminal. The text
// is untrusted (0.9) and goes in as text, never as markup. Following the newest output, paging from the daemon's log,
// expandable calls and the raw JSONL behind a toggle are Stage 9b.
import { Button } from '@/components/ui/button'
import type { OutputEntry } from '../types.ts'
import { TOOL_KIND_WORDS } from '../words.ts'

export interface OutputPaneProps {
  readonly entries: readonly OutputEntry[]
}

export function OutputPane({ entries }: OutputPaneProps) {
  return (
    <div className="flex flex-col gap-3">
      <div className="flex items-center justify-between gap-2">
        <p className="text-sm text-muted-foreground">What the agent said and did, newest last.</p>
        <Button variant="outline" size="sm" disabled>
          Jump to latest
        </Button>
      </div>
      <ol className="flex flex-col gap-2">
        {entries.map((entry) => (
          <li key={entry.index} className="rounded-md border p-2 text-sm">
            <Entry entry={entry} />
          </li>
        ))}
      </ol>
    </div>
  )
}

function Entry({ entry }: { readonly entry: OutputEntry }) {
  const { event } = entry
  switch (event.type) {
    case 'message':
      return <p className="whitespace-pre-wrap">{event.text}</p>
    case 'tool_call':
      return (
        <div className="flex flex-col gap-1">
          <span className="font-medium">{TOOL_KIND_WORDS[event.kind]}</span>
          {event.input === undefined ? null : (
            <code className="overflow-x-auto font-mono text-xs text-muted-foreground">{JSON.stringify(event.input)}</code>
          )}
        </div>
      )
    case 'permission_request':
      return <p>Asked permission to use {event.request.tool ?? 'a tool'}. The answer is in the Queue.</p>
    case 'final_report':
      return <p className="whitespace-pre-wrap">{event.text}</p>
    case 'permission_unanswerable':
    case 'session_bound':
    case 'usage':
    case 'turn_end':
    case 'other':
      return <p className="text-muted-foreground">Details for engineers: {event.type}</p>
  }
}
