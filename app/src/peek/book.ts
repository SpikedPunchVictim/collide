// The ids a board's text names, and what `task.get` found for each (ts-0189-fg; note 34 of the board mock). A word is
// asked for once, however often and in however many texts it is named; a book lasts while the board is shown, and a new
// one is made when the Field reads again, so a peek never shows a task as it was long ago. A word that no task has,
// that two tasks have (the daemon names no one), or that the daemon failed on, is plain text and is not asked again.
import type { Got } from '../drawer/types.ts'
import type { OwnerClient } from '../owner/client.ts'
import { candidateWords } from './ids.ts'

export class IdBook {
  private readonly settled = new Map<string, Got | null>()
  private readonly asked = new Map<string, Promise<void>>()
  private found: ReadonlySet<string> = new Set()
  private readonly listeners = new Set<() => void>()

  constructor(
    private readonly owner: OwnerClient,
    private readonly boardId: string,
    readonly patterns: readonly RegExp[],
  ) {}

  /** The words of `text` that could be ids and are worth asking about. */
  candidates(text: string): string[] {
    return candidateWords(text, this.patterns)
  }

  /** Asks `task.get` for each word not asked yet. Finishes when every one of `words` has an answer. */
  async ensure(words: readonly string[]): Promise<void> {
    await Promise.all(words.map((word) => this.ask(word)))
  }

  /** What `task.get` gave for a word: the task, null when none was found, undefined while that is not known. */
  get(word: string): Got | null | undefined {
    return this.settled.get(word)
  }

  /** The words that found a task: the same set until another is found, so a reader may compare by identity. */
  linked = (): ReadonlySet<string> => this.found

  /** Calls `listener` when a word has found a task. Gives the function that stops it. */
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener)
    return () => {
      this.listeners.delete(listener)
    }
  }

  private ask(word: string): Promise<void> {
    const before = this.asked.get(word)
    if (before !== undefined) return before
    const now = this.owner
      .rpc('task.get', { board: this.boardId, id: word })
      .then(
        (got) => (got.task === undefined || got.task === null ? null : got),
        () => null,
      )
      .then((got) => {
        this.settled.set(word, got)
        if (got === null) return
        this.found = new Set([...this.found, word])
        for (const listener of [...this.listeners]) listener()
      })
    this.asked.set(word, now)
    return now
  }
}
