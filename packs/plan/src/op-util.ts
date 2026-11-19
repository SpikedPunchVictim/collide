import type { ApplyResultInput } from '@collide/pack-sdk'
import type { Edit } from './frontmatter-edit.ts'
import type { FoundItem, Scan } from './scan.ts'
import type { Asker } from './transitions.ts'

export const refused = (message: string): ApplyResultInput => ({ outcome: 'refused', message })

export const stringArg = (args: Record<string, unknown>, name: string): string | null => (typeof args[name] === 'string' ? args[name] : null)

/** Refuses an argument that the op does not declare, so a misspelled one is not silently dropped. */
export const onlyKeys = (args: Record<string, unknown>, allowed: string[]): string | null => {
  const extra = Object.keys(args).find((key) => !allowed.includes(key))
  return extra === undefined ? null : `unknown argument "${extra}"; this op takes ${allowed.length === 0 ? 'no arguments' : allowed.join(', ')}`
}

/** A control character: never allowed in a title or a claimant, where it would break a line of frontmatter. */
export const CONTROL = /[\u0000-\u001f\u007f]/

export type Frontmatter = Record<string, unknown>

/** What an op does to one item: nothing, a refusal, or a line edit plus the frontmatter it must produce. */
export type Prepared =
  | { kind: 'refuse'; message: string }
  | { kind: 'nothing' }
  | {
      kind: 'edit'
      edit: (text: string) => Edit
      expected: Frontmatter
      /** Changes the body too, given the body and the file's own line ending. The body is read back after the write. */
      body?: (body: string, eol: string) => string
    }

/** What an op is told besides the item: who asked. */
export interface OpContext {
  asker: Asker | null
}

export interface OpSpec {
  /** The problem with `args`, or null. Runs before the lock is taken. */
  check: (args: Record<string, unknown>) => string | null
  prepare: (
    found: FoundItem,
    scan: Scan,
    args: Record<string, unknown>,
    frontmatter: Frontmatter,
    context: OpContext,
  ) => Prepared
}

export const article = (kind: string): string => `${/^[aeiou]/.test(kind) ? 'an' : 'a'} ${kind}`
