// The shapes the drawer holds, named from the generated protocol types (0.6), as `field/types.ts` does.
import type { TaskGetResultOutput } from '../owner/generated.ts'

/** What `task.get` gave for one task. */
export type Got = TaskGetResultOutput
export type GotCopy = Got['copies'][number]
export type GotLink = Got['links'][number]

/** The board the drawer is for, as the details name it. The line is null when the board lists none. */
export interface BoardNote {
  readonly name: string
  readonly line: string | null
}
