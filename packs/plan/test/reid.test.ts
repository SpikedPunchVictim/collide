import { existsSync, readFileSync, writeFileSync } from 'node:fs'
import { join } from 'node:path'
import { describe, expect, test } from 'vitest'
import { replaceLinkTarget, setIdLine } from '../src/frontmatter-edit.ts'
import { call, commitAll, EPIC_FILE, git, makeRepo } from './helpers.ts'

const EPICS = '.collide/planning/epics'
const TASKS = `${EPICS}/ep-001-aa-alpha/tasks`

const task = (id: string, links: string, extra = ''): string => `---\nid: ${id}\nkind: task\nstatus: ready\ntitle: "Item ${id}"\nowners: []\n${links}${extra}---\n\n## Goal\n\nMentions ts-0002-bb in prose, which is not a link.\n`
const blockedBy = (...ids: string[]): string => `links:\n${ids.map((id) => `  - { type: blocked-by, to: ${id} }\n`).join('')}`

/**
 * ts-0002-bb is in two files (the first is "one", the second "two": the scan keeps the first and sets the second aside).
 * Links to it are in several files and several forms; `quiet` has none.
 */
function sharedIdRepo(): string {
  const root = makeRepo({
    'ep-001-aa-alpha/epic.md': EPIC_FILE,
    'ep-001-aa-alpha/tasks/ts-0001-aa-start.md': task('ts-0001-aa', 'links: []\n'),
    'ep-001-aa-alpha/tasks/ts-0002-bb-one.md': task('ts-0002-bb', 'links: []\n'),
    'ep-001-aa-alpha/tasks/ts-0002-bb-two.md': task('ts-0002-bb', blockedBy('ts-0001-aa', 'ts-0002-bb')),
    'ep-001-aa-alpha/tasks/ts-0003-cc-one-line.md': task('ts-0003-cc', blockedBy('ts-0001-aa', 'ts-0002-bb')),
    'ep-001-aa-alpha/tasks/ts-0004-dd-quoted.md': task('ts-0004-dd', 'links:\n  - { type: blocked-by, to: "ts-0002-bb" }  # why\n'),
    'ep-001-aa-alpha/tasks/ts-0005-ee-block.md': task('ts-0005-ee', 'links:\n  - type: blocked-by\n    to: ts-0002-bb\n  - type: relates-to\n    to: ts-0001-aa\n'),
    'ep-001-aa-alpha/tasks/ts-0006-ff-quiet.md': task('ts-0006-ff', blockedBy('ts-0001-aa')),
    'ep-002-bb-beta/epic.md': EPIC_FILE.replaceAll('ep-001-aa', 'ep-002-bb'),
    'ep-002-bb-beta/tasks/ts-0007-gg-other-epic.md': task('ts-0007-gg', blockedBy('ts-0002-bb')),
  })
  git(root, 'init', '--quiet')
  commitAll(root, 'fixture')
  return root
}

const reid = (root: string, id: string, args: Record<string, unknown> = {}, extra: Record<string, unknown> = {}) => call(root, 'apply', { op: 'reid', id, args, ...extra })
const read = (root: string, path: string): string => readFileSync(join(root, path), 'utf8')
const commits = (root: string): number => Number(git(root, 'rev-list', '--count', '--all').trim())

