// The window that asks before a link opens (ts-0183-ee; plan 0.9, R5; the mock's `extModal`). `ExternalLinks` is
// provided once, at the top of the app. A link in a task's text (`Markdown`) asks it with `useAskToOpen`, and nothing
// opens until the person presses Open. Open calls the shell's own method, `shell.open_external`, which checks the
// scheme again and runs `/usr/bin/open`; Cancel opens nothing and makes no call. The window is the browser's own
// `<dialog>`, which holds the focus and closes on Escape with no library and no inline style. Cancel, Escape and an
// opened link all close it the same way, so the browser gives the focus back to the link each time.
import { type ReactNode, createContext, useCallback, useContext, useEffect, useId, useRef, useState } from 'react'
import { Button } from '@/components/ui/button'
import { type OwnerClient, toOwnerError } from '../owner/index.ts'

const AskToOpen = createContext<(url: string) => void>(() => {
  // Without a provider (a test that draws text alone) a link has nowhere to ask, and opens nothing.
})

/** The way for a link to ask the person. */
export const useAskToOpen = () => useContext(AskToOpen)

/** The host a link leads to, or null for an address with none (a mail address). */
function hostOf(url: string): string | null {
  try {
    return new URL(url).host || null
  } catch {
    return null
  }
}

/** The window's look: centred, with the page behind it dimmed. */
const WINDOW = [
  'm-auto w-[min(32rem,calc(100vw-2rem))] rounded-lg border bg-card p-5',
  'text-sm text-card-foreground shadow-lg backdrop:bg-black/40',
].join(' ')

export interface LinkWindowProps {
  readonly url: string
  /** Why the link could not be opened, when it could not. */
  readonly problem: string | null
  readonly opening: boolean
  /** Asks to open the link: true when it opened, and the window then closes. */
  readonly onOpen: () => Promise<boolean>
  /** The window has closed: by Cancel, by Escape, or once the link opened. */
  readonly onClosed: () => void
}

/** The window: the whole address, where it leads, and the two buttons. It is a function of its props. */
export function LinkWindow({ url, problem, opening, onOpen, onClosed }: LinkWindowProps) {
  const dialog = useRef<HTMLDialogElement>(null)
  const titleId = useId()
  const host = hostOf(url)
  useEffect(() => {
    const element = dialog.current
    if (element !== null && !element.open) element.showModal()
  }, [])
  // The dialog's own `close` is what Escape does, and it puts the focus back where it was before the window opened.
  // Taking the window away without it would leave the focus nowhere.
  const shut = () => dialog.current?.close()
  return (
    // Cancel is first, so it has the focus and Enter does not open.
    <dialog
      ref={dialog}
      aria-labelledby={titleId}
      onClose={onClosed}
      className={WINDOW}
    >
      <div className="flex flex-col gap-3">
        <h2 id={titleId} className="text-base font-semibold">
          Open this link in your browser?
        </h2>
        {host === null ? null : (
          <p>
            {'It leads to '}
            <b>{host}</b>
            {':'}
          </p>
        )}
        <p className="font-mono text-[13px] [overflow-wrap:anywhere]">{url}</p>
        <p className="text-muted-foreground">
          The link came from text written outside Collide. Collide does not open links by itself.
        </p>
        {problem === null ? null : (
          <p role="alert" className="text-destructive">
            {problem}
          </p>
        )}
        <div className="flex justify-end gap-2">
          <Button variant="outline" onClick={shut}>
            Cancel
          </Button>
          <Button
            disabled={opening}
            onClick={() => {
              void onOpen().then((opened) => {
                if (opened) shut()
              })
            }}
          >
            Open in browser
          </Button>
        </div>
      </div>
    </dialog>
  )
}

export interface ExternalLinksProps {
  /** The way to the shell, which opens the link. */
  readonly owner: OwnerClient
  readonly children: ReactNode
}

export function ExternalLinks({ owner, children }: ExternalLinksProps) {
  const [url, setUrl] = useState<string | null>(null)
  const [problem, setProblem] = useState<string | null>(null)
  const [opening, setOpening] = useState(false)
  const ask = useCallback((next: string) => {
    setProblem(null)
    setUrl(next)
  }, [])
  const closed = useCallback(() => {
    setUrl(null)
    setProblem(null)
  }, [])
  const open = async () => {
    if (url === null) return false
    setOpening(true)
    try {
      await owner.rpc('shell.open_external', { url })
      return true
    } catch (failure) {
      setProblem(`Collide could not open the link: ${toOwnerError(failure).message}`)
      return false
    } finally {
      setOpening(false)
    }
  }
  return (
    <AskToOpen value={ask}>
      {children}
      {url === null ? null : (
        <LinkWindow url={url} problem={problem} opening={opening} onOpen={open} onClosed={closed} />
      )}
    </AskToOpen>
  )
}
