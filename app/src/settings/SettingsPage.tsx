// The Settings page (ts-0193-gk; note 12 of the board mock): "Settings for <board>", Back to the board, and four
// tabs. General is the table of every setting with its layer; Packs lists the board's packs and adds one; Secrets
// shows Set or Not set and sets one into the keychain; Agents is Stage 6b's. The page reads `config.get` (with its
// problems in the answer, so a bad value shows on its row) and `config.pack_list` each time it opens, and again after
// a change.
import { type ReactNode, useCallback, useEffect, useState } from 'react'
import { Button } from '@/components/ui/button'
import { Card } from '@/components/ui/card'
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs'
import type { SourceDecl } from '../field/types.ts'
import { type OwnerClient, toOwnerError } from '../owner/index.ts'
import { AddPackWindow } from './AddPackWindow.tsx'
import { GeneralTab } from './GeneralTab.tsx'
import { PacksTab } from './PacksTab.tsx'
import { SecretWindow } from './SecretWindow.tsx'
import { SecretsTab } from './SecretsTab.tsx'
import type { Listing, Pack, SecretRow } from './model.ts'

export interface SettingsPageProps {
  readonly owner: OwnerClient
  /** The board the settings are for. Null when none is open. */
  readonly boardId: string | null
  readonly boardName: string | null
  /** The Field's sources, which say how many statuses a pack's tasks have. */
  readonly sources: readonly SourceDecl[]
  readonly onBack: () => void
}

/** One read: still going, or what came, or the daemon's words for why not. */
type Read<T> = { readonly state: 'loading' } | { readonly state: 'ready'; readonly value: T } | Failed
interface Failed {
  readonly state: 'failed'
  readonly message: string
}

const LOADING = { state: 'loading' } as const

async function read<T>(ask: () => Promise<T>): Promise<Read<T>> {
  try {
    return { state: 'ready', value: await ask() }
  } catch (failure) {
    return { state: 'failed', message: toOwnerError(failure).message }
  }
}

export function SettingsPage({ owner, boardId, boardName, sources, onBack }: SettingsPageProps) {
  const [listing, setListing] = useState<Read<Listing>>(LOADING)
  const [packs, setPacks] = useState<Read<readonly Pack[]>>(LOADING)
  const [secret, setSecret] = useState<SecretRow | null>(null)
  const [adding, setAdding] = useState(false)

  const load = useCallback(async () => {
    if (boardId === null) return
    const [got, listed] = await Promise.all([
      read(() => owner.rpc('config.get', { board: boardId, with_problems: true })),
      read(async () => (await owner.rpc('config.pack_list', { board: boardId })).packs),
    ])
    setListing(got)
    setPacks(listed)
  }, [owner, boardId])
  useEffect(() => {
    void load()
  }, [load])

  // The value is passed through and never kept. The daemon's answer to a refusal is shown as it is.
  const saveSecret = async (row: SecretRow, value: string): Promise<string | null> => {
    try {
      await owner.rpc('config.set', { board: boardId, key: row.key, value, layer: 'user', secret: true })
    } catch (failure) {
      return toOwnerError(failure).message
    }
    void load()
    return null
  }

  return (
    <Card aria-label="Settings" className="mx-auto w-full max-w-5xl gap-0 p-0">
      <div className="flex items-center gap-3 border-b px-4 py-3">
        <h1 className="text-xl font-semibold">Settings</h1>
        {boardName === null ? null : <span className="text-muted-foreground">{`for ${boardName}`}</span>}
        <span className="grow" />
        <Button variant="outline" size="sm" onClick={onBack}>
          Back to the board
        </Button>
      </div>
      {boardId === null ? (
        <p className="p-4 text-muted-foreground">Open a board to see its settings.</p>
      ) : (
        <Tabs defaultValue="general" className="gap-0">
          <TabsList variant="line" aria-label="Settings pages" className="px-4">
            <TabsTrigger value="general">General</TabsTrigger>
            <TabsTrigger value="packs">Packs</TabsTrigger>
            <TabsTrigger value="secrets">Secrets</TabsTrigger>
            <TabsTrigger value="agents">Agents</TabsTrigger>
          </TabsList>
          <TabsContent value="general">
            <Reading what={listing}>{(value) => <GeneralTab listing={value} />}</Reading>
          </TabsContent>
          <TabsContent value="packs">
            <Reading what={packs}>
              {(value) => <PacksTab packs={value} sources={sources} onAdd={() => setAdding(true)} />}
            </Reading>
          </TabsContent>
          <TabsContent value="secrets">
            <Reading what={listing}>{(value) => <SecretsTab listing={value} onSet={setSecret} />}</Reading>
          </TabsContent>
          <TabsContent value="agents">
            <p className="p-4 text-muted-foreground">This page arrives with the in-app agent, in Stage 6b.</p>
          </TabsContent>
        </Tabs>
      )}
      {secret === null ? null : (
        <SecretWindow
          name={secret.name}
          onSave={(value) => saveSecret(secret, value)}
          onClosed={() => setSecret(null)}
        />
      )}
      {adding ? <AddPackWindow owner={owner} onAdded={() => void load()} onClosed={() => setAdding(false)} /> : null}
    </Card>
  )
}

/** A read's page when it has come, and its words while it is on the way or when it failed. */
function Reading<T>({ what, children }: { what: Read<T>; children: (value: T) => ReactNode }) {
  if (what.state === 'loading') return <p className="p-4 text-muted-foreground">Reading…</p>
  if (what.state === 'failed') {
    return (
      <p role="alert" className="whitespace-pre-wrap p-4 text-destructive">
        {what.message}
      </p>
    )
  }
  return children(what.value)
}
