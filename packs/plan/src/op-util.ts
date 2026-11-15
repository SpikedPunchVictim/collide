import type { ApplyResultInput } from '@collide/pack-sdk'

export const refused = (message: string): ApplyResultInput => ({ outcome: 'refused', message })

export const stringArg = (args: Record<string, unknown>, name: string): string | null => (typeof args[name] === 'string' ? args[name] : null)

/** Refuses an argument that the op does not declare, so a misspelled one is not silently dropped. */
export const onlyKeys = (args: Record<string, unknown>, allowed: string[]): string | null => {
  const extra = Object.keys(args).find((key) => !allowed.includes(key))
  return extra === undefined ? null : `unknown argument "${extra}"; this op takes ${allowed.length === 0 ? 'no arguments' : allowed.join(', ')}`
}

/** A control character: never allowed in a title or a claimant, where it would break a line of frontmatter. */
export const CONTROL = /[\u0000-\u001f\u007f]/
