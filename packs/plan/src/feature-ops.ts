import { TITLE_MAX } from './create.ts'
import { SECTION_MAX } from './features.ts'
import { addEntry, moveEntry } from './feature-text.ts'
import type { TextResult, Where } from './feature-text.ts'
import { CONTROL, onlyKeys } from './op-util.ts'
import type { OpSpec, Prepared } from './op-util.ts'
import { forProposal } from './person-ops.ts'
import type { FoundItem } from './scan.ts'

/**
 * The ops that write a proposal's Features part (0.10, "A proposal's features"): `addFeature` and `moveFeature`. Any
 * actor may make them, so they are not in `transitions.ts`. `renameSection` changes many files and has its own module.
 */
export const ADD_FEATURE = 'addFeature'
export const MOVE_FEATURE = 'moveFeature'

/** The form of a feature's id in `moveFeature`'s `feature`: the part's own `F<n>`, with `n` from 1. */
export const FEATURE_PATTERN = '^F[1-9][0-9]*$'
const featureRegex = new RegExp(FEATURE_PATTERN)

/** A control character other than a tab, which a summary may hold. */
const OTHER_CONTROL = /[\u0000-\u0008\u000b-\u001f\u007f]/

/**
 * The problem with a section name that an op is to write, or null: the part's own rule (1 to `SECTION_MAX` characters,
 * no `#`), and no control character. An empty name is the caller's to allow, as Unsorted.
 */
export function sectionProblem(name: string): string | null {
  const length = [...name.trim()].length
  if (CONTROL.test(name)) return 'a section name has no line break or other control character'
  if (length > SECTION_MAX) return `a section name is at most ${SECTION_MAX} characters; this one has ${length}`
  if (name.includes('#')) return 'a section name has no #'
  return null
}

/** The problem with `section`, which is required and may be empty (the part's Unsorted), or null. */
export function wantedSectionProblem(args: Record<string, unknown>): string | null {
  if (args.section === undefined) return 'section is required: give a section name, or an empty one for Unsorted'
  return typeof args.section === 'string' ? sectionProblem(args.section) : 'section must be text'
}

/** The lines of a summary, each trimmed: one line, or several that continue the entry. */
export function summaryLinesOf(summary: string): string[] {
  return summary.trim().split(/\r\n|\r|\n/).map((line) => line.trim())
}

function titleProblem(title: unknown): string | null {
  if (typeof title !== 'string' || title.trim() === '' || [...title.trim()].length > TITLE_MAX) {
    return `title must be text of 1 to ${TITLE_MAX} characters`
  }
  if (CONTROL.test(title)) return 'title is one line: no line break or other control character'
  return title.includes('**') ? 'title has no **: it would end the bold title early' : null
}

function summaryProblem(summary: unknown): string | null {
  if (typeof summary !== 'string' || summary.trim() === '') return 'summary must be text, one line or several'
  if (OTHER_CONTROL.test(summary.replace(/\r\n|\r|\n/g, ' '))) return 'summary has no control character but a tab'
  if (summaryLinesOf(summary).some((line) => line === '')) {
    return 'summary has no blank line: a blank line ends the entry, so write one paragraph'
  }
  return null
}

/** What a feature op does to a body: given the body and the file's line ending, the new body or the reason not to. */
type Run = (where: Where, body: string, eol: string) => TextResult

/**
 * The op's `prepare`: runs `run` once on the body as read, to refuse or to find there is nothing to do, and hands the
 * write the same edit to make on the body as the file has it, with the file's own line ending. The frontmatter is
 * not changed.
 */
function bodyEdit(op: string, found: FoundItem, frontmatter: Record<string, unknown>, run: Run): Prepared {
  const wrong = forProposal(op, found)
  if (wrong !== null) return wrong
  const where: Where = { id: found.item.id, path: found.path, firstLine: found.bodyLine }
  const dry = run(where, found.body, '\n')
  if (!dry.ok) return { kind: 'refuse', message: dry.message }
  if (!dry.changed) return { kind: 'nothing' }
  return {
    kind: 'edit',
    edit: (text) => ({ ok: true, text }),
    expected: frontmatter,
    body: (body, eol) => {
      const result = run(where, body, eol)
      return result.ok ? result.body : { refuse: result.message }
    },
  }
}

export const FEATURE_OPS: Record<string, OpSpec> = {
  [ADD_FEATURE]: {
    check: (args) =>
      wantedSectionProblem(args) ??
      titleProblem(args.title) ??
      summaryProblem(args.summary) ??
      onlyKeys(args, ['section', 'title', 'summary']),
    prepare: (found, _scan, args, frontmatter) => {
      const title = (args.title as string).trim()
      const lines = summaryLinesOf(args.summary as string)
      const run: Run = (where, body, eol) => addEntry(where, body, eol, args.section as string, title, lines)
      return bodyEdit(ADD_FEATURE, found, frontmatter, run)
    },
  },
  [MOVE_FEATURE]: {
    check: (args) => {
      if (typeof args.feature !== 'string' || !featureRegex.test(args.feature)) {
        return 'feature must be F<n>, such as F2, with n from 1'
      }
      return wantedSectionProblem(args) ?? onlyKeys(args, ['feature', 'section'])
    },
    prepare: (found, _scan, args, frontmatter) => {
      const run: Run = (where, body, eol) => moveEntry(where, body, eol, args.feature as string, args.section as string)
      return bodyEdit(MOVE_FEATURE, found, frontmatter, run)
    },
  },
}
