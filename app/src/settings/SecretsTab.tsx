// The Secrets tab of Settings (ts-0193-gk): each secret the board's packs ask for, with whether the keychain holds it.
// It never shows a value: the daemon sends none. "Set…" and "Replace…" ask `onSet` to open the window that takes one.
import { Button } from '@/components/ui/button'
import { type Listing, type SecretRow, secrets } from './model.ts'

export interface SecretsTabProps {
  readonly listing: Listing
  readonly onSet: (row: SecretRow) => void
}

export function SecretsTab({ listing, onSet }: SecretsTabProps) {
  const rows = secrets(listing)
  return (
    <div className="flex flex-col gap-3 p-4">
      <p className="max-w-[80ch] text-muted-foreground">
        Secrets go into your Mac&apos;s keychain. Collide never shows them again; it shows only whether one is set.
      </p>
      {rows.length === 0 ? <p className="text-muted-foreground">No pack of this board asks for a secret.</p> : null}
      <table className="w-full text-left">
        <tbody>
          {rows.map((row) => (
            <tr key={row.key} className="border-b align-top">
              <td className="py-2 pr-4">
                <b>{row.name}</b>
                <span className="block text-muted-foreground">{row.pack} asks for it</span>
                {row.problems.map((message) => (
                  <span key={message} className="block text-xs text-destructive">
                    {message}
                  </span>
                ))}
              </td>
              <td className="py-2 pr-4">
                <span className="rounded-4xl bg-muted px-2 py-0.5 text-xs">{row.set === true ? 'Set' : 'Not set'}</span>
              </td>
              <td className="py-2 text-right">
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => onSet(row)}
                  aria-label={`${row.set === true ? 'Replace' : 'Set'} ${row.name}`}
                >
                  {row.set === true ? 'Replace…' : 'Set…'}
                </Button>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
