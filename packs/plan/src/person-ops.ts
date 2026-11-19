import { chain, setFieldLine, setStatusLine } from './frontmatter-edit.ts'
import { parseFeatures } from './features.ts'
import { ID_PATTERN, INCLUDES, PROPOSAL, QUESTION } from './item.ts'
import { article, CONTROL, onlyKeys, stringArg } from './op-util.ts'
import type { Frontmatter, OpContext, OpSpec, Prepared } from './op-util.ts'
import type { FoundItem, Scan } from './scan.ts'
import { moveRefusal } from './transitions.ts'

/**
 * The ops that only a person makes (0.10, Planning sources), and the move of a proposal that `setStatus` and they
 * share. Who may make them is `personOnlyWrite` in `transitions.ts`; what a move writes is here.
 */
export const ACCEPT = 'accept'
export const DEFER = 'defer'
export const REJECT = 'reject'
export const RESOLVE = 'resolve'

/** The longest trigger or reason, in characters. */
export const NOTE_MAX = 200
/** The longest decision text, in characters. */
export const DECISION_MAX = 10_000

const idRegex = new RegExp(ID_PATTERN)
const DATE = /^[0-9]{4}-[0-9]{2}-[0-9]{2}$/
/** A control character other than a tab and a line break, which a decision may hold. */
const OTHER_CONTROL = /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f]/

const refuse = (message: string): Prepared => ({ kind: 'refuse', message })

/** The refusal for a move to `proposed` while the Features part has problems: every problem, with its line. */
function proposedRefusal(found: FoundItem): string | null {
  const { problems } = parseFeatures(found.body, found.bodyLine)
  if (problems.length === 0) return null
  const count = `${problems.length} problem${problems.length === 1 ? '' : 's'}`
  const lines = problems.map((problem) => `  line ${problem.line}: ${problem.reason}`)
  const fix = problems.length === 1 ? 'it' : 'them'
  const head = `${found.item.id} cannot move to proposed: its ## Features part in ${found.path} has ${count}`
  return `${head}; fix ${fix} first:\n${lines.join('\n')}`
}

/** `human:<date>` with the day of `actor.at`, or why there is none: the daemon sets it, and no one else's counts. */
function humanBy(op: string, id: string, context: OpContext): { by: string } | { problem: string } {
  const at = context.asker?.at
  if (typeof at === 'string' && DATE.test(at)) return { by: `human:${at}` }
  const said = `${op} ${id} writes who and when, and the day is actor.at (YYYY-MM-DD), which the daemon sets`
  return { problem: `${said}; there is none` }
}

/** The open questions that `found` includes, as `id (title)` each. */
function openQuestions(found: FoundItem, scan: Scan): string[] {
  const included = found.item.links.filter((link) => link.type === INCLUDES).map((link) => link.to)
  return scan.items
    .filter((other) => included.includes(other.item.id) && other.item.kind === QUESTION)
    .filter((other) => other.item.status === 'open')
    .map((other) => `${other.item.id} (${other.item.title})`)
}

/** A field to write with the status: its name, its text in YAML, and the value the file must read back as. */
interface Field {
  key: string
  yaml: string
  value: string
}

const quoted = (key: string, value: string): Field => ({ key, yaml: JSON.stringify(value), value })

const acceptRefusal = (id: string, open: string[]): string => {
  const one = open.length === 1
  const who = one ? 'a question it includes is' : 'questions it includes are'
  return `${id} cannot be accepted while ${who} open: ${open.join(', ')}; resolve or drop ${one ? 'it' : 'them'} first`
}

/**
 * Moves a proposal to `to`, writing `fields` with the status. The same table and the same rules answer `setStatus` and
 * the ops: a move the table does not have, a move to `proposed` with Feature problems, and an `accepted` while a
 * question the proposal includes is open. Accepting writes `by:`. Moving to the status it has changes nothing.
 */
export function proposalMove(
  found: FoundItem,
  scan: Scan,
  frontmatter: Frontmatter,
  to: string,
  context: OpContext,
  fields: Field[] = [],
): Prepared {
  const { id, status: from } = found.item
  if (from === to) return { kind: 'nothing' }
  const move = moveRefusal(id, from, to)
  if (move !== null) return refuse(move)
  if (to === 'proposed') {
    const message = proposedRefusal(found)
    if (message !== null) return refuse(message)
  }
  const written = [...fields]
  if (to === 'accepted') {
    const open = openQuestions(found, scan)
    if (open.length > 0) return refuse(acceptRefusal(id, open))
    const by = humanBy(ACCEPT, id, context)
    if ('problem' in by) return refuse(by.problem)
    written.unshift({ key: 'by', yaml: by.by, value: by.by })
  }
  const steps = written.map((field) => (text: string) => setFieldLine(text, field.key, field.yaml))
  return {
    kind: 'edit',
    edit: (text) => chain(text, (t) => setStatusLine(t, to), ...steps),
    expected: { ...frontmatter, status: to, ...Object.fromEntries(written.map((field) => [field.key, field.value])) },
  }
}

