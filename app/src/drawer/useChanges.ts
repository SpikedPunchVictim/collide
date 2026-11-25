// What the Changes part reads (ts-0189-fg): a `ChangesFeed` for the task shown, while the part is open. What is held is
// keyed by board and task, so a task never shows another's changes while its own are on their way.
import { useEffect, useState } from 'react'
import type { OwnerClient } from '../owner/client.ts'
import { type ChangesRead, ChangesFeed } from './changes-feed.ts'

interface Held extends ChangesRead {
  readonly boardId: string
  readonly key: string
}

/**
 * The changes of a task, read again as the daemon says they may have changed. `enabled` is false while the part is
 * folded, or before the task is known to be one the daemon names: nothing is read or listened to then. Null until the
 * first read of this task.
 */
export function useChanges(owner: OwnerClient, boardId: string, key: string, enabled: boolean): ChangesRead | null {
  const [held, setHeld] = useState<Held | null>(null)
  useEffect(() => {
    if (!enabled) return
    const feed = new ChangesFeed({
      owner,
      boardId,
      task: key,
      onRead: (read) => setHeld({ boardId, key, ...read }),
    })
    void feed.start()
    return () => feed.stop()
  }, [owner, boardId, key, enabled])
  return enabled && held !== null && held.boardId === boardId && held.key === key
    ? { changes: held.changes, problem: held.problem }
    : null
}
