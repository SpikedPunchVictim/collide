// A page for a tab, or the Settings page, that is not built yet (0.11: "A tab whose stage has not shipped says which
// stage brings it"). It says what the page will hold and when it arrives, in the board mock's words (`laterHtml`).
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'

export interface LaterPageProps {
  readonly title: string
  /** What the page will show, as a sentence. */
  readonly what: string
  /** When it arrives, as a sentence. */
  readonly arrives: string
  /** Given, the page offers the Loop page, which shows a run until the real page arrives. */
  readonly onOpenLoop?: () => void
}

export function LaterPage({ title, what, arrives, onOpenLoop }: LaterPageProps) {
  return (
    <Card aria-label={title} className="mx-auto w-full max-w-xl items-center px-6 text-center">
      <h1 className="text-xl font-semibold">{title}</h1>
      <p>{what}</p>
      <p className="text-muted-foreground">{arrives}</p>
      {onOpenLoop === undefined ? null : (
        <Button variant="outline" onClick={onOpenLoop}>
          Open the Loop page
        </Button>
      )}
    </Card>
  )
}