describe('the line edits', () => {
  test('setIdLine changes the value and keeps a quote, a comment and the line ending', () => {
    expect(setIdLine('id: ts-0001-aa\r\nkind: task\r\n', 'ts-0001-bc')).toEqual({ ok: true, text: 'id: ts-0001-bc\r\nkind: task\r\n' })
    expect(setIdLine('id:  "ts-0001-aa"  # tag\n', 'ts-0001-bc')).toEqual({ ok: true, text: 'id:  "ts-0001-bc"  # tag\n' })
    expect(setIdLine('kind: task\n', 'ts-0001-bc')).toMatchObject({ ok: false })
    expect(setIdLine('id: !!str ts-0001-aa\n', 'ts-0001-bc')).toMatchObject({ ok: false })
  })

  test('replaceLinkTarget changes a target in a one-line entry, a quoted one, a block mapping and a flow list, and nothing else', () => {
    const text = 'id: ts-0009-zz\nlinks:\n  - { type: blocked-by, to: ts-0002-bb }\n  - { type: blocked-by, to: "ts-0002-bb" } # c\n  - type: blocked-by\n    to: ts-0002-bb\n  - { type: blocked-by, to: ts-0002-bbb }\n  - { type: blocked-by, to: ts-0003-cc }\nowners: []\n'
    expect(replaceLinkTarget(text, 'ts-0002-bb', 'ts-0002-cd')).toEqual({
      ok: true,
      text: 'id: ts-0009-zz\nlinks:\n  - { type: blocked-by, to: ts-0002-cd }\n  - { type: blocked-by, to: "ts-0002-cd" } # c\n  - type: blocked-by\n    to: ts-0002-cd\n  - { type: blocked-by, to: ts-0002-bbb }\n  - { type: blocked-by, to: ts-0003-cc }\nowners: []\n',
    })
    expect(replaceLinkTarget('links: [{ type: blocked-by, to: ts-0002-bb }]\n', 'ts-0002-bb', 'ts-0002-cd')).toEqual({ ok: true, text: 'links: [{ type: blocked-by, to: ts-0002-cd }]\n' })
    expect(replaceLinkTarget('links: []\ntitle: "to: ts-0002-bb"\n', 'ts-0002-bb', 'ts-0002-cd')).toMatchObject({ ok: false })
  })
})

