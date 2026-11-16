// The prompt box: a prompt makes a scratch task and starts a run on it (Stage 3c, from Stage 9a).
import { type FormEvent, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Textarea } from '@/components/ui/textarea'

export interface PromptBoxProps {
  /** Whether there is a repository to send it to. */
  readonly ready: boolean
  readonly sending: boolean
  /** Starts the run. True when it started, so the box can empty. */
  readonly onStart: (prompt: string) => Promise<boolean>
}

export function PromptBox({ ready, sending, onStart }: PromptBoxProps) {
  const [prompt, setPrompt] = useState('')
  const submit = (event: FormEvent) => {
    event.preventDefault()
    void onStart(prompt).then((started) => {
      if (started) setPrompt('')
    })
  }
  return (
    <section aria-labelledby="prompt-heading" className="flex flex-col gap-2">
      <h2 id="prompt-heading" className="text-lg font-semibold">
        What do you want done?
      </h2>
      <form onSubmit={submit} className="flex flex-col gap-2">
        <Textarea
          aria-label="What should the agent do?"
          placeholder="Describe the change in a sentence or two."
          value={prompt}
          onChange={(e) => setPrompt(e.target.value)}
        />
        <div>
          <Button type="submit" disabled={!ready || sending || prompt.trim() === ''}>
            Start
          </Button>
        </div>
      </form>
    </section>
  )
}
