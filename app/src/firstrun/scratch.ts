// Start with a task (ts-0194-qp): a scratch task made of the person's words, through the same `task.apply` the thin
// loop uses (`loop/loop.ts`). A scratch task stays on the machine until it is shared.
import { type OwnerClient, toOwnerError } from '../owner/index.ts'

/** Makes the task. Resolves with the daemon's words when it did not make one, and null when it did. */
export async function makeScratch(owner: OwnerClient, board: string, prompt: string): Promise<string | null> {
  const words = prompt.trim()
  if (words === '') return 'Say what needs doing first.'
  try {
    const { result, queued } = await owner.rpc('task.apply', {
      board,
      source: 'scratch',
      apply: { op: 'create', args: { prompt: words } },
    })
    if (result?.outcome === 'ok') return null
    if (result?.outcome === 'refused') return result.message
    return queued?.reason ?? 'Collide could not make the task.'
  } catch (failure) {
    return toOwnerError(failure).message
  }
}
