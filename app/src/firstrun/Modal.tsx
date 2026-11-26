// The window of this folder's pages (ts-0194-qp): the browser's own `<dialog>`, shown modal, as `LocateWindow` and
// `AddPackWindow` are, so it holds the focus and closes on Escape. `onClosed` runs for every way it closes.
import { type ReactNode, useEffect, useId, useRef } from 'react'
import { WINDOW } from '../frame/LocateWindow.tsx'

export interface ModalProps {
  readonly title: string
  readonly onClosed: () => void
  /** The body, given the way to close the window. */
  readonly children: (shut: () => void) => ReactNode
}

export function Modal({ title, onClosed, children }: ModalProps) {
  const dialog = useRef<HTMLDialogElement>(null)
  const titleId = useId()
  useEffect(() => {
    const element = dialog.current
    if (element !== null && !element.open) element.showModal()
  }, [])
  const shut = () => dialog.current?.close()
  return (
    <dialog ref={dialog} aria-labelledby={titleId} onClose={onClosed} className={WINDOW}>
      <div className="flex flex-col gap-3">
        <h2 id={titleId} className="text-base font-semibold">
          {title}
        </h2>
        {children(shut)}
      </div>
    </dialog>
  )
}
