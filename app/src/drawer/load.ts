// What the drawer reads from the owner (ts-0184-hv): `task.get` for the one task, read again whenever the Field's own
// read changed (`stamp`), so the drawer follows the files as the board does, with no second subscription.
import { useEffect, useState } from 'react'
import type { OwnerClient } from '../owner/client.ts'
import type { Got } from './types.ts'

export interface TaskGet {
  /** What the last read of this task gave. Null until the first read of it, and never another task's. */
  readonly got: Got | null
  /** The words of the last read if it failed. A task that was read stays shown as it was. */
  readonly problem: string | null
}

interface Held extends TaskGet {
  readonly boardId: string
  readonly key: string
}

const words = (failure: unknown) => (failure instanceof Error ? failure.message : String(failure))

export function useTaskGet(owner: OwnerClient, boardId: string, key: string, stamp: number | null): TaskGet {
  const [held, setHeld] = useState<Held | null>(null)
  useEffect(() => {
    let current = true
    owner.rpc('task.get', { board: boardId, id: key }).then(
      (got) => current && setHeld({ boardId, key, got, problem: null }),
      (failure) =>
        current &&
        setHeld((before) => ({
          boardId,
          key,
          got: before?.boardId === boardId && before.key === key ? before.got : null,
          problem: words(failure),
        })),
    )
    return () => {
      current = false
    }
  }, [owner, boardId, key, stamp])
  return held !== null && held.boardId === boardId && held.key === key
    ? { got: held.got, problem: held.problem }
    : { got: null, problem: null }
}
