// The General tab of Settings (ts-0193-gk; the mock's `settingsHtml`): one table, each setting with its key beneath,
// the value in use, and the layer it was set in as a badge. A value another layer set and this one replaced shows
// struck out beside it. A problem of a setting shows on its row in the daemon's words, and the problems that name no
// row show above the table. Details lists the files the daemon reads, as it reports them.
import { cn } from '@/lib/utils'
import { type Layer, type Listing, files, general } from './model.ts'

const BADGE: Readonly<Record<Layer, string>> = {
  default: 'bg-muted text-muted-foreground',
  user: 'bg-primary/10 text-primary',
  repo: 'bg-secondary text-secondary-foreground',
  profile: 'bg-accent text-accent-foreground',
}

export function GeneralTab({ listing }: { readonly listing: Listing }) {
  const { rows, loose } = general(listing)
  const listed = files(listing)
  return (
    <div className="flex flex-col gap-3 p-4">
      <p className="max-w-[80ch] text-muted-foreground">
        Each setting shows the value in use and where it came from. This repo&apos;s file wins over yours; a
        pack&apos;s wins over the default.
      </p>
      {loose.length === 0 ? null : (
        <ul aria-label="Problems with the settings" className="flex flex-col gap-1 text-destructive">
          {loose.map((problem) => (
            <li key={`${problem.layer}:${problem.key}:${problem.message}`}>{problem.message}</li>
          ))}
        </ul>
      )}
      <div className="overflow-x-auto">
        <table className="w-full text-left">
          <thead>
            <tr className="border-b text-muted-foreground">
              <th className="py-2 pr-4 font-medium">Setting</th>
              <th className="py-2 pr-4 font-medium">Value in use</th>
              <th className="py-2 font-medium">Set in</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={row.key} className="border-b align-top">
                <td className="py-2 pr-4">
                  <b>{row.name}</b>
                  {row.name === row.key ? null : (
                    <>
                      <br />
                      <span className="font-mono text-xs text-muted-foreground">{row.key}</span>
                    </>
                  )}
                </td>
                <td className="py-2 pr-4">
                  {row.value}
                  {row.struck === null ? null : (
                    <>
                      <br />
                      <s className="text-muted-foreground">{row.struck.value}</s>{' '}
                      <span className="text-xs text-muted-foreground">{row.struck.words}</span>
                    </>
                  )}
                  {row.problems.map((message) => (
                    <p key={message} className="mt-1 text-xs text-destructive">
                      {message}
                    </p>
                  ))}
                </td>
                <td className="py-2">
                  <span className={cn('rounded-4xl px-2 py-0.5 text-xs font-medium', BADGE[row.layer])}>
                    {row.layerWord}
                  </span>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <details>
        <summary className="cursor-pointer text-muted-foreground">Details for engineers: the files</summary>
        <dl className="mt-2 grid grid-cols-[max-content_1fr] gap-x-4 gap-y-1">
          <dt>Default</dt>
          <dd>the settings schema</dd>
          {listed.map((file) => (
            <FileLine key={`${file.label}:${file.path}`} {...file} />
          ))}
          <dt>Run profile</dt>
          <dd>applies inside a run only</dd>
        </dl>
      </details>
    </div>
  )
}

function FileLine({ label, path, exists }: { label: string; path: string; exists: boolean }) {
  return (
    <>
      <dt>{label}</dt>
      <dd>
        <span className="font-mono">{path}</span>
        {exists ? null : <span className="text-muted-foreground"> (no file yet)</span>}
      </dd>
    </>
  )
}
