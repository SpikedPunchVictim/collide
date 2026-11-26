// The window that sets a secret (ts-0193-gk; the mock's `secretModal`). The value lives only in its password box.
// `onSave` gets it once, to send with `config.set`; the box is cleared whether the daemon took it or refused it, and
// nothing else holds it: no state, no store, no address, no log. A refusal shows in the daemon's words, which never
// echo it. The window is the browser's own `<dialog>`, as `LocateWindow` is, so it holds the focus and closes on
// Escape.
import { type FormEvent, useEffect, useId, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { WINDOW } from '../frame/LocateWindow.tsx'

export interface SecretWindowProps {
  /** What the secret is called, as the Secrets tab names it. */
  readonly name: string
  /** Sends the value. Gives the daemon's words when it was refused, or null when it was taken. */
  readonly onSave: (value: string) => Promise<string | null>
  /** The window has closed: by Cancel, by Escape, or once the secret was saved. */
  readonly onClosed: () => void
}

export function SecretWindow({ name, onSave, onClosed }: SecretWindowProps) {
  const dialog = useRef<HTMLDialogElement>(null)
  const box = useRef<HTMLInputElement>(null)
  const titleId = useId()
  const inputId = useId()
  // Only whether the box has something in it is state: it enables the button. The text is never read into state.
  const [filled, setFilled] = useState(false)
  const [problem, setProblem] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)
  useEffect(() => {
    const element = dialog.current
    if (element !== null && !element.open) element.showModal()
  }, [])
  const shut = () => dialog.current?.close()
  const submit = async (event: FormEvent) => {
    event.preventDefault()
    const input = box.current
    if (input === null || input.value === '') return
    setBusy(true)
    setProblem(null)
    try {
      const refused = await onSave(input.value)
      if (refused === null) shut()
      else setProblem(refused)
    } finally {
      input.value = ''
      setFilled(false)
      setBusy(false)
    }
  }
  return (
    <dialog ref={dialog} aria-labelledby={titleId} onClose={onClosed} className={WINDOW}>
      <form onSubmit={submit} className="flex flex-col gap-3">
        <h2 id={titleId} className="text-base font-semibold">{`Set ${name}`}</h2>
        <label htmlFor={inputId}>Value</label>
        <Input
          ref={box}
          id={inputId}
          type="password"
          autoComplete="off"
          placeholder="Paste it here"
          onChange={(event) => setFilled(event.target.value !== '')}
        />
        <p className="text-muted-foreground">It goes into your Mac&apos;s keychain. Collide will not show it again.</p>
        {problem === null ? null : (
          <p role="alert" className="whitespace-pre-wrap text-destructive">
            {problem}
          </p>
        )}
        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={shut}>
            Cancel
          </Button>
          <Button type="submit" disabled={busy || !filled}>
            Save to keychain
          </Button>
        </div>
      </form>
    </dialog>
  )
}
