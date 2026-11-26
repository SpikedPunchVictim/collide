// What a person is shown before a pack is added (0.9, Per pack source; the mock's `trustModal`): where it is from, what
// it runs, and what it needs. `config.pack_add` without `confirm` sends it as the pack's `review`. It is the window's
// body, so a test draws it to text.
import type { Pack } from './model.ts'

type Review = NonNullable<Pack['review']>

export interface PackReviewProps {
  readonly source: string
  /** A `dev:` folder: 0.9 trusts it per folder, so a change to it does not ask again. */
  readonly dev: boolean
  readonly review: Review
}

export function PackReview({ source, dev, review }: PackReviewProps) {
  const runs = [
    ...review.hooks.map((line) => ({ kind: 'hook', ...line })),
    ...review.providers.map((line) => ({ kind: 'provider', ...line })),
    ...review.observers.map((line) => ({ kind: 'observer', ...line })),
  ]
  return (
    <div className="flex flex-col gap-2">
      <p>
        <b>{review.name}</b> is a pack. A pack runs code on this Mac, with your permissions, to read its tasks.
      </p>
      <ul className="flex flex-col gap-1">
        <li>
          From: <code>{source}</code>
        </li>
        {review.version == null ? null : <li>Version: {review.version}</li>}
        {review.commit == null ? null : (
          <li>
            Commit: <code>{review.commit.slice(0, 12)}</code>
          </li>
        )}
        <li>
          It runs:{' '}
          {runs.length === 0 ? (
            'nothing of its own'
          ) : (
            <ul className="ml-4 list-disc">
              {runs.map((line) => (
                <li key={`${line.kind}:${line.key}`}>
                  {line.kind} <code>{line.key}</code>: <code>{line.script}</code>
                </li>
              ))}
            </ul>
          )}
        </li>
        {review.requires.length === 0 ? null : <li>It needs: {review.requires.join(', ')}</li>}
      </ul>
      {dev ? (
        <p className="text-xs text-muted-foreground">
          It is a folder being developed, so it is trusted as a folder (now at tree{' '}
          <code>{review.tree.slice(0, 12)}</code>). A change to it does not ask again; each run that loads it is marked
          dev and records its tree.
        </p>
      ) : (
        <p className="text-xs text-muted-foreground">
          It is trusted as it is now (tree <code>{review.tree.slice(0, 12)}</code>). A change to it asks again.
        </p>
      )}
    </div>
  )
}
