import { describe, expect, test } from 'vitest'
import {
  LIVE_WORDS,
  NOTHING_YET,
  filesWords,
  hasLiveRun,
  isEmpty,
  isLive,
  runHeading,
  shortCommit,
  sizeWords,
  whereWords,
} from '../src/drawer/changes.ts'
import { changesOf, commitEntry, file, runEntry } from './support/changes-world.ts'

// The words and rules of the Changes part (ts-0189-fg; note 17). Drawing it is in `drawer-changes.test.tsx`.

describe('which runs are live', () => {
  test('a running run is live, and a finished, failed, stopped or paused one is not', () => {
    expect(isLive(runEntry('r-1'))).toBe(true)
    expect(isLive(runEntry('r-1', { state: { state: 'ended', outcome: 'done' } }))).toBe(false)
    expect(isLive(runEntry('r-1', { state: { state: 'failed' } }))).toBe(false)
    expect(isLive(runEntry('r-1', { state: { state: 'stopped' } }))).toBe(false)
    expect(isLive(runEntry('r-1', { state: { state: 'paused', cause: 'ask_human' } }))).toBe(false)
  })

  test('the part has a live run when any of its runs is live', () => {
    const done = runEntry('r-2', { state: { state: 'ended', outcome: 'done' } })
    expect(hasLiveRun(changesOf([done, runEntry('r-1')]))).toBe(true)
    expect(hasLiveRun(changesOf([done]))).toBe(false)
    expect(hasLiveRun(changesOf([]))).toBe(false)
  })
})

describe('when there is nothing to show', () => {
  test('no run and no commit is empty, and says so in the words of the item', () => {
    expect(isEmpty(changesOf([]))).toBe(true)
    expect(NOTHING_YET).toBe('No changes yet.')
  })

  test('a run with no files is not empty, and neither is a commit alone', () => {
    expect(isEmpty(changesOf([runEntry('r-1')]))).toBe(false)
    expect(isEmpty(changesOf([], [commitEntry('abc1234', 'x', [])]))).toBe(false)
  })

  test('the live words are the item\'s', () => {
    expect(LIVE_WORDS).toBe('Live, as the agent edits')
  })
})

describe('a run in words', () => {
  test('its heading names the run and its state', () => {
    expect(runHeading(runEntry('r-12'))).toBe('Run r-12 · Working')
    const ended = runEntry('r-9', { state: { state: 'ended', outcome: 'done' } })
    expect(runHeading(ended)).toBe('Run r-9 · Finished: done')
    expect(runHeading(runEntry('r-4', { state: { state: 'failed' } }))).toBe('Run r-4 · Failed')
  })

  test('where it ran and how are the words the wire gives, in one line', () => {
    expect(whereWords({ place: "Run r-1's worktree", how: 'from where the agent phase began to where it ended' })).toBe(
      "Run r-1's worktree, from where the agent phase began to where it ended.",
    )
  })
})

describe('the counts of a list of files', () => {
  test('says how many files, and the lines added and removed, with a true minus sign', () => {
    expect(filesWords([file('a', 3, 1), file('b', 4, 0)], 0)).toBe('2 files, +7 −1')
  })

  test('says "1 file" for one', () => {
    expect(filesWords([file('a', 3, 1)], 0)).toBe('1 file, +3 −1')
  })

  test('a binary file adds nothing to the counts', () => {
    expect(filesWords([file('a', 3, 1), file('b', null, null)], 0)).toBe('2 files, +3 −1')
  })

  test('counts the files that were left out of the list', () => {
    expect(filesWords([file('a', 3, 1)], 4)).toBe('5 files, +3 −1 in the 1 listed')
  })

  test('has no counts for no files', () => {
    expect(filesWords([], 0)).toBe('No files')
  })
})

describe('a size and a commit in words', () => {
  test('sizes', () => {
    expect(sizeWords(0)).toBe('0 B')
    expect(sizeWords(999)).toBe('999 B')
    expect(sizeWords(2048)).toBe('2.0 KB')
    expect(sizeWords(4_500_000)).toBe('4.3 MB')
  })

  test('a commit by its first seven characters', () => {
    expect(shortCommit('abc1234def5678')).toBe('abc1234')
    expect(shortCommit('abc')).toBe('abc')
  })
})
