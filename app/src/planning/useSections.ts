// What the Planning tab reads besides the Field: `task.sections`, read again whenever the Field's own read changed
// (`stamp`), as the drawer reads its task, so the tab follows the files with no second subscription (ts-0195-ju).
import { useEffect, useState } from 'react'
import type { OwnerClient } from '../owner/client.ts'
import type { Sections } from './model.ts'

interface Held {
  readonly boardId: string
  readonly sections: Sections
}

/** The sections of the board, or null before the first read, when it failed, or for another board's. */
export function useSections(owner: OwnerClient, boardId: string | null, stamp: number | null): Sections | null {
  const [held, setHeld] = useState<Held | null>(null)
  useEffect(() => {
    if (boardId === null) return
    let current = true
    owner.rpc('task.sections', { board: boardId }).then(
      (sections) => current && setHeld({ boardId, sections }),
      // A read that failed leaves what was read; the tab then names the features without the daemon's order.
      () => undefined,
    )
    return () => {
      current = false
    }
  }, [owner, boardId, stamp])
  return held !== null && held.boardId === boardId ? held.sections : null
}
