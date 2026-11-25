// A card of the Field (ts-0181-ft; notes 5 to 7 of the board mock): the title, the short id and stage, and the chips.
// The board's columns and the run order draw the same card, so a task is read the same in each.
import { Badge } from '@/components/ui/badge'
import { cn } from '@/lib/utils'
import { DOT, TEXT } from './colours.ts'
import type { Card, Chip } from './model.ts'

export interface CardButtonProps {
  readonly card: Card
  readonly selected: boolean
  /** The card changed in a read just now: it flashes once (`field-flash` in `index.css`), and says nothing. */
  readonly flash: boolean
  /** The card is of the epic that is picked (ts-0216-ve): it is lit, and says nothing. */
  readonly lit?: boolean
  readonly onSelect: (id: string) => void
}

export function CardButton({ card, selected, flash, lit = false, onSelect }: CardButtonProps) {
  return (
    <button
      type="button"
      data-card={card.id}
      data-flash={flash ? 'true' : undefined}
      data-lit={lit ? 'true' : undefined}
      aria-current={selected}
      onClick={() => onSelect(card.id)}
      className={cn(
        'flex w-full cursor-pointer flex-col gap-1.5 rounded-[10px] border bg-card px-3 py-2.5 text-left shadow-sm',
        'hover:border-input focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
        // A card being worked on has the lit border (note 5).
        card.category === 'active' && 'border-c-active ring-1 ring-c-active',
        selected && 'outline-2 outline-offset-1 outline-ring',
        lit && 'border-primary/60 bg-primary/10',
        flash && 'field-flash',
      )}
    >
      <span
        className={cn('leading-snug font-medium', card.category === 'dropped' && 'text-muted-foreground line-through')}
      >
        {card.title}
      </span>
      <span className="text-xs text-muted-foreground">
        {card.shortId}
        {card.stage === null ? null : ` · ${card.stage}`}
      </span>
      {card.chips.length === 0 ? null : (
        <span className="flex flex-wrap gap-1.5">
          {card.chips.map((chip) => (
            <ChipView key={chip.id} chip={chip} />
          ))}
        </span>
      )}
    </button>
  )
}

function ChipView({ chip }: { chip: Chip }) {
  const style =
    chip.id === 'status'
      ? TEXT[chip.category!]
      : chip.id === 'divergence'
        ? 'border-waiting/40 text-waiting'
        : chip.id === 'collision'
          ? 'border-destructive/40 text-destructive'
          : 'text-muted-foreground'
  return (
    <Badge variant="outline" className={cn('font-normal', style)}>
      {chip.id === 'status' ? <span className={cn('size-2 rounded-full', DOT[chip.category!])} /> : null}
      {chip.id === 'divergence' ? <span aria-hidden="true">↗</span> : null}
      {chip.text}
    </Badge>
  )
}
