// The trust prompt's state for the board in use (ts-0194-qp). When the board's default line is untrusted, the daemon's
// `board.trust_review` says what its Collide folder brings. If that is packs, views or settings, the prompt shows once
// for what the line holds now (its hash). Open without it keeps the board open with a banner, and the prompt comes back
// only if the line changes or the person asks. Trust and open sends the review's hash, so the person trusts what they
// were shown and nothing newer.
import { useCallback, useEffect, useRef, useState } from 'react'
import type { BoardSummary } from '../loop/types.ts'
import { type OwnerClient, toOwnerError } from '../owner/index.ts'
import { type Brought, type Review, broughtBy } from './trust.ts'

export interface TrustPrompt {
  readonly review: Review
  readonly brought: Brought
}

export interface TrustState {
  /** What the untrusted line brings, or null when it is trusted, brings nothing that waits, or is not read yet. */
  readonly prompt: TrustPrompt | null
  /** The window is open. */
  readonly open: boolean
  readonly show: () => void
  /** Open without it: the window closes, the banner stays. */
  readonly without: () => void
  /** Trusts what the window showed. Resolves with the daemon's words when it did not. */
  readonly trust: () => Promise<string | null>
}

/** The line to review: the board's default one, when it is untrusted. */
function untrustedLine(board: BoardSummary | undefined): { readonly name: string; readonly hash: string } | null {
  const line = board?.lines[0]
  if (board === undefined || board.missing === true || line === undefined) return null
  if (line.trust.state !== 'untrusted') return null
  return { name: line.ref_name, hash: line.trust.hash }
}

/** `trusted` runs after a yes, so the page reads the board and its views again. */
export function useTrust(owner: OwnerClient, board: BoardSummary | undefined, trusted: () => void): TrustState {
  const line = untrustedLine(board)
  const boardId = board?.id ?? null
  const lineName = line?.name ?? null
  const lineHash = line?.hash ?? null
  const [read, setRead] = useState<{ key: string; prompt: TrustPrompt | null } | null>(null)
  // The hash the person opened without, so the window does not come back for the same line.
  const declined = useRef<string | null>(null)
  const [open, setOpen] = useState(false)
  const key = boardId === null || lineName === null || lineHash === null ? null : `${boardId}\n${lineName}\n${lineHash}`
  useEffect(() => {
    // Another board or line has not been shown the window yet.
    setOpen(false)
    if (key === null || boardId === null || lineName === null) return
    let current = true
    owner.rpc('board.trust_review', { board: boardId, line: lineName }).then(
      (review) => {
        if (!current) return
        const brought = broughtBy(review)
        setRead({ key, prompt: brought === null ? null : { review, brought } })
        if (brought !== null && declined.current !== review.trust.hash) setOpen(true)
      },
      // A review that failed shows no prompt: the board opens with what is trusted, and the line's own problems show.
      () => current && setRead({ key, prompt: null }),
    )
    return () => {
      current = false
    }
  }, [owner, key, boardId, lineName])
  const prompt = key !== null && read?.key === key ? read.prompt : null
  const hash = prompt?.review.trust.hash ?? null
  const show = useCallback(() => setOpen(true), [])
  const without = useCallback(() => {
    setOpen(false)
    declined.current = hash
  }, [hash])
  const trust = useCallback(async (): Promise<string | null> => {
    if (boardId === null || lineName === null || prompt === null) return 'There is nothing to trust.'
    try {
      await owner.rpc('board.trust', { board: boardId, line: lineName, hash: prompt.review.trust.hash })
    } catch (failure) {
      return toOwnerError(failure).message
    }
    setOpen(false)
    trusted()
    return null
  }, [owner, boardId, lineName, prompt, trusted])
  return { prompt, open: open && prompt !== null, show, without, trust }
}
