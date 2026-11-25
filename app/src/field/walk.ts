// The run order and the graph over what the daemon walked (ts-0187-hb; 0.10, Views, `closure`; 0.11, Field; note 11 of
// the board mock). The daemon does the walk: `ViewReport.steps` are its layers and `ViewReport.cycle` the tasks that
// wait on each other. Nothing here walks a link to find an order. This module only names the goals to pick, turns the
// steps' keys into tasks, and places the tasks and their lines, so each rule is tested with no page.
import type { ViewInput } from '../owner/generated.ts'
import { sourceOf } from './model.ts'
import type { FieldData, Task, ViewEntry, ViewReport } from './types.ts'

/** Where the graph puts things, in pixels: a column is `W` wide and a row `H` high, a task `nw` by `nh`. */
export const GEOMETRY = { W: 236, H: 68, top: 34, left: 16, nw: 200, nh: 52 }

/** How far before its task a line that changes row turns to run level into it. */
const LEAD = 12

/** The link types a closure follows: `DEPENDENCY_LINKS` of `core/src/view_expr.rs`, so the lines match the steps. */
const DEPENDENCY_LINKS: readonly string[] = ['blocked-by', 'depends_on']

const isOpen = (task: Task) => task.state.category !== 'done' && task.state.category !== 'dropped'

/** A collection the picker offers as a goal: one with a task that is open. */
export interface Goal {
  readonly id: string
  readonly title: string
  readonly open: number
}

/** The tasks of the board, the shown ones first, so that a key the view lists is read from its own copy. */
function everyTask(data: FieldData): readonly Task[] {
  return [...data.tasks, ...data.corpus]
}

/** The collections that have an open task, in the order the sources list them, with how many tasks that is. */
export function goalsOf(data: FieldData): Goal[] {
  const tasks = data.corpus.length > 0 ? data.corpus : data.tasks
  const goals: Goal[] = []
  for (const collection of data.collections) {
    const members = tasks.filter((task) => isOpen(task) && task.collections.includes(collection.id))
    const open = new Set(members.map((task) => task.key)).size
    if (open > 0) goals.push({ id: collection.id, title: collection.title, open })
  }
  return goals
}

/** What a goal is called: a collection by its title, a task by its title, and a key that is neither as it is. */
export function goalTitle(data: FieldData, goal: string): string {
  const collection = data.collections.find((each) => each.id === goal)
  if (collection !== undefined) return collection.title
  return everyTask(data).find((task) => task.key === goal)?.title ?? goal
}

/**
 * The view as the daemon is asked for a goal's walk: the shown view with the closure added, or a view of nothing but
 * the closure for the whole board. The view's own rule and members stay, so the walk is of what the view holds.
 */
export function inlineView(entry: ViewEntry | undefined, goal: string): ViewInput {
  return { ...(entry?.view ?? { name: 'All tasks' }), closure: { goal, follow: 'depends_on' } }
}

/** The steps of a walk as tasks. A key the board does not list is left out, and so is a step left with nothing. */
export function stepsOf(report: ViewReport | null, data: FieldData): Task[][] {
  if (report === null) return []
  const tasks = new Map<string, Task>()
  for (const task of everyTask(data)) {
    if (!tasks.has(task.key)) tasks.set(task.key, task)
  }
  return report.steps.map((step) => step.flatMap((key) => tasks.get(key) ?? [])).filter((step) => step.length > 0)
}

const plural = (count: number, one: string, many: string) => `${count} ${count === 1 ? one : many}`

/** How many open tasks are walked in how many steps: "5 open tasks in 4 steps." */
export function walkWords(steps: readonly (readonly Task[])[]): string {
  const tasks = steps.reduce((total, step) => total + step.length, 0)
  return `${plural(tasks, 'open task', 'open tasks')} in ${plural(steps.length, 'step', 'steps')}.`
}

export interface GraphNode {
  readonly task: Task
  /** The column, counted from 0. */
  readonly step: number
  readonly x: number
  readonly y: number
}

