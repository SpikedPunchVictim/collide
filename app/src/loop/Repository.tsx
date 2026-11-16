// The repository picker (ts-0081-wm): the boards the daemon knows, from `board.list`, and a box to type a folder. The
// window has no folder dialog, because the shell's capabilities are `rpc` and one event channel (Stage 6a).
import { type FormEvent, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { NativeSelect } from '@/components/ui/native-select'
import type { BoardSummary } from './types.ts'

export interface RepositoryProps {
  readonly boards: readonly BoardSummary[]
  readonly boardId: string | null
  /** Why the last folder was not opened. */
  readonly problems: readonly string[]
  readonly onChoose: (id: string) => void
  readonly onOpenFolder: (path: string) => Promise<void>
}

export function Repository({ boards, boardId, problems, onChoose, onOpenFolder }: RepositoryProps) {
  const [path, setPath] = useState('')
  const submit = (event: FormEvent) => {
    event.preventDefault()
    void onOpenFolder(path).then(() => setPath(''))
  }
  return (
    <section aria-labelledby="repository-heading" className="flex flex-col gap-2">
      <h2 id="repository-heading" className="text-lg font-semibold">
        Repository
      </h2>
      {boards.length === 0 ? (
        <p className="text-sm text-muted-foreground">
          Choose a repository: type the folder of a git checkout, and open it.
        </p>
      ) : (
        <NativeSelect aria-label="Repository" value={boardId ?? ''} onChange={(e) => onChoose(e.target.value)}>
          {boards.map((board) => (
            <option key={board.id} value={board.id}>
              {board.root}
            </option>
          ))}
        </NativeSelect>
      )}
      <form onSubmit={submit} className="flex gap-2">
        <Input
          aria-label="Folder path"
          placeholder="/path/to/a/git/checkout"
          value={path}
          onChange={(e) => setPath(e.target.value)}
        />
        <Button type="submit" variant="outline" disabled={path.trim() === ''}>
          Open folder
        </Button>
      </form>
      {problems.map((problem) => (
        <p key={problem} role="alert" className="text-sm text-destructive">
          {problem}
        </p>
      ))}
    </section>
  )
}
