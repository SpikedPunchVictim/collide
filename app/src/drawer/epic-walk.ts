// What the drawer reads for a picked epic (ts-0216-ve): the daemon's walk of the epic's closure, which is the steps of
// its open tasks and of what they wait on. It is `task.list` over an inline view of the whole board whose closure goal
// is the collection, read again whenever the Field's own read changed (`stamp`), as `useTaskGet` reads a task.
import { useEffect, useState } from 'react'
import { loadWalk } from '../field/load.ts'
import type { ViewReport } from '../field/types.ts'
import { inlineView } from '../field/walk.ts'
import type { OwnerClient } from '../owner/client.ts'

export interface EpicWalk {
  /** What the last read of this epic's walk gave. Null until the first read of it, and never another epic's. */
  readonly report: ViewReport | null
  /** The words of a walk the daemon refused. A walk that was read stays shown as it was. */
  readonly problem: string | null
}

interface Held extends EpicWalk {
  readonly boardId: string
  readonly goal: string
}

export function useEpicWalk(owner: OwnerClient, boardId: string, goal: string, stamp: number | null): EpicWalk {
  const [held, setHeld] = useState<Held | null>(null)
  useEffect(() => {
    let current = true
    void loadWalk(owner, boardId, inlineView(undefined, goal), null).then((walked) => {
      if (!current) return
      setHeld((before) => ({
        boardId,
        goal,
        report: walked.report ?? (before?.boardId === boardId && before.goal === goal ? before.report : null),
        problem: walked.problem,
      }))
    })
    return () => {
      current = false
    }
  }, [owner, boardId, goal, stamp])
  return held !== null && held.boardId === boardId && held.goal === goal
    ? { report: held.report, problem: held.problem }
    : { report: null, problem: null }
}
