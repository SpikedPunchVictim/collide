// Which words of a text could be the id of an item (ts-0189-fg; 0.11, Task drawer, "An id peeks"; note 34 of the board
// mock). Only the patterns the board's sources declare count (`id_pattern` and `short_id_pattern`, ts-0180-ad), so no
// shape is guessed. A word that matches is only a candidate: it becomes a link once `task.get` finds it (`book.ts`).

/**
 * The most words of one text that are looked up, so a text of numbers cannot ask the daemon about each of them. A
 * pattern for plain numbers matches every number of a long text, so the cap is far above what a task's text names.
 */
export const MAX_LOOKUPS = 200

/** What of a source says which words are its ids: the part of `SourceDecl` this reads. */
export interface PatternSource {
  readonly id_pattern?: string | null
  readonly short_id_pattern?: string | null
}

/** The patterns of the sources, compiled. One a source declares empty, or that is not a pattern, names no word. */
export function compilePatterns(sources: readonly PatternSource[]): RegExp[] {
  const patterns: RegExp[] = []
  for (const source of sources) {
    for (const text of [source.id_pattern, source.short_id_pattern]) {
      if (text === undefined || text === null || text === '') continue
      try {
        patterns.push(new RegExp(text))
      } catch {
        // A pattern this engine cannot read names no word.
      }
    }
  }
  return patterns
}

/** A run of letters, digits, `_` and `-`: a word as an id is written. */
const WORD = /[A-Za-z0-9][A-Za-z0-9_-]*/g

/** A word is not an id when it ends a path, an address or a fragment: the mock's rule for what comes before. */
const NOT_BEFORE = new Set(['/', '.', '#'])

function wordsOf(text: string, patterns: readonly RegExp[]): { word: string; at: number }[] {
  if (patterns.length === 0) return []
  const found: { word: string; at: number }[] = []
  for (const match of text.matchAll(WORD)) {
    const at = match.index
    if (at > 0 && NOT_BEFORE.has(text.charAt(at - 1))) continue
    if (patterns.some((pattern) => pattern.test(match[0]))) found.push({ word: match[0], at })
  }
  return found
}

/** The words of `text` that match a pattern, each once, in the order they come, at most `MAX_LOOKUPS`. */
export function candidateWords(text: string, patterns: readonly RegExp[]): string[] {
  return [...new Set(wordsOf(text, patterns).map((each) => each.word))].slice(0, MAX_LOOKUPS)
}

/** A piece of a text: plain text, or a word that links. */
export type Piece = string | { readonly id: string }

/** `text` cut at each word of `linked` that is also a candidate, so a text is drawn as plain pieces and id links. */
export function splitIds(text: string, linked: ReadonlySet<string>, patterns: readonly RegExp[]): Piece[] {
  const pieces: Piece[] = []
  let from = 0
  for (const { word, at } of wordsOf(text, patterns)) {
    if (!linked.has(word)) continue
    if (at > from) pieces.push(text.slice(from, at))
    pieces.push({ id: word })
    from = at + word.length
  }
  if (from < text.length) pieces.push(text.slice(from))
  return pieces.length === 0 ? [text] : pieces
}
