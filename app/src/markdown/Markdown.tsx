// Task, issue and agent text, as markdown that cannot run, fetch or navigate (ts-0183-ee; plan 0.9, R5). It renders to
// React elements and never to an HTML string, with no `rehype-raw`, so raw HTML in the text shows as text. Of the
// links, only `http:`, `https:` and `mailto:` are kept, and a kept link is a button that asks the link window (`links`)
// before anything opens: it has no `href`, so the webview has nothing to navigate to. An image is never loaded: it
// shows as its alt text and its address. A heading is a level-4 heading, so a body never changes the page's outline.
// A word that is the id of a task of the board is an id link, which peeks at the task (ts-0189-fg; `../peek/`).
import { type ComponentProps, type ReactNode, useContext, useEffect, useSyncExternalStore } from 'react'
import ReactMarkdown, { type Components } from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { cn } from '@/lib/utils'
import { useAskToOpen } from './ExternalLinks.tsx'
import { IdBookContext } from '../peek/context.ts'
import { IdRef } from '../peek/IdRef.tsx'
import { remarkIds } from '../peek/remarkIds.ts'
import { openableLink, schemeOf } from './links.ts'

/** What a refused link says, after its text. */
function refusedWords(href: string | undefined): string {
  const scheme = schemeOf(href)
  return scheme === null
    ? '(link not opened: it has no web address)'
    : `(link not opened: ${scheme} links are never opened)`
}

function Link({ href, children }: ComponentProps<'a'>) {
  const ask = useAskToOpen()
  // A link to a place in the same text (a footnote's) goes nowhere else, so it is only its text.
  if (href?.startsWith('#')) return <>{children}</>
  const url = openableLink(href)
  if (url === null) {
    return (
      <>
        {children} <span className="gone">{refusedWords(href)}</span>
      </>
    )
  }
  return (
    <button type="button" title={url} className="md-link" onClick={() => ask(url)}>
      {children}
    </button>
  )
}

function Image({ src, alt }: ComponentProps<'img'>) {
  const words = alt === undefined || alt === '' ? 'no description' : alt
  return <span className="gone">{`[image not loaded: ${words}, from ${src ?? 'nowhere'}]`}</span>
}

function Heading({ children }: { children?: ReactNode }) {
  return <h4>{children}</h4>
}

const COMPONENTS = {
  // The element `remarkIds` makes of a word that is the id of a task.
  'collide-id': IdRef,
  a: Link,
  img: Image,
  h1: Heading,
  h2: Heading,
  h3: Heading,
  h4: Heading,
  h5: Heading,
  h6: Heading,
} as Components

/** Only the two attributes the components above read keep their address; the others are made empty. */
function keepAddresses(url: string, key: string): string {
  return key === 'href' || key === 'src' ? url : ''
}

export interface MarkdownProps {
  readonly text: string
  readonly className?: string
}

const NO_WORDS: ReadonlySet<string> = new Set()
const noWords = () => NO_WORDS
const noListener = () => () => {}

/**
 * The words of a text that are ids of the board's tasks (ts-0189-fg; note 34): each word a source's pattern matches is
 * asked of `task.get` once, and a word that found a task is drawn as an id link. With no book there are none.
 */
function useLinkedIds(text: string) {
  const book = useContext(IdBookContext)
  useEffect(() => {
    if (book !== null) void book.ensure(book.candidates(text))
  }, [book, text])
  const linked = useSyncExternalStore(book?.subscribe ?? noListener, book?.linked ?? noWords, book?.linked ?? noWords)
  return { linked, patterns: book?.patterns ?? [] }
}

export function Markdown({ text, className }: MarkdownProps) {
  const { linked, patterns } = useLinkedIds(text)
  return (
    <div className={cn('md', className)}>
      <ReactMarkdown
        remarkPlugins={[remarkGfm, [remarkIds, { linked, patterns }]]}
        urlTransform={keepAddresses}
        components={COMPONENTS}
      >
        {text}
      </ReactMarkdown>
    </div>
  )
}
