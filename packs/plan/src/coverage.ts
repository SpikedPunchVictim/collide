import type { CoverageInput } from '@collide/pack-sdk'
import { parseFeatureTarget } from './feature-target.ts'
import { IMPLEMENTS, INCLUDES, PROPOSAL, stateOf } from './item.ts'
import { featuresOf } from './links.ts'
import type { FoundItem } from './scan.ts'

const TASK = 'task'

/** Whether `found` is a task that is not dropped: only such a task covers a feature or is named. */
const isLiveTask = (found: FoundItem): boolean =>
  found.item.kind === TASK && stateOf(found.item.kind, found.item.status).category !== 'dropped'

/** The feature ids of `proposal` that the `implements` links of `item` name, in link order. */
function implemented(item: FoundItem, proposal: string): string[] {
  const named: string[] = []
  for (const link of item.item.links) {
    if (link.type !== IMPLEMENTS) continue
    const target = parseFeatureTarget(link.to)
    if (target.ok && target.item === proposal) named.push(target.feature)
  }
  return named
}

/** Whether `task` implements a feature that exists: a feature of a proposal that is in `items` and has it. */
function implementsAFeature(task: FoundItem, items: readonly FoundItem[]): boolean {
  return task.item.links.some((link) => {
    if (link.type !== IMPLEMENTS) return false
    const target = parseFeatureTarget(link.to)
    if (!target.ok) return false
    const found = items.find((candidate) => candidate.item.id === target.item)
    if (found === undefined || found.item.kind !== PROPOSAL) return false
    return featuresOf(found).some((feature) => feature.id === target.feature)
  })
}

/**
 * What a proposal's features and tasks cover (0.10, "A proposal's quality parts"), derived from the scan and never
 * written. `featuresWithoutTask` are its features, in order, that no task implements: a task implements a feature when
 * it has an `implements` link to `<proposal>#F<n>`, whether or not the proposal includes it, and a dropped task
 * covers nothing. `tasksWithoutFeature` are the ids of the tasks the proposal includes, in the order of its links, that are not
 * dropped and implement no feature that exists. Only a task is counted: an included question, spike or epic never is,
 * and nor is an id that no item has. A proposal with no features and no includes has no coverage, and nor has any
 * other kind.
 */
export function coverageOf(
  found: FoundItem,
  features: readonly string[],
  items: readonly FoundItem[],
): CoverageInput | null {
  if (found.item.kind !== PROPOSAL) return null
  const included = [...new Set(found.item.links.filter((link) => link.type === INCLUDES).map((link) => link.to))]
  if (features.length === 0 && included.length === 0) return null

  const covered = new Set<string>()
  for (const candidate of items) {
    if (!isLiveTask(candidate)) continue
    for (const feature of implemented(candidate, found.item.id)) covered.add(feature)
  }
  const tasksWithoutFeature = included.filter((id) => {
    const task = items.find((candidate) => candidate.item.id === id)
    if (task === undefined || !isLiveTask(task)) return false
    return !implementsAFeature(task, items)
  })
  return { featuresWithoutTask: features.filter((feature) => !covered.has(feature)), tasksWithoutFeature }
}