describe('reid', () => {
  test('is declared with an optional seed', async () => {
    const { result } = await call(sharedIdRepo(), 'initialize', { config: {} })
    const op = (result?.ops as Array<Record<string, any>>).find((candidate) => candidate.name === 'reid')
    expect(op?.args).toMatchObject({ properties: { seed: { type: 'integer' } }, additionalProperties: false })
    expect(op?.args.required).toBeUndefined()
  })

  test('on an id two files share, renames the second (the task.invalid), rewrites every link to it in every file, and commits nothing', async () => {
    const root = sharedIdRepo()
    const before = await call(root, 'list')
    expect(before.invalid).toEqual([{ path: `${TASKS}/ts-0002-bb-two.md`, checkout: null, reason: `id "ts-0002-bb" is already used by ${TASKS}/ts-0002-bb-one.md` }])
    const untouched = ['ts-0001-aa-start.md', 'ts-0002-bb-one.md', 'ts-0006-ff-quiet.md'].map((name) => [name, read(root, `${TASKS}/${name}`)])
    const commitsBefore = commits(root)

    const { result } = await reid(root, 'ts-0002-bb', { seed: 1 })

    const renamed = `${TASKS}/ts-0002-ra-two.md`
    const links = ['ts-0003-cc-one-line.md', 'ts-0004-dd-quoted.md', 'ts-0005-ee-block.md'].map((name) => `${TASKS}/${name}`)
    const otherEpic = `${EPICS}/ep-002-bb-beta/tasks/ts-0007-gg-other-epic.md`
    expect(result).toMatchObject({ outcome: 'ok', task: { id: 'ts-0002-ra', path: renamed, title: 'Item ts-0002-bb' } })
    expect([...result?.files].sort()).toEqual([...links, otherEpic, renamed, `${TASKS}/ts-0002-bb-two.md`].sort())
    expect(result?.items.map((effect: { key: string }) => effect.key).sort()).toEqual(['plan:ts-0002-ra', 'plan:ts-0003-cc', 'plan:ts-0004-dd', 'plan:ts-0005-ee', 'plan:ts-0007-gg'])

    // The renamed file has the new id and its own link to the shared id, which names itself, follows it.
    expect(existsSync(join(root, `${TASKS}/ts-0002-bb-two.md`))).toBe(false)
    expect(read(root, renamed)).toBe(task('ts-0002-ra', blockedBy('ts-0001-aa', 'ts-0002-ra')).replace('Item ts-0002-ra', 'Item ts-0002-bb'))
    // Each link's file differs only in the target. The body's prose mention is not a link and stays.
    expect(read(root, links[0] as string)).toBe(task('ts-0003-cc', blockedBy('ts-0001-aa', 'ts-0002-ra')))
    expect(read(root, links[1] as string)).toBe(task('ts-0004-dd', 'links:\n  - { type: blocked-by, to: "ts-0002-ra" }  # why\n'))
    expect(read(root, links[2] as string)).toBe(task('ts-0005-ee', 'links:\n  - type: blocked-by\n    to: ts-0002-ra\n  - type: relates-to\n    to: ts-0001-aa\n'))
    expect(read(root, otherEpic)).toBe(task('ts-0007-gg', blockedBy('ts-0002-ra')))
    for (const [name, text] of untouched) expect(read(root, `${TASKS}/${name}`)).toBe(text)

    // Nothing is committed; the changes are for `git diff` and `git status`.
    expect(commits(root)).toBe(commitsBefore)
    expect(git(root, 'diff', '--numstat', '--', EPICS).split('\n').filter(Boolean).sort()).toEqual(
      [`1\t1\t${links[0]}`, `1\t1\t${links[1]}`, `1\t1\t${links[2]}`, `1\t1\t${otherEpic}`, `0\t14\t${TASKS}/ts-0002-bb-two.md`].sort(),
    )
    expect(git(root, 'status', '--porcelain', '--', EPICS).split('\n').filter(Boolean).sort()).toEqual(
      [` M ${links[0]}`, ` M ${links[1]}`, ` M ${links[2]}`, ` M ${otherEpic}`, ` D ${TASKS}/ts-0002-bb-two.md`, `?? ${renamed}`].sort(),
    )

    // The two items now have distinct ids and nothing is invalid.
    const after = await call(root, 'list')
    expect(after.invalid).toEqual([])
    const ids = (after.result?.tasks as Array<{ id: string }>).map((candidate) => candidate.id)
    expect(ids).toEqual(expect.arrayContaining(['ts-0002-bb', 'ts-0002-ra']))
    expect(git(root, 'ls-files', '--others', '--exclude-standard', '--', '.collide/planning/.write-lock')).toBe('')
  })

  test('an id one file has: the item is renamed, its links follow, and the new suffix is a draw of the seed', async () => {
    const root = sharedIdRepo()
    await reid(root, 'ts-0002-bb', { seed: 1 })
    const { result } = await reid(root, 'ts-0002-bb', { seed: 2 })
    // Only the first file has the id now; its suffix comes from the second seed's first draw.
    expect(result).toMatchObject({ outcome: 'ok', task: { id: expect.stringMatching(/^ts-0002-(?!bb)[a-hjkmnp-z]{2}$/), path: expect.stringMatching(/ts-0002-[a-z]{2}-one\.md$/) } })
    expect((await call(root, 'list')).invalid).toEqual([])
  })

  test('a drawn suffix that a ref holds is redrawn: seed 1 would give ra, which a branch has', async () => {
    const root = sharedIdRepo()
    git(root, 'checkout', '--quiet', '-b', 'elsewhere')
    writeFileSync(join(root, TASKS, 'ts-0002-ra-on-a-branch.md'), task('ts-0002-ra', 'links: []\n'))
    commitAll(root, 'elsewhere')
    git(root, 'checkout', '--quiet', 'main')
    const { result } = await reid(root, 'ts-0002-bb', { seed: 1 })
    expect(result).toMatchObject({ outcome: 'ok', task: { id: 'ts-0002-pz' } })
  })

  test('renames an epic\'s folder with every file in it, and reports each file at its old and new path', async () => {
    const root = makeRepo({
      'ep-001-aa-alpha/epic.md': EPIC_FILE,
      'ep-001-aa-alpha/tasks/ts-0001-aa-in-alpha.md': task('ts-0001-aa', 'links: []\n'),
      'ep-001-aa-beta/epic.md': EPIC_FILE,
      'ep-001-aa-beta/tasks/ts-0002-bb-in-beta.md': task('ts-0002-bb', blockedBy('ep-001-aa')),
      'ep-001-aa-beta/notes.txt': 'free text\n',
      'ep-003-cc-gamma/epic.md': EPIC_FILE.replaceAll('ep-001-aa', 'ep-003-cc'),
      'ep-003-cc-gamma/tasks/ts-0003-cc-links.md': task('ts-0003-cc', blockedBy('ep-001-aa')),
    })
    git(root, 'init', '--quiet')
    commitAll(root, 'fixture')
    const { result } = await reid(root, 'ep-001-aa', { seed: 1 })
    expect(result).toMatchObject({ outcome: 'ok', task: { id: 'ep-001-ra', path: `${EPICS}/ep-001-ra-beta/epic.md`, kind: 'epic' } })
    expect([...result?.files].sort()).toEqual(
      [
        `${EPICS}/ep-001-aa-beta/epic.md`,
        `${EPICS}/ep-001-ra-beta/epic.md`,
        `${EPICS}/ep-001-aa-beta/tasks/ts-0002-bb-in-beta.md`,
        `${EPICS}/ep-001-ra-beta/tasks/ts-0002-bb-in-beta.md`,
        `${EPICS}/ep-001-aa-beta/notes.txt`,
        `${EPICS}/ep-001-ra-beta/notes.txt`,
        `${EPICS}/ep-003-cc-gamma/tasks/ts-0003-cc-links.md`,
      ].sort(),
    )
    expect(existsSync(join(root, EPICS, 'ep-001-aa-beta'))).toBe(false)
    expect(read(root, `${EPICS}/ep-001-ra-beta/tasks/ts-0002-bb-in-beta.md`)).toBe(task('ts-0002-bb', blockedBy('ep-001-ra')))
    expect(read(root, `${EPICS}/ep-003-cc-gamma/tasks/ts-0003-cc-links.md`)).toBe(task('ts-0003-cc', blockedBy('ep-001-ra')))
    const after = await call(root, 'list')
    expect(after.invalid).toEqual([])
    const tasks = after.result?.tasks as Array<{ id: string; links: Array<{ type: string; to: string }> }>
    expect(tasks.find((candidate) => candidate.id === 'ts-0002-bb')?.links).toContainEqual({ type: 'part-of', to: 'ep-001-ra' })
  })

  test('refuses and writes nothing when a link is in a form the provider does not edit', async () => {
    const root = sharedIdRepo()
    const odd = `${TASKS}/ts-0003-cc-one-line.md`
    writeFileSync(join(root, odd), task('ts-0003-cc', 'links:\n  - { type: blocked-by, to: !!str ts-0002-bb }\n'))
    commitAll(root, 'odd')
    const { result } = await reid(root, 'ts-0002-bb', { seed: 1 })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining('ts-0003-cc-one-line.md') })
    expect(git(root, 'status', '--porcelain')).toBe('')
  })

  test.each([
    [{ id: 'ts-0099-zz' }, 'there is no item ts-0099-zz'],
    [{ id: 'ts-0002-bb', version: 'abc' }, 'reid takes no version'],
    [{ id: 'ts-0002-bb', args: { seed: 'x' } }, 'seed must be a whole number'],
    [{ id: 'ts-0002-bb', args: { name: 'x' } }, 'unknown argument "name"'],
    [{ id: '' }, 'reid needs the id'],
  ])('refuses %j and writes nothing', async (extra, message) => {
    const root = sharedIdRepo()
    const { result } = await call(root, 'apply', { op: 'reid', args: {}, ...extra })
    expect(result).toMatchObject({ outcome: 'refused', message: expect.stringContaining(message) })
    expect(git(root, 'status', '--porcelain')).toBe('')
  })
})
