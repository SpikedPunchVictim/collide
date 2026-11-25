import { describe, expect, test } from 'vitest'
import { firstChange, patchLines, wholeLines } from '../src/drawer/diff.ts'
import { PARSER_PATCH, wholeOf } from './support/changes-world.ts'

// The diff window's lines (ts-0189-fg; note 29): a patch as numbered, marked lines, and the whole file after the
// changes. The window itself, its two views and its scroll are in `browser/fake-diff.spec.ts`.

describe('a patch as lines', () => {
  const lines = patchLines(PARSER_PATCH)
  const shown = lines.map((line) => `${line.kind}:${line.n ?? '-'}`)

  test('marks the header, each hunk, and each added, removed and kept line', () => {
    expect(shown).toEqual([
      'meta:-',
      'meta:-',
      'meta:-',
      'meta:-',
      'hunk:-',
      'ctx:1',
      'del:-',
      'add:2',
      'ctx:3',
      'ctx:4',
      'hunk:-',
      'ctx:40',
      'add:41',
      'ctx:42',
    ])
  })

  test('numbers a line by its place in the file after the changes, and a removed line has none', () => {
    expect(lines.filter((line) => line.kind === 'add').map((line) => line.n)).toEqual([2, 41])
    expect(lines.find((line) => line.kind === 'del')!.n).toBeNull()
  })

  test('numbers the second hunk from its own header', () => {
    expect(lines.find((line) => line.text === '  return 1')!.n).toBe(40)
  })

  test('keeps a line as it is, less its mark', () => {
    expect(lines.find((line) => line.kind === 'add')!.text).toBe('export const VERSION = 2')
    expect(lines.find((line) => line.kind === 'hunk')!.text).toBe('@@ -1,4 +1,4 @@')
  })

  test('reads a header line that looks like a change as a header, until the first hunk', () => {
    const text = ['--- a/x', '+++ b/x', '@@ -1 +1 @@', '-+++ in the file', '++a plus'].join('\n')
    expect(patchLines(text).map((line) => line.kind)).toEqual(['meta', 'meta', 'hunk', 'del', 'add'])
  })

  test('puts no number on a "no newline" note, and takes a bare empty line in a hunk as a kept line', () => {
    const found = patchLines('@@ -1,2 +1,2 @@\n a\n\n\\ No newline at end of file\n')
    expect(found.map((line) => `${line.kind}:${line.n ?? '-'}`)).toEqual(['hunk:-', 'ctx:1', 'ctx:2', 'meta:-'])
  })

  test('starts again at the next file of a patch', () => {
    const text = '@@ -1 +1 @@\n+a\ndiff --git a/y b/y\n--- a/y\n+++ b/y\n@@ -5 +9 @@\n+b'
    const found = patchLines(text)
    expect(found.filter((line) => line.kind === 'add').map((line) => line.n)).toEqual([1, 9])
    expect(found.filter((line) => line.kind === 'meta')).toHaveLength(3)
  })

  test('gives no line for an empty patch', () => {
    expect(patchLines('')).toEqual([])
  })

  test('a patch of a binary file is its header and the words git gives', () => {
    const text = 'diff --git a/a.png b/a.png\nBinary files /dev/null and b/a.png differ\n'
    expect(patchLines(text).map((line) => line.kind)).toEqual(['meta', 'meta'])
  })
})

describe('the whole file as lines', () => {
  test('numbers each line as the file does and marks the changed ones as added', () => {
    const lines = wholeLines(wholeOf(5, [2, 4]).lines)
    expect(lines.map((line) => `${line.kind}:${line.n}`)).toEqual(['ctx:1', 'add:2', 'ctx:3', 'add:4', 'ctx:5'])
    expect(lines[1]!.text).toBe('line 2')
  })
})

describe('where the window scrolls to', () => {
  test('the first added or removed line of a patch, past the header and the context', () => {
    expect(firstChange(patchLines(PARSER_PATCH))).toBe(6)
  })

  test('the first changed line of a whole file', () => {
    expect(firstChange(wholeLines(wholeOf(200, [150, 151]).lines))).toBe(149)
  })

  test('nothing for a file the changes left as it was', () => {
    expect(firstChange(wholeLines(wholeOf(3, []).lines))).toBe(-1)
  })
})
