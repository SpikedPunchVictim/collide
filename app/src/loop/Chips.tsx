// The two `claude` version chips (0.3, Two version chips): the version in use, marked in words when it is outside the
// tested range, and the latest published version. What each tooltip says is in `words.ts` (`versionChips`).
import { Badge } from '@/components/ui/badge'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'
import type { Chip } from './words.ts'

function ChipView({ chip }: { readonly chip: Chip }) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Badge asChild variant={chip.flag === null ? 'secondary' : 'destructive'}>
          <button type="button" aria-label={chip.name}>
            <span>{chip.text}</span>
            {chip.flag === null ? null : <span>{chip.flag}</span>}
          </button>
        </Badge>
      </TooltipTrigger>
      <TooltipContent>{chip.tooltip === '' ? chip.name : chip.tooltip}</TooltipContent>
    </Tooltip>
  )
}

export interface ChipsProps {
  readonly inUse: Chip
  /** Null hides the published chip. */
  readonly published: Chip | null
}

export function Chips({ inUse, published }: ChipsProps) {
  return (
    <div className="flex flex-wrap items-center gap-2" role="group" aria-label="Claude versions">
      <ChipView chip={inUse} />
      {published === null ? null : <ChipView chip={published} />}
    </div>
  )
}
