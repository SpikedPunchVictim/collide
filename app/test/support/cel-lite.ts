// Just enough CEL for the fake owner to answer the filter bar (ts-0190-tz): the terms `filters/model.ts` `celOf`
// writes, and-ed on lines, with `||` inside brackets. A term it does not know is an error, so a test never passes by a
// filter the fake world ignored. The real daemon's CEL is tested in `host/tests/daemon_corpus.rs` and the bridge specs.
import type { TaskOutput } from '../../src/owner/generated.ts'

/** Splits on `sep` where it is outside a string and outside brackets. */
function splitTop(text: string, sep: string): string[] {
  const parts: string[] = []
  let depth = 0
  let inString = false
  let from = 0
  for (let at = 0; at < text.length; at++) {
    const char = text[at]!
    if (inString) {
      if (char === '\\') at++
      else if (char === '"') inString = false
    } else if (char === '"') inString = true
    else if (char === '(' || char === '[') depth++
    else if (char === ')' || char === ']') depth--
    else if (depth === 0 && text.startsWith(sep, at)) {
      parts.push(text.slice(from, at))
      at += sep.length - 1
      from = at + 1
    }
  }
  parts.push(text.slice(from))
  return parts
}

/** The text inside one pair of brackets that wraps all of it, or null. */
function unwrapped(term: string): string | null {
  if (!term.startsWith('(') || !term.endsWith(')')) return null
  const inside = term.slice(1, -1)
  let depth = 0
  let inString = false
  for (let at = 0; at < inside.length; at++) {
    const char = inside[at]!
    if (inString) {
      if (char === '\\') at++
      else if (char === '"') inString = false
    } else if (char === '"') inString = true
    else if (char === '(') depth++
    else if (char === ')' && --depth < 0) return null
  }
  return inside
}

const sourceOf = (key: string) => key.slice(0, key.indexOf(':'))

function term(text: string, task: TaskOutput): boolean {
  const inside = unwrapped(text)
  if (inside !== null) return splitTop(inside, ' || ').some((each) => term(each.trim(), task))
  let found = /^(category|source) in (\[.*\])$/.exec(text)
  if (found !== null) {
    const values = JSON.parse(found[2]!) as string[]
    return values.includes(found[1] === 'category' ? task.state.category : sourceOf(task.key))
  }
  found = /^("(?:[^"\\]|\\.)*") in (collections|labels)$/.exec(text)
  if (found !== null) return (task[found[2] as 'collections' | 'labels'] as string[]).includes(JSON.parse(found[1]!))
  found = /^(title|key)\.matches\(("(?:[^"\\]|\\.)*")\)$/.exec(text)
  if (found !== null) {
    const pattern = (JSON.parse(found[2]!) as string).replace(/^\(\?i\)/, '')
    return new RegExp(pattern, 'i').test(found[1] === 'title' ? task.title : task.key)
  }
  throw new Error(`the fake world cannot evaluate: ${text}`)
}

/** Whether a task passes an expression, as the daemon would say. */
export function passes(expression: string, task: TaskOutput): boolean {
  return splitTop(expression, '\n  && ').every((each) => term(each.trim(), task))
}
