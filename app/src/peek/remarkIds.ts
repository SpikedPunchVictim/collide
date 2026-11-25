// Turns the words of a text that are ids of tasks into id links (ts-0189-fg; note 34 of the board mock), as a remark
// plugin for `Markdown`. A word links when a source's pattern matches it and `task.get` found it (`linked`, from the
// book). The words of a link, a fenced block and raw HTML are left as they are, so a link never holds a link, and an
// inline code span links only when the whole span is one id.
import { type Piece, splitIds } from './ids.ts'

/** The part of an mdast node this reads and writes; the plugin takes any tree, so no mdast types are needed. */
interface Node {
  type: string
  value?: string
  children?: Node[]
  data?: Record<string, unknown>
}

export interface RemarkIdsOptions {
  readonly linked: ReadonlySet<string>
  readonly patterns: readonly RegExp[]
}

/** Nodes whose text is not prose that may link: a link holds no link, a reference's text is a link's. */
const LEAVE = new Set(['link', 'linkReference', 'definition'])

/** An id as the element `Markdown` maps to `IdRef`. */
const idNode = (id: string): Node => ({
  type: 'collideId',
  data: { hName: 'collide-id', hProperties: { dataId: id } },
  children: [{ type: 'text', value: id }],
})

const nodeOf = (piece: Piece): Node => (typeof piece === 'string' ? { type: 'text', value: piece } : idNode(piece.id))

function linkIds(node: Node, { linked, patterns }: RemarkIdsOptions): void {
  if (node.children === undefined || LEAVE.has(node.type)) return
  const next: Node[] = []
  for (const child of node.children) {
    if (child.type === 'text' && child.value !== undefined) {
      next.push(...splitIds(child.value, linked, patterns).map(nodeOf))
    } else if (child.type === 'inlineCode' && child.value !== undefined) {
      const [only, ...rest] = splitIds(child.value, linked, patterns)
      if (only !== undefined && typeof only !== 'string' && rest.length === 0) {
        child.data = {
          ...child.data,
          hChildren: [
            {
              type: 'element',
              tagName: 'collide-id',
              properties: { dataId: only.id },
              children: [{ type: 'text', value: only.id }],
            },
          ],
        }
      }
      next.push(child)
    } else {
      linkIds(child, { linked, patterns })
      next.push(child)
    }
  }
  node.children = next
}

/** The plugin: with no linked word it leaves the tree alone. */
export function remarkIds(options: RemarkIdsOptions) {
  return (tree: Node): void => {
    if (options.linked.size > 0) linkIds(tree, options)
  }
}
