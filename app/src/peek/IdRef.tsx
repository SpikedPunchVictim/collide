// A word in a task's text that is the id of a task (ts-0189-fg; note 34 of the board mock). It is a button, with no
// address, so the webview has nothing to navigate to: the pointer over it shows the peek, a click keeps the peek open.
import { type ComponentProps, useContext } from 'react'
import { InPeekContext, PeekActionsContext } from './context.ts'

/** The id is on the element as `data-id`: `Markdown` draws an id as `<collide-id data-id=...>`, and this answers it. */
export function IdRef(props: ComponentProps<'button'> & { 'data-id'?: string }) {
  const actions = useContext(PeekActionsContext)
  const inside = useContext(InPeekContext)
  const id = props['data-id'] ?? ''
  return (
    <button
      type="button"
      data-id={id}
      title="Peek at this task"
      className="id-ref"
      onMouseEnter={(event) => {
        if (!inside) actions.hover(id, event.currentTarget)
      }}
      onMouseLeave={() => {
        if (!inside) actions.leave()
      }}
      onClick={(event) => (inside ? actions.inner(id) : actions.pin(id, event.currentTarget))}
    >
      {props.children}
    </button>
  )
}