/** A line from a blocker to the task it holds up. */
export interface GraphEdge {
  /** The blocker's key. */
  readonly from: string
  /** The key of the task that waits. */
  readonly to: string
  /** The path, as the `d` of an svg `path`. */
  readonly d: string
}

export interface Graph {
  readonly nodes: readonly GraphNode[]
  readonly edges: readonly GraphEdge[]
  readonly width: number
  readonly height: number
}

/** The name of a line, which a hover is matched against. */
export const edgeId = (edge: { readonly from: string; readonly to: string }) => `${edge.from}>${edge.to}`

/**
 * Places each step in a column, and the tasks of a step one under another, ordered by where their blockers sit so that
 * lines cross less. A line runs from each blocker to what it holds up when both are shown. A link to a task that is
 * not shown draws none, and neither does a link between two tasks of a cycle.
 */
export function graphOf(steps: readonly (readonly Task[])[], cycle: readonly string[]): Graph {
  const { W, H, top, left, nw, nh } = GEOMETRY
  const onCycle = new Set(cycle)
  // The shown tasks by key, and by `source:alias`, which is how a link names a task by another of its names.
  const shown = new Map<string, Task>()
  for (const task of steps.flat()) {
    if (!shown.has(task.key)) shown.set(task.key, task)
  }
  for (const task of [...shown.values()]) {
    for (const alias of task.aliases) {
      const name = `${sourceOf(task.key)}:${alias}`
      if (!shown.has(name)) shown.set(name, task)
    }
  }
  /** The keys of the shown tasks that a task waits on. */
  const blockers = (task: Task): string[] => {
    const keys = new Set<string>()
    for (const link of task.links) {
      if (!DEPENDENCY_LINKS.includes(link.type) || link.blocking === false) continue
      const found = shown.get(link.to.includes(':') ? link.to : `${sourceOf(task.key)}:${link.to}`)
      if (found !== undefined && found.key !== task.key) keys.add(found.key)
    }
    return [...keys]
  }
  const nodes: GraphNode[] = []
  const placed = new Map<string, GraphNode>()
  steps.forEach((step, index) => {
    const meanY = (task: Task) => {
      const ys = blockers(task).flatMap((key) => placed.get(key)?.y ?? [])
      return ys.length === 0 ? 0 : ys.reduce((sum, y) => sum + y, 0) / ys.length
    }
    const ordered = [...step].sort((a, b) => meanY(a) - meanY(b))
    ordered.forEach((task, row) => {
      const node = { task, step: index, x: left + index * W, y: top + row * H }
      nodes.push(node)
      placed.set(task.key, node)
    })
  })
  const edges: GraphEdge[] = []
  for (const node of nodes) {
    for (const key of blockers(node.task)) {
      const blocker = placed.get(key)
      if (blocker === undefined || (onCycle.has(key) && onCycle.has(node.task.key))) continue
      const [x1, y1, x2, y2] = [blocker.x + nw, blocker.y + nh / 2, node.x, node.y + nh / 2]
      const d = y1 === y2 ? `M${x1} ${y1} L${x2} ${y2}` : `M${x1} ${y1} L${x2 - LEAD} ${y2} L${x2} ${y2}`
      edges.push({ from: key, to: node.task.key, d })
    }
  }
  if (steps.length === 0) return { nodes, edges, width: 0, height: 0 }
  const rows = Math.max(...steps.map((step) => step.length))
  return { nodes, edges, width: left * 2 + (steps.length - 1) * W + nw, height: top + rows * H + 6 }
}

/** What a hover or focus on a task lights: the lines into and out of it, and the tasks at their ends. */
export function lightOf(graph: Graph, key: string | null): { hot: Set<string>; near: Set<string> } {
  const hot = new Set<string>()
  const near = new Set<string>(key === null ? [] : [key])
  if (key === null) return { hot, near }
  for (const edge of graph.edges) {
    if (edge.from !== key && edge.to !== key) continue
    hot.add(edgeId(edge))
    near.add(edge.from)
    near.add(edge.to)
  }
  return { hot, near }
}
