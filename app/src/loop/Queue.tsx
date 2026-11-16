// The one-item Queue of the thin loop (Stage 3c; 0.11, Queue): a permission request, with what the agent asks to do,
// and a run that is ready to review. An item of another kind is listed and not answered here (Stage 9a).
import { type FormEvent, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card'
import { Textarea } from '@/components/ui/textarea'
import { cn } from '@/lib/utils'
import { QUEUE_KIND_WORDS } from '../components/words.ts'
import type { DiffResult, QueueItem } from './types.ts'
import { INPUT_UNKNOWN_WORDS, REQUEST_WORDS, countWords, fileStatusWords } from './words.ts'

export interface QueueActions {
  readonly allowOnce: (item: string) => Promise<void>
  readonly deny: (item: string) => Promise<void>
  readonly accept: (item: string) => Promise<void>
  readonly reprompt: (item: string, words: string) => Promise<void>
  readonly discard: (item: string) => Promise<void>
  readonly keep: () => void
  readonly pickFile: (path: string | null) => Promise<void>
  readonly openItem: (item: string) => Promise<void>
}

export interface QueueProps {
  /** The item of the run the page follows, or null when nothing waits on that run. */
  readonly open: QueueItem | null
  readonly others: readonly QueueItem[]
  readonly diff: DiffResult | null
  readonly picked: string | null
  readonly patch: { readonly text: string; readonly truncated: boolean } | null
  readonly sending: boolean
  readonly confirmingDiscard: boolean
  readonly actions: QueueActions
}

const plural = (count: number, word: string) => (count === 1 ? word : `${word}s`)

export function Queue(props: QueueProps) {
  const { open, others } = props
  return (
    <section aria-labelledby="queue-heading" className="flex flex-col gap-3">
      <h2 id="queue-heading" className="text-lg font-semibold">
        Needs you
      </h2>
      {open === null && others.length === 0 ? (
        <p className="text-sm text-muted-foreground">Nothing is waiting on you.</p>
      ) : null}
      {open === null ? null : <Item {...props} item={open} />}
      {others.length === 0 ? null : (
        <div className="flex flex-col gap-2">
          <h3 className="text-sm font-medium text-muted-foreground">Also waiting</h3>
          <ul className="flex flex-col gap-2">
            {others.map((item) => (
              <li key={item.id} className="flex items-center justify-between gap-2 rounded-lg border p-3 text-sm">
                <span>
                  <span className="block font-medium">{item.title}</span>
                  <span className="block text-muted-foreground">{QUEUE_KIND_WORDS[item.kind]}</span>
                </span>
                {item.run == null ? null : (
                  <Button variant="outline" size="sm" onClick={() => void props.actions.openItem(item.id)}>
                    Open
                  </Button>
                )}
              </li>
            ))}
          </ul>
        </div>
      )}
    </section>
  )
}

function Item(props: QueueProps & { readonly item: QueueItem }) {
  const { item } = props
  const takes = (kind: QueueItem['answers'][number]) => item.answers.includes(kind)
  if (item.kind === 'permission' && item.permission != null && takes('permission')) {
    return <PermissionItem {...props} />
  }
  if (item.kind === 'review') return <ReviewItem {...props} />
  return (
    <Card>
      <CardHeader>
        <CardTitle>{item.title}</CardTitle>
        <CardDescription>{QUEUE_KIND_WORDS[item.kind]}</CardDescription>
      </CardHeader>
      <CardContent className="text-sm">This page cannot answer this one yet.</CardContent>
    </Card>
  )
}

function PermissionItem({ item, sending, actions }: QueueProps & { readonly item: QueueItem }) {
  const request = item.permission!
  const input = request.input
  return (
    <Card>
      <CardHeader>
        <CardTitle>{REQUEST_WORDS[request.kind]}</CardTitle>
        <CardDescription>{item.title}</CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-3 text-sm">
        {request.tool == null ? null : <p>{`Tool: ${request.tool}`}</p>}
        {request.reason == null ? null : <p>{request.reason}</p>}
        <div>
          <h3 className="font-medium">What it would use</h3>
          {input == null ? (
            <p className="text-muted-foreground">{INPUT_UNKNOWN_WORDS}</p>
          ) : (
            <pre className="mt-1 overflow-x-auto rounded-md bg-muted p-2 font-mono text-xs">
              {typeof input === 'string' ? input : JSON.stringify(input, null, 2)}
            </pre>
          )}
          {item.untrusted_input ? (
            <p className="mt-1 text-destructive">
              This input came from text Collide does not trust. Read it before you answer.
            </p>
          ) : null}
        </div>
        <div className="flex flex-wrap gap-2">
          <Button disabled={sending} onClick={() => void actions.allowOnce(item.id)}>
            Allow once
          </Button>
          <Button variant="outline" disabled={sending} onClick={() => void actions.deny(item.id)}>
            Deny
          </Button>
        </div>
      </CardContent>
    </Card>
  )
}

function ReviewItem(props: QueueProps & { readonly item: QueueItem }) {
  const { item, diff, picked, patch, sending, confirmingDiscard, actions } = props
  const [words, setWords] = useState('')
  const takes = (kind: QueueItem['answers'][number]) => item.answers.includes(kind)
  const changed = diff === null ? 0 : diff.files.length + diff.more
  const sendBack = (event: FormEvent) => {
    event.preventDefault()
    void actions.reprompt(item.id, words).then(() => setWords(''))
  }
  return (
    <Card>
      <CardHeader>
        <CardTitle>{item.title}</CardTitle>
        <CardDescription>
          Look at what changed, then accept it, send it back with new words, or discard it.
        </CardDescription>
      </CardHeader>
      <CardContent className="flex flex-col gap-4 text-sm">
        {diff === null ? null : (
          <div className="flex flex-col gap-2">
            <h3 className="font-medium">What changed</h3>
            {diff.files.length === 0 ? <p className="text-muted-foreground">No files changed.</p> : null}
            <ul aria-label="Changed files" className="flex flex-col gap-1">
              {diff.files.map((file) => (
                <li key={file.path}>
                  <button
                    type="button"
                    aria-pressed={picked === file.path}
                    onClick={() => void actions.pickFile(picked === file.path ? null : file.path)}
                    className={cn(
                      'flex w-full flex-wrap justify-between gap-2 rounded-md border px-3 py-2 text-left',
                      'hover:bg-muted',
                      picked === file.path && 'bg-muted ring-1 ring-ring',
                    )}
                  >
                    <span className="font-medium">{file.path}</span>
                    <span className="text-muted-foreground">
                      {`${fileStatusWords(file.status)}, ${countWords(file)}`}
                    </span>
                  </button>
                </li>
              ))}
            </ul>
            {diff.more > 0 ? (
              <p className="text-muted-foreground">{`${diff.more} more ${plural(diff.more, 'file')} not listed.`}</p>
            ) : null}
            {diff.collapsed.map((folder) => (
              <p key={folder.path} className="text-muted-foreground">
                {`${folder.path} holds ${folder.files} files, which are not listed.`}
              </p>
            ))}
            {patch === null ? null : (
              <div>
                <pre className="overflow-x-auto rounded-md bg-muted p-2 font-mono text-xs">{patch.text}</pre>
                {patch.truncated ? <p className="text-muted-foreground">This patch was cut short.</p> : null}
              </div>
            )}
          </div>
        )}
        <div className="flex flex-wrap gap-2">
          {takes('accept') ? (
            <Button disabled={sending} onClick={() => void actions.accept(item.id)}>
              Accept
            </Button>
          ) : null}
          {takes('discard') && !confirmingDiscard ? (
            <Button variant="outline" disabled={sending} onClick={() => void actions.discard(item.id)}>
              Discard
            </Button>
          ) : null}
        </div>
        {takes('discard') && confirmingDiscard ? (
          <div
            role="group"
            aria-label="Confirm discard"
            className="flex flex-col gap-2 rounded-md border border-destructive/40 p-3"
          >
            <p>
              <strong>Delete this work?</strong>
            </p>
            <p>
              {`This deletes the agent's copy of the project and its branch, which hold ${changed} changed ` +
                `${plural(changed, 'file')}. Your own checkout is not touched. This cannot be undone.`}
            </p>
            <div className="flex flex-wrap gap-2">
              <Button variant="destructive" disabled={sending} onClick={() => void actions.discard(item.id)}>
                Yes, delete this work
              </Button>
              <Button variant="outline" onClick={() => actions.keep()}>
                Keep it
              </Button>
            </div>
          </div>
        ) : null}
        {takes('reprompt') ? (
          <form onSubmit={sendBack} className="flex flex-col gap-2">
            <Textarea
              aria-label="What should change?"
              placeholder="Say what to change. The agent carries on from where it stopped."
              value={words}
              onChange={(e) => setWords(e.target.value)}
            />
            <div>
              <Button type="submit" variant="outline" disabled={sending || words.trim() === ''}>
                Send back
              </Button>
            </div>
          </form>
        ) : null}
      </CardContent>
    </Card>
  )
}
