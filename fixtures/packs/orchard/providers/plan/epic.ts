import { TASK_ID_IN_TEXT } from './ids.ts'

/** One ordered group of an epic: a slice or a stream, with the short ids of its tasks in the order of the table. */
export interface EpicGroup {
  name: string
  shortIds: string[]
}

export interface EpicTables {
  /** Every task id the epic's tables name, in the order they appear: the generated list, the slices and the streams. */
  members: string[]
  groups: EpicGroup[]
}

const GENERATED_BEGIN = /^<!--\s*BEGIN GENERATED tasks\s*-->\s*$/
const GENERATED_END = /^<!--\s*END GENERATED\s*-->\s*$/
const FENCE = /^\s*(```|~~~)/
const HEADING = /^(#{1,6})\s+(.*?)\s*#*\s*$/
const SLICE_HEADING = /^Slice \d+\b/
const STREAMS_HEADING = /\bstreams?\b/i
const SEPARATOR_CELL = /^:?-+:?$/

const cellsOf = (line: string): string[] =>
  line.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map((cell) => cell.trim())

const idsIn = (text: string): string[] => text.match(TASK_ID_IN_TEXT) ?? []

const once = (ids: string[]): string[] => [...new Set(ids)]

/** The data rows of one table: its header row and the separator row under it are left out. */
function dataRows(block: string[]): string[][] {
  const cells = block.map(cellsOf)
  const hasSeparator = cells[1]?.every((cell) => SEPARATOR_CELL.test(cell)) ?? false
  return hasSeparator ? cells.slice(2) : []
}

type Section = 'slice' | 'streams' | 'other'

/**
 * Reads an epic file's body for its tasks. Three things count: the generated list of tasks (the first column of its
 * rows), each table under a `Slice N` heading (one group per slice, from the second column), and each row of the
 * table under a heading that says "streams" (one group per row, from the second column). A table under any other
 * heading, a line in a code fence and prose name tasks without joining a group or the epic.
 */
export function readEpicTables(body: string): EpicTables {
  const members: string[] = []
  const groups: EpicGroup[] = []
  let section: Section = 'other'
  let sliceGroup: EpicGroup | null = null
  let inFence = false
  let inGenerated = false
  let table: string[] = []

  const endTable = (): void => {
    if (table.length === 0) return
    const rows = dataRows(table)
    table = []
    if (inGenerated) {
      for (const row of rows) members.push(...idsIn(row[0] ?? ''))
    } else if (section === 'slice' && sliceGroup !== null) {
      for (const row of rows) sliceGroup.shortIds.push(...idsIn(row[1] ?? ''))
    } else if (section === 'streams') {
      for (const row of rows) {
        const name = (row[0] ?? '').replace(/\*\*/g, '').trim()
        const shortIds = once(idsIn(row[1] ?? ''))
        if (name !== '') groups.push({ name, shortIds })
        members.push(...shortIds)
      }
    }
  }

  for (const line of body.split(/\r?\n/)) {
    if (FENCE.test(line)) {
      endTable()
      inFence = !inFence
      continue
    }
    if (inFence) continue
    if (GENERATED_BEGIN.test(line)) {
      endTable()
      inGenerated = true
      continue
    }
    if (GENERATED_END.test(line)) {
      endTable()
      inGenerated = false
      continue
    }
    const heading = inGenerated ? null : HEADING.exec(line)
    if (heading !== null) {
      endTable()
      const text = heading[2] as string
      if (SLICE_HEADING.test(text)) {
        section = 'slice'
        sliceGroup = { name: text, shortIds: [] }
        groups.push(sliceGroup)
      } else {
        section = STREAMS_HEADING.test(text) ? 'streams' : 'other'
        sliceGroup = null
      }
      continue
    }
    if (line.trimStart().startsWith('|')) table.push(line)
    else endTable()
  }
  endTable()

  for (const group of groups) group.shortIds = once(group.shortIds)
  // Slice tables name tasks for the slice; the epic holds them too.
  for (const group of groups) members.push(...group.shortIds)
  return { members: once(members), groups }
}
