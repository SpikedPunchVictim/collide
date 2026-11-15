// Queue (0.11): every decision waiting on the user, oldest first, as a list grouped by kind and a detail pane. The pane
// shows the exact command or arguments (untrusted ones flagged, 0.9), the prompt-cache countdown, and the context view.
// The mini track and recent activity, and the keyboard triage (`j k i o u a d r`), are Stage 9a.
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { cn } from '@/lib/utils'
import type { AnswerKind, ContextRecord, QueueItem } from '../types.ts'
import { ANSWER_WORDS, QUEUE_KIND_WORDS, TOOL_KIND_WORDS, durationWords } from '../words.ts'
import { ContextView } from './ContextView.tsx'

export interface QueueSurfaceProps {
  /** Ordered by how long each has blocked work, oldest first, as `queue.list` returns them. */
  readonly items: readonly QueueItem[]
  /** The open item's id, or `null` for the first one. */
  readonly selected: string | null
  /** What Collide assembled for the open item, or `null` before it has loaded. Not every section need be present. */
  readonly context: Partial<ContextRecord> | null
  /** The wall clock "now", passed in so a render is the same every time. */
  readonly nowMs: number
  readonly onSelect: (itemId: string) => void
  readonly onAnswer: (itemId: string, answer: AnswerKind) => void
}

export function QueueSurface(props: QueueSurfaceProps) {
  const { items, nowMs } = props
  const open = items.find((i) => i.id === props.selected) ?? items[0] ?? null
  return (
    <section aria-labelledby="queue-heading" className="flex flex-col gap-4">
      <h2 id="queue-heading" className="text-xl font-semibold">
        Queue
      </h2>
      {open === null ? (
        <p className="text-sm text-muted-foreground">Nothing is waiting on you.</p>
      ) : (
        <div className="grid gap-4 md:grid-cols-[minmax(14rem,1fr)_2fr]">
          <ItemList items={items} openId={open.id} nowMs={nowMs} onSelect={props.onSelect} />
          <ItemDetail item={open} context={props.context} nowMs={nowMs} onAnswer={props.onAnswer} />
        </div>
      )}
    </section>
  )
}

function waitingFor(item: QueueItem, nowMs: number): string {
  return `waiting ${durationWords(Math.max(0, nowMs - item.since_ms))}`
}

interface ItemListProps {
  readonly items: readonly QueueItem[]
  readonly openId: string
  readonly nowMs: number
  readonly onSelect: (itemId: string) => void
}

function ItemList({ items, openId, nowMs, onSelect }: ItemListProps) {
  const kinds = [...new Set(items.map((i) => i.kind))]
  return (
    <div className="flex flex-col gap-3">
      {kinds.map((kind) => (
        <div key={kind} className="flex flex-col gap-1">
          <h3 className="text-sm font-medium text-muted-foreground">{QUEUE_KIND_WORDS[kind]}</h3>
          {items
            .filter((i) => i.kind === kind)
            .map((item) => (
              <button
                key={item.id}
                type="button"
                aria-current={item.id === openId}
                onClick={() => onSelect(item.id)}
                className={cn(
                  'rounded-lg border p-3 text-left text-sm hover:bg-muted',
                  item.id === openId && 'bg-muted ring-1 ring-ring',
                )}
              >
                <span className="block font-medium">{item.title}</span>
                <span className="block text-muted-foreground">
                  {item.run ? `${item.run}, ` : ''}
                  {waitingFor(item, nowMs)}
                </span>
              </button>
            ))}
        </div>
      ))}
    </div>
  )
}

interface ItemDetailProps {
  readonly item: QueueItem
  readonly context: Partial<ContextRecord> | null
  readonly nowMs: number
  readonly onAnswer: (itemId: string, answer: AnswerKind) => void
}

function ItemDetail({ item, context, nowMs, onAnswer }: ItemDetailProps) {
  return (
    <Card>
      <CardHeader>
        <CardTitle>{item.title}</CardTitle>
        <CardDescription>
          {QUEUE_KIND_WORDS[item.kind]}, {waitingFor(item, nowMs)}
          {item.run ? ` (run ${item.run})` : ''}
        </CardDescription>
      </CardHeader>
      <CardContent className="gap-4">
        {item.permission ? <PermissionBox item={item} /> : null}
        {item.cache ? (
          <p className="text-sm">
            Warm for {durationWords(item.cache.warm_secs_left * 1000)}
            {item.cache.resume_cost ? `, then $${item.cache.resume_cost.amount.toFixed(2)} to resume` : ''}.
          </p>
        ) : null}
        <div className="flex flex-wrap gap-2">
          {item.answers.map((answer) => (
            <Button key={answer} variant="outline" onClick={() => onAnswer(item.id, answer)}>
              {ANSWER_WORDS[answer]}
            </Button>
          ))}
        </div>
        <section aria-labelledby="context-heading" className="flex flex-col gap-2 border-t pt-4">
          <h3 id="context-heading" className="text-base font-medium">
            Why is this asked?
          </h3>
          <ContextView record={context ?? {}} />
        </section>
      </CardContent>
    </Card>
  )
}

/** The request as the agent sent it. Its command is code, so it is the one place monospace belongs here. */
function PermissionBox({ item }: { readonly item: QueueItem }) {
  const request = item.permission!
  return (
    <div className="flex flex-col gap-2 text-sm">
      <p className="flex items-center gap-2">
        {TOOL_KIND_WORDS[request.kind]}
        {item.untrusted_input ? <Badge variant="destructive">From untrusted text</Badge> : null}
      </p>
      {request.input === undefined || request.input === null ? (
        <p className="text-muted-foreground italic">input not available until approved</p>
      ) : (
        <pre className="overflow-x-auto rounded-md bg-muted p-2 font-mono text-xs">{JSON.stringify(request.input, null, 2)}</pre>
      )}
      {item.untrusted_input ? (
        <p className="text-muted-foreground">These arguments came from text Collide does not trust. Read them before you allow.</p>
      ) : null}
    </div>
  )
}
