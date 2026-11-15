// "Why is this asked?" (0.11, Queue). The six sections are the fields of one record that Collide and the pack fill. A
// section the data lacks shows as "Not available in this view" and is never hidden: the gap shows what people want and
// cannot get. Collide's facts come first and the agent's own words last, marked unverified because an agent fed a
// hostile issue can argue for anything (0.9). Every text is shown as text, never as markup.
import { Badge } from '@/components/ui/badge'
import type { ContextRecord } from '../types.ts'

export const NOT_AVAILABLE = 'Not available in this view'

export interface ContextSectionDef {
  readonly key: keyof ContextRecord
  readonly title: string
  /** The agent's words are not Collide's facts, so they carry a mark. */
  readonly unverified?: boolean
}

/** The six sections, in the order the plan lists them. */
export const CONTEXT_SECTIONS: readonly ContextSectionDef[] = [
  { key: 'why_it_asks', title: 'Why it asks' },
  { key: 'what_it_would_do', title: 'What it would do' },
  { key: 'task', title: 'The task' },
  { key: 'what_led_here', title: 'What led here' },
  { key: 'history', title: 'History' },
  { key: 'agents_own_words', title: "The agent's own words", unverified: true },
]

export interface ContextViewProps {
  /** A key left out reads the same as `not_available`: both show the gap. */
  readonly record: Partial<ContextRecord>
}

export function ContextView({ record }: ContextViewProps) {
  return (
    <div className="flex flex-col gap-4">
      {CONTEXT_SECTIONS.map(({ key, title, unverified }) => {
        const section = record[key]
        return (
          <section key={key} data-section={key} className="flex flex-col gap-1">
            <h4 className="flex items-center gap-2 text-sm font-medium">
              {title}
              {unverified ? <Badge variant="outline">Unverified</Badge> : null}
            </h4>
            {section?.status === 'filled' ? (
              <p className="text-sm whitespace-pre-wrap">{section.text}</p>
            ) : (
              <p className="text-sm text-muted-foreground italic">{NOT_AVAILABLE}</p>
            )}
          </section>
        )
      })}
    </div>
  )
}
