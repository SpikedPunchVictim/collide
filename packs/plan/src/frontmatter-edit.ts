/**
 * Line-level edits of an item's frontmatter text (the lines between the two `---` lines). Each function changes the
 * lines of one key and returns every other line, its spacing and its line ending as it was; nothing is parsed and
 * re-written, so a comment or a quote style survives. A line in a form the function does not know is `refused` with
 * the line quoted, never reformatted. The caller parses the result and checks that only the intended field changed.
 */
export type Edit = { ok: true; text: string } | { ok: false; reason: string }

const refuse = (reason: string): Edit => ({ ok: false, reason })

/** The lines of `text`, each with its own line ending. */
const splitLines = (text: string): string[] => text.match(/[^\n]*\n|[^\n]+$/g) ?? []

const withoutEnding = (line: string): string => line.replace(/\r?\n$/, '')
const endingOf = (line: string): string => (line.endsWith('\r\n') ? '\r\n' : line.endsWith('\n') ? '\n' : '')

/** The index of the one line that starts the top-level key, -1 when there is none, or a refusal when there are several. */
function find(all: string[], key: string): number | Edit {
  const at = all.flatMap((line, index) => (line.startsWith(`${key}:`) ? [index] : []))
  return at.length > 1 ? refuse(`the frontmatter has "${key}:" on ${at.length} lines`) : (at[0] ?? -1)
}

const unknownForm = (key: string, line: string, expected: string): Edit => refuse(`the "${key}" line is not in a form the provider edits (${expected}): ${withoutEnding(line)}`)

const join = (all: string[]): Edit => ({ ok: true, text: all.join('') })