/** The problem with a trigger or a reason: 1 to `NOTE_MAX` characters, no control character, no edge space. */
function noteProblem(name: string, value: unknown): string | null {
  const text = typeof value === 'string' ? value : null
  const bad = text === null || text === '' || text.trim() !== text || text.length > NOTE_MAX || CONTROL.test(text)
  if (!bad) return null
  const form = `1 to ${NOTE_MAX} characters, with no line break or other control character and no space at either end`
  return `${name} must be text of ${form}`
}

export const forProposal = (op: string, found: FoundItem): Prepared | null => {
  const { id, kind } = found.item
  return kind === PROPOSAL ? null : refuse(`${id} is ${article(kind)}; ${op} is for a proposal`)
}

/** Makes the `## Decision` section hold `text`: at the end of the body when missing, after what it has otherwise. */
export function withDecision(body: string, text: string, eol: string): string {
  const entry = text.split(/\r?\n/).join(eol)
  const lines = body.match(/[^\n]*\n|[^\n]+$/g) ?? []
  const at = lines.findIndex((line) => /^## Decision[ \t]*\r?\n?$/.test(line))
  if (at === -1) {
    const kept = body.replace(/\s+$/, '')
    const lead = /^(?:[ \t]*\r?\n)+/.exec(body)?.[0] ?? eol
    return `${kept === '' ? lead : `${kept}${eol}${eol}`}## Decision${eol}${eol}${entry}${eol}`
  }
  let end = at + 1
  while (end < lines.length && !/^#{1,2}[ \t]/.test(lines[end] as string)) end++
  const held = lines
    .slice(at + 1, end)
    .join('')
    .replace(/^(?:[ \t]*\r?\n)+/, '')
    .replace(/\s+$/, '')
  const heading = `${(lines[at] as string).replace(/\r?\n?$/, '')}${eol}`
  const rest = lines.slice(end).join('')
  const before = lines.slice(0, at).join('')
  const kept = held === '' ? '' : `${held}${eol}${eol}`
  return `${before}${heading}${eol}${kept}${entry}${eol}${rest === '' ? '' : `${eol}${rest}`}`
}

/** The problem with `resolve`'s arguments, or null. */
function resolveProblem(args: Record<string, unknown>): string | null {
  const text = stringArg(args, 'text')
  if (text === null || text.trim() === '' || text.length > DECISION_MAX || OTHER_CONTROL.test(text)) {
    const form = `1 to ${DECISION_MAX} characters, with no control character but a tab and a line break`
    return `text must be the decision, ${form}`
  }
  if (args.by !== undefined && !(typeof args.by === 'string' && idRegex.test(args.by))) {
    return `by must be the id of the item that settled the question, matching ${ID_PATTERN}`
  }
  return onlyKeys(args, ['by', 'text'])
}

export const PERSON_OPS: Record<string, OpSpec> = {
  [ACCEPT]: {
    check: (args) => onlyKeys(args, []),
    prepare: (found, scan, _args, frontmatter, context) =>
      forProposal(ACCEPT, found) ?? proposalMove(found, scan, frontmatter, 'accepted', context),
  },
  [DEFER]: {
    check: (args) => noteProblem('trigger', args.trigger) ?? onlyKeys(args, ['trigger']),
    prepare: (found, scan, args, frontmatter, context) =>
      forProposal(DEFER, found) ??
      proposalMove(found, scan, frontmatter, 'deferred', context, [quoted('trigger', args.trigger as string)]),
  },
  [REJECT]: {
    check: (args) =>
      (args.reason === undefined ? null : noteProblem('reason', args.reason)) ?? onlyKeys(args, ['reason']),
    prepare: (found, scan, args, frontmatter, context) => {
      const fields = args.reason === undefined ? [] : [quoted('reason', args.reason as string)]
      return forProposal(REJECT, found) ?? proposalMove(found, scan, frontmatter, 'rejected', context, fields)
    },
  },
  [RESOLVE]: {
    check: resolveProblem,
    prepare: (found, scan, args, frontmatter, context) => {
      const { id, kind, status, fields } = found.item
      if (kind !== QUESTION) return refuse(`${id} is ${article(kind)}; resolve is for a question`)
      if (status === 'resolved') {
        return refuse(`${id} is already resolved, by ${String(fields.resolved_by)}; reopen it by hand`)
      }
      if (status === 'dropped') return refuse(`${id} is dropped; set it to open first, then resolve it`)
      let by: string
      if (args.by === undefined) {
        const person = humanBy(RESOLVE, id, context)
        if ('problem' in person) return refuse(person.problem)
        by = person.by
      } else {
        by = args.by as string
        if (by === id) return refuse(`${id} cannot be resolved by itself`)
        if (!scan.items.some((other) => other.item.id === by)) {
          return refuse(`there is no item ${by} to resolve ${id} by`)
        }
      }
      const text = (args.text as string).trim()
      return {
        kind: 'edit',
        edit: (front) => chain(front, (t) => setStatusLine(t, 'resolved'), (t) => setFieldLine(t, 'resolved_by', by)),
        expected: { ...frontmatter, status: 'resolved', resolved_by: by },
        body: (body, eol) => withDecision(body, text, eol),
      }
    },
  },
}
