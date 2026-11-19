import { createHash } from 'node:crypto'
import { open } from 'node:fs/promises'
import { parseDocument } from 'yaml'

/** The frontmatter block (the text between the two `---` lines) may be this many bytes. Checked before parsing. */
export const FRONTMATTER_CAP = 64 * 1024

/**
 * The body (everything after the closing `---` line) may be this many bytes: about five times the largest record
 * in this repo (51 KB), and small enough that one item cannot make a `get` reply unreasonable.
 */
export const BODY_CAP = 256 * 1024

/** How many aliases `yaml` expands before it refuses (its own default, kept explicit). */
export const MAX_ALIAS_COUNT = 100

/** The `yaml` options for everything the pack reads (S5): YAML 1.2 core, duplicate keys refused, no merge key. */
export const YAML_OPTIONS = { version: '1.2', schema: 'core', uniqueKeys: true, merge: false } as const

export type ReadResult =
  | { ok: true; frontmatter: Record<string, unknown>; body: string; bodyLine: number; version: string }
  | { ok: false; reason: string }

const fail = (reason: string): ReadResult => ({ ok: false, reason })

/** Where the frontmatter block starts and ends in the head of a file, or why there is none. */
export type Fence = { ok: true; start: number; end: number; bodyStart: number } | { ok: false; reason: string }

const lf = 0x0a
const cr = 0x0d

/** Finds the opening and closing `---` lines in `head`, which holds the first bytes of a file of `size` bytes. */
export function findFence(head: Buffer, size: number): Fence {
  let start: number
  if (head.subarray(0, 4).equals(Buffer.from('---\n'))) start = 4
  else if (head.subarray(0, 5).equals(Buffer.from('---\r\n'))) start = 5
  else return { ok: false, reason: 'no frontmatter: the file must start with a line of three dashes' }

  for (let at = head.indexOf('---', start); at !== -1; at = head.indexOf('---', at + 1)) {
    const atLineStart = at === start || head[at - 1] === lf
    if (!atLineStart) continue
    const after = at + 3
    if (after === size) return { ok: true, start, end: at, bodyStart: after }
    if (head[after] === lf) return { ok: true, start, end: at, bodyStart: after + 1 }
    if (head[after] === cr && head[after + 1] === lf) return { ok: true, start, end: at, bodyStart: after + 2 }
  }
  if (head.length >= size) return { ok: false, reason: 'no frontmatter: the closing line of three dashes is missing' }
  return { ok: false, reason: `frontmatter block is over the ${FRONTMATTER_CAP}-byte cap` }
}

const isMapping = (value: unknown): value is Record<string, unknown> => typeof value === 'object' && value !== null && !Array.isArray(value)

/**
 * Parses frontmatter text as YAML 1.2 core. Duplicate keys and an excess of aliases are errors, there is no merge
 * key and no custom tag, and nothing is coerced beyond what the core schema does. The caller checks each field's type.
 */
export function parseFrontmatter(text: string): { ok: true; value: Record<string, unknown> } | { ok: false; reason: string } {
  try {
    const doc = parseDocument(text, YAML_OPTIONS)
    const problem = doc.errors[0] ?? doc.warnings[0]
    if (problem) return { ok: false, reason: `frontmatter is not valid YAML: ${problem.message.split('\n')[0]}` }
    const value: unknown = doc.toJS({ maxAliasCount: MAX_ALIAS_COUNT })
    if (!isMapping(value)) return { ok: false, reason: 'frontmatter must be a mapping of keys to values' }
    return { ok: true, value }
  } catch (error) {
    return { ok: false, reason: `frontmatter is not valid YAML: ${error instanceof Error ? error.message : String(error)}` }
  }
}

/** A task's `version`: the sha256 of the file's whole content. */
export const versionOf = (content: Buffer): string => createHash('sha256').update(content).digest('hex')

const utf8 = new TextDecoder('utf-8', { fatal: true })

const decode = (bytes: Buffer): string | null => {
  try {
    return utf8.decode(bytes)
  } catch {
    return null
  }
}

/**
 * Reads one item file. Only the head of the file is read until the frontmatter block is known to be under its cap,
 * and the rest only when the body is under its cap too, so an oversized file is never held in memory. The body is
 * returned as text, with the line of the file it starts on, and never parsed here.
 */
export async function readItemFile(path: string): Promise<ReadResult> {
  const handle = await open(path, 'r')
  try {
    const { size } = await handle.stat()
    const head = Buffer.alloc(Math.min(size, FRONTMATTER_CAP + 16))
    await handle.read(head, 0, head.length, 0)

    const fence = findFence(head, size)
    if (!fence.ok) return fail(fence.reason)
    if (fence.end - fence.start > FRONTMATTER_CAP) return fail(`frontmatter block is ${fence.end - fence.start} bytes, over the ${FRONTMATTER_CAP}-byte cap`)
    const bodyBytes = size - fence.bodyStart
    if (bodyBytes > BODY_CAP) return fail(`body is ${bodyBytes} bytes, over the ${BODY_CAP}-byte cap`)

    const whole = Buffer.alloc(size)
    await handle.read(whole, 0, size, 0)
    const frontmatterText = decode(whole.subarray(fence.start, fence.end))
    const body = decode(whole.subarray(fence.bodyStart))
    if (frontmatterText === null || body === null) return fail('the file is not valid UTF-8')

    const parsed = parseFrontmatter(frontmatterText)
    if (!parsed.ok) return fail(parsed.reason)
    // The body's first line is a line of the file, so a problem found in the body can name it.
    const bodyLine = 1 + whole.subarray(0, fence.bodyStart).filter((byte) => byte === lf).length
    return { ok: true, frontmatter: parsed.value, body, bodyLine, version: versionOf(whole) }
  } finally {
    await handle.close()
  }
}