const STATUS = /^status:([ \t]+)(?:"([a-z]+)"|'([a-z]+)'|([a-z]+))([ \t]*(?:#.*)?)$/

/** Changes the value on the `status:` line and nothing else on it. */
export function setStatusLine(text: string, category: string): Edit {
  const all = splitLines(text)
  const at = find(all, 'status')
  if (typeof at !== 'number') return at
  if (at === -1) return refuse('the frontmatter has no "status:" line')
  const line = all[at] as string
  const match = STATUS.exec(withoutEnding(line))
  if (match === null) return unknownForm('status', line, 'status: ready, with the value on the same line')
  const quote = match[2] !== undefined ? '"' : match[3] !== undefined ? "'" : ''
  all[at] = `status:${match[1]}${quote}${category}${quote}${match[5]}${endingOf(line)}`
  return join(all)
}

const OWNERS_FLOW = /^owners:([ \t]*)\[([^\]#"'\\]*)\]([ \t]*(?:#.*)?)$/
const OWNERS_BLOCK = /^owners:[ \t]*(#.*)?$/
const BLOCK_ITEM = /^[ \t]*-[ \t]/

/** YAML 1.2 core reads these as something other than text. */
const NOT_TEXT = /^(true|false|null)$/i

/** A claimant as it is written in a flow list: bare when that is plainly text, else double-quoted. */
const owner = (name: string): string => (/^[A-Za-z][A-Za-z0-9._-]*$/.test(name) && !NOT_TEXT.test(name) ? name : JSON.stringify(name))

/** Empties the owners: `owners: [a, b]` or a block list becomes `owners: []`. An item with no `owners:` line has none to clear. */
export function clearOwnersLine(text: string): Edit {
  const all = splitLines(text)
  const at = find(all, 'owners')
  if (typeof at !== 'number') return at
  if (at === -1) return join(all)
  const line = all[at] as string
  const flow = OWNERS_FLOW.exec(withoutEnding(line))
  if (flow !== null) {
    all[at] = `owners:${flow[1]}[]${flow[3]}${endingOf(line)}`
    return join(all)
  }
  const block = OWNERS_BLOCK.exec(withoutEnding(line))
  if (block === null) return unknownForm('owners', line, 'owners: [a, b], or a list of "- a" lines')
  let end = at + 1
  while (end < all.length && BLOCK_ITEM.test(all[end] as string)) end++
  all.splice(at, end - at, `owners: []${block[1] === undefined ? '' : ` ${block[1]}`}${endingOf(line)}`)
  return join(all)
}

/** Puts `name` on an empty owners list: `owners: []` becomes `owners: [name]`. */
export function setOwnerLine(text: string, name: string): Edit {
  const all = splitLines(text)
  const at = find(all, 'owners')
  if (typeof at !== 'number') return at
  if (at === -1) return refuse('the frontmatter has no "owners:" line to put the claimant on')
  const line = all[at] as string
  const flow = OWNERS_FLOW.exec(withoutEnding(line))
  if (flow === null || flow[2]?.trim() !== '') return unknownForm('owners', line, 'owners: []')
  all[at] = `owners:${flow[1]}[${owner(name)}]${flow[3]}${endingOf(line)}`
  return join(all)
}

const LINKS_EMPTY = /^links:([ \t]*)\[[ \t]*\]([ \t]*(?:#.*)?)$/
const LINKS_BLOCK = /^links:[ \t]*(#.*)?$/
const LINK_ENTRY = /^([ \t]*)-[ \t]+\{[^\n]*\}[ \t]*(#.*)?$/
const SKIPPED = /^[ \t]*(#.*)?$/

/**
 * Adds `- { type: blocked-by, to: <to> }` to the `links` list. An empty `links: []` becomes `links:` with the one
 * entry under it. A list of one-line `- { ... }` entries gets the new entry after the last one, indented like it.
 * Any other form (a list in `[...]` with entries, entries over several lines) is refused.
 */
export function addBlockedByLine(text: string, to: string): Edit {
  const all = splitLines(text)
  const at = find(all, 'links')
  if (typeof at !== 'number') return at
  if (at === -1) return refuse('the frontmatter has no "links:" line to add the link to')
  const line = all[at] as string
  const eol = endingOf(line) || '\n'
  const entry = (indent: string): string => `${indent}- { type: blocked-by, to: ${to} }${eol}`

  const empty = LINKS_EMPTY.exec(withoutEnding(line))
  if (empty !== null) {
    all.splice(at, 1, `links:${empty[2]}${endingOf(line)}`, entry('  '))
    return join(all)
  }
  if (LINKS_BLOCK.exec(withoutEnding(line)) === null) return unknownForm('links', line, 'links: [], or a list of "- { type: blocked-by, to: ts-0001-aa }" lines')

  let last = at
  let indent = '  '
  for (let index = at + 1; index < all.length; index++) {
    const next = withoutEnding(all[index] as string)
    if (SKIPPED.test(next)) continue
    const match = LINK_ENTRY.exec(next)
    if (match === null) {
      if (/^[ \t]/.test(next) || next.startsWith('-')) return unknownForm('links', all[index] as string, 'every entry on one line, as "- { type: blocked-by, to: ts-0001-aa }"')
      break
    }
    last = index
    indent = match[1] as string
  }
  if (last === at) return unknownForm('links', line, 'a list with at least one entry under it')
  all.splice(last + 1, 0, entry(indent))
  return join(all)
}

const ID_LINE = /^id:([ \t]+)(?:"([a-z0-9-]+)"|'([a-z0-9-]+)'|([a-z0-9-]+))([ \t]*(?:#.*)?)$/

/** Changes the value on the `id:` line and nothing else on it. */
export function setIdLine(text: string, id: string): Edit {
  const all = splitLines(text)
  const at = find(all, 'id')
  if (typeof at !== 'number') return at
  if (at === -1) return refuse('the frontmatter has no "id:" line')
  const line = all[at] as string
  const match = ID_LINE.exec(withoutEnding(line))
  if (match === null) return unknownForm('id', line, 'id: ts-0012-ua, with the value on the same line')
  const quote = match[2] !== undefined ? '"' : match[3] !== undefined ? "'" : ''
  all[at] = `id:${match[1]}${quote}${id}${quote}${match[5]}${endingOf(line)}`
  return join(all)
}

/** A top-level key starts in the first column; a list item, an indented line and a comment belong to the key above. */
const TOP_LEVEL_KEY = /^[^\s#-]/

/**
 * Changes every `to: <from>` of the `links` lines to `to: <to>`: the target of a link, in a one-line entry, a block
 * mapping or a flow list, quoted or not. Nothing else on those lines changes. A `links` key with no such target is
 * refused. The caller parses the result and checks that only the targets changed, which catches a link in a form
 * this does not match.
 */
export function replaceLinkTarget(text: string, from: string, to: string): Edit {
  const all = splitLines(text)
  const at = find(all, 'links')
  if (typeof at !== 'number') return at
  if (at === -1) return refuse('the frontmatter has no "links:" line')
  let end = at + 1
  while (end < all.length && !TOP_LEVEL_KEY.test(all[end] as string)) end++
  const target = new RegExp(`(\\bto:[ \\t]*["']?)${from}(?=["']?(?:[ \\t]*[},#]|[ \\t]*\\r?\\n?$))`, 'g')
  let changed = 0
  for (let index = at; index < end; index++) {
    const line = all[index] as string
    const edited = line.replace(target, (_whole, lead: string) => {
      changed++
      return `${lead}${to}`
    })
    all[index] = edited
  }
  if (changed === 0) return refuse(`no link on the "links" lines has "to: ${from}" in a form the provider edits`)
  return join(all)
}
