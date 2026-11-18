import { createHash } from 'node:crypto'
import { open } from 'node:fs/promises'
import { parse } from '@collide/pack-sdk/yaml'

/** The frontmatter block (the text between the two `---` lines) may be this many bytes. Checked before parsing. */
export const FRONTMATTER_CAP = 64 * 1024

/**
 * The body (everything after the closing `---` line) may be this many bytes. Real task files reach about 330 KB, so
 * the cap is 1 MiB: three times the largest, and small enough that one file cannot make a `get` reply unreasonable.
 */
export const BODY_CAP = 1024 * 1024

export type ReadResult =
  | { ok: true; frontmatter: Record<string, unknown>; body: string; version: string }
  | { ok: false; reason: string }

const fail = (reason: string): ReadResult => ({ ok: false, reason })

/** Where the frontmatter block starts and ends in the head of a file, or why there is none. */
type Fence = { ok: true; start: number; end: number; bodyStart: number } | { ok: false; reason: string }

const lf = 0x0a
const cr = 0x0d

/** Finds the opening and closing `---` lines in `head`, which holds the first bytes of a file of `size` bytes. */
function findFence(head: Buffer, size: number): Fence {
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

const isMapping = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

/** Whether a mapping, at any depth, has the merge key `<<`, which this reader does not honor. */
function hasMergeKey(value: unknown): boolean {
  if (Array.isArray(value)) return value.some(hasMergeKey)
  if (!isMapping(value)) return false
  return Object.entries(value).some(([key, inner]) => key === '<<' || hasMergeKey(inner))
}

/**
 * Parses frontmatter text as YAML 1.2 core. A duplicate key is an error, an anchor or an alias is refused, and so is a
 * merge key. Nothing is coerced beyond what the core schema does; the caller checks each field's type.
 */
function parseFrontmatter(text: string): { ok: true; value: Record<string, unknown> } | { ok: false; reason: string } {
  let anchored = false
  try {
    const value: unknown = parse(text, {
      version: '1.2',
      schema: 'core',
      uniqueKeys: true,
      merge: false,
      // An anchor is refused below, so an alias has nothing to name; a limit of 0 also stops one at the first read.
      maxAliasCount: 0,
      onAnchor: () => void (anchored = true),
      // `parse` would otherwise print a warning to the process's stderr, which is Collide's log.
      logLevel: 'error',
    })
    if (anchored) return { ok: false, reason: 'frontmatter uses an anchor, which is not allowed' }
    if (!isMapping(value)) return { ok: false, reason: 'frontmatter must be a mapping of keys to values' }
    if (hasMergeKey(value)) return { ok: false, reason: 'frontmatter uses a merge key (<<), which is not allowed' }
    return { ok: true, value }
  } catch (error) {
    const message = (error instanceof Error ? error.message : String(error)).split('\n')[0]
    if (/alias/i.test(message ?? '')) {
      return { ok: false, reason: 'frontmatter uses an alias or anchor, which is not allowed' }
    }
    return { ok: false, reason: `frontmatter is not valid YAML: ${message}` }
  }
}

/** A task's `version`: the sha256 of the file's whole content. */
const versionOf = (content: Buffer): string => createHash('sha256').update(content).digest('hex')

const utf8 = new TextDecoder('utf-8', { fatal: true })

const decode = (bytes: Buffer): string | null => {
  try {
    return utf8.decode(bytes)
  } catch {
    return null
  }
}

const CONFLICT_MARKER = /^(<{7}|>{7})( |$)/m

/**
 * Reads one file of the corpus. Only the head of the file is read until the frontmatter block is known to be under its
 * cap, and the rest only when the body is under its cap too, so an oversized file is never held in memory.
 */
export async function readCorpusFile(path: string): Promise<ReadResult> {
  const handle = await open(path, 'r')
  try {
    const { size } = await handle.stat()
    const head = Buffer.alloc(Math.min(size, FRONTMATTER_CAP + 16))
    await handle.read(head, 0, head.length, 0)

    const fence = findFence(head, size)
    if (!fence.ok) return fail(fence.reason)
    if (fence.end - fence.start > FRONTMATTER_CAP) {
      return fail(`frontmatter block is ${fence.end - fence.start} bytes, over the ${FRONTMATTER_CAP}-byte cap`)
    }
    const bodyBytes = size - fence.bodyStart
    if (bodyBytes > BODY_CAP) return fail(`body is ${bodyBytes} bytes, over the ${BODY_CAP}-byte cap`)

    const whole = Buffer.alloc(size)
    await handle.read(whole, 0, size, 0)
    const frontmatterText = decode(whole.subarray(fence.start, fence.end))
    const body = decode(whole.subarray(fence.bodyStart))
    if (frontmatterText === null || body === null) return fail('the file is not valid UTF-8')
    if (CONFLICT_MARKER.test(frontmatterText) || CONFLICT_MARKER.test(body)) {
      return fail('the file holds a merge conflict marker (a line that starts with <<<<<<< or >>>>>>>)')
    }

    const parsed = parseFrontmatter(frontmatterText)
    if (!parsed.ok) return fail(parsed.reason)
    return { ok: true, frontmatter: parsed.value, body, version: versionOf(whole) }
  } finally {
    await handle.close()
  }
}
