// The Copy button of a fix that is a command (ts-0194-qp): it puts the command on the clipboard and says so. A page
// that cannot write to the clipboard says that instead, and the command stays on screen to select.
import { useState } from 'react'
import { Button } from '@/components/ui/button'

export interface CopyButtonProps {
  readonly text: string
  /** What the clipboard is written with. The page's own, unless a test gives another. */
  readonly write?: (text: string) => Promise<void>
}

const clipboard = (text: string): Promise<void> => {
  if (typeof navigator === 'undefined' || navigator.clipboard === undefined) {
    return Promise.reject(new Error('this page has no clipboard'))
  }
  return navigator.clipboard.writeText(text)
}

export function CopyButton({ text, write = clipboard }: CopyButtonProps) {
  const [state, setState] = useState<'idle' | 'copied' | 'failed'>('idle')
  const copy = async () => {
    try {
      await write(text)
      setState('copied')
    } catch {
      setState('failed')
    }
  }
  return (
    <span className="flex items-center gap-2">
      <Button variant="outline" size="sm" onClick={() => void copy()}>
        {state === 'copied' ? 'Copied' : 'Copy'}
      </Button>
      {state === 'failed' ? (
        <span role="alert" className="text-xs text-destructive">
          Could not copy. Select the command instead.
        </span>
      ) : null}
    </span>
  )
}
