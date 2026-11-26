// The window that adds a pack (ts-0193-gk; the owner took this at the split, and the mock shows "(Not designed
// yet.)"). Three steps over `config.pack_add`: a source box, then what the pack runs, as the daemon reviews it, then
// Add. Nothing is written until Add. A git source from a host Collide has not fetched from asks a yes for that host
// first. A refusal stays in the window in the daemon's words. It is the browser's own `<dialog>`, as `LocateWindow`
// is.
import { type FormEvent, useEffect, useId, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { WINDOW } from '../frame/LocateWindow.tsx'
import { type OwnerClient, toOwnerError } from '../owner/index.ts'
import { PackReview } from './PackReview.tsx'
import { type Pack, sourceOf } from './model.ts'

export interface AddPackWindowProps {
  readonly owner: OwnerClient
  /** A pack was added: the page reads its packs again. */
  readonly onAdded: () => void
  /** The window has closed: by Cancel, by Escape, or once the pack was added. */
  readonly onClosed: () => void
}

/** What the window shows: the box, a host that needs a yes, the review of a pack, or a note that ends it. */
type Step =
  | { readonly step: 'source' }
  | { readonly step: 'host'; readonly host: string }
  | { readonly step: 'review'; readonly pack: Pack }
  | { readonly step: 'note'; readonly note: string }

export function AddPackWindow({ owner, onAdded, onClosed }: AddPackWindowProps) {
  const dialog = useRef<HTMLDialogElement>(null)
  const titleId = useId()
  const inputId = useId()
  const [typed, setTyped] = useState('')
  const [step, setStep] = useState<Step>({ step: 'source' })
  const [approved, setApproved] = useState<readonly string[]>([])
  const [problem, setProblem] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    const element = dialog.current
    if (element !== null && !element.open) element.showModal()
  }, [])
  const shut = () => dialog.current?.close()
  const source = sourceOf(typed)

  /** Asks the daemon, with `confirm` to write, and with the hosts the person has said yes to. */
  const ask = async (confirm: boolean, hosts: readonly string[]) => {
    setBusy(true)
    setProblem(null)
    try {
      const result = await owner.rpc('config.pack_add', { source, confirm, approve_hosts: [...hosts] })
      if (result.needs_host != null) setStep({ step: 'host', host: result.needs_host })
      else if (result.written) {
        onAdded()
        shut()
      } else if (result.pack.review != null && !confirm) setStep({ step: 'review', pack: result.pack })
      else setStep({ step: 'note', note: result.note ?? 'Nothing was added.' })
    } catch (failure) {
      setProblem(toOwnerError(failure).message)
      setStep({ step: 'source' })
    } finally {
      setBusy(false)
    }
  }
  const look = (event: FormEvent) => {
    event.preventDefault()
    void ask(false, approved)
  }
  const allow = (host: string) => {
    const hosts = [...approved, host]
    setApproved(hosts)
    void ask(false, hosts)
  }
  return (
    <dialog ref={dialog} aria-labelledby={titleId} onClose={onClosed} className={WINDOW}>
      <div className="flex flex-col gap-3">
        <h2 id={titleId} className="text-base font-semibold">
          Add a pack
        </h2>
        {step.step === 'source' ? (
          <form onSubmit={look} className="flex flex-col gap-3">
            <label htmlFor={inputId}>Where is the pack?</label>
            <Input
              id={inputId}
              placeholder="A folder, such as /path/to/pack, or a git address"
              value={typed}
              onChange={(event) => setTyped(event.target.value)}
            />
            <p className="text-muted-foreground">
              Collide shows what the pack runs before it adds it. Nothing is written until you add it.
            </p>
            {problem === null ? null : (
              <p role="alert" className="whitespace-pre-wrap text-destructive">
                {problem}
              </p>
            )}
            <div className="flex justify-end gap-2">
              <Button type="button" variant="outline" onClick={shut}>
                Cancel
              </Button>
              <Button type="submit" disabled={busy || typed.trim() === ''}>
                Review the pack
              </Button>
            </div>
          </form>
        ) : null}
        {step.step === 'host' ? (
          <>
            <p>
              Collide has not fetched from <b>{step.host}</b> before. Fetching asks that host for the pack&apos;s
              files.
            </p>
            <div className="flex justify-end gap-2">
              <Button variant="outline" onClick={shut}>
                Cancel
              </Button>
              <Button disabled={busy} onClick={() => allow(step.host)}>
                {`Fetch from ${step.host}`}
              </Button>
            </div>
          </>
        ) : null}
        {step.step === 'review' && step.pack.review != null ? (
          <>
            <PackReview source={step.pack.source} dev={step.pack.dev} review={step.pack.review} />
            {problem === null ? null : (
              <p role="alert" className="whitespace-pre-wrap text-destructive">
                {problem}
              </p>
            )}
            <div className="flex justify-end gap-2">
              <Button variant="outline" onClick={shut}>
                Cancel
              </Button>
              <Button disabled={busy} onClick={() => void ask(true, approved)}>
                Add the pack
              </Button>
            </div>
          </>
        ) : null}
        {step.step === 'note' ? (
          <>
            <p>{step.note}</p>
            <div className="flex justify-end">
              <Button onClick={shut}>Close</Button>
            </div>
          </>
        ) : null}
      </div>
    </dialog>
  )
}
