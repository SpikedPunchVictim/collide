import { parseFeatures } from './features.ts'
import type { Feature } from './features.ts'
import { parseFeatureTarget } from './feature-target.ts'
import { PROPOSAL, targetOf } from './item.ts'
import type { ItemLink } from './item.ts'
import type { FoundItem } from './scan.ts'

/** The features of a proposal, read from its body. */
export const featuresOf = (found: FoundItem): Feature[] => parseFeatures(found.body, found.bodyLine).features

/** `F1 (Trust prompt), F2 (Pack lock)`, or what to say when there are none. */
export const describeFeatures = (features: Feature[]): string =>
  features.length === 0 ? 'it has no features' : `its features are ${features.map((feature) => `${feature.id} (${feature.title})`).join(', ')}`

/**
 * Whether a link points at nothing: an item that was not read, or, for a link type whose target is a feature, anything but
 * a feature that its proposal has. A broken link is counted, never dropped.
 */
export function isBrokenLink(link: ItemLink, byId: ReadonlyMap<string, FoundItem>): boolean {
  if (targetOf(link.type) === 'item') return !byId.has(link.to)
  const target = parseFeatureTarget(link.to)
  if (!target.ok) return true
  const found = byId.get(target.item)
  return found === undefined || found.item.kind !== PROPOSAL || !featuresOf(found).some((feature) => feature.id === target.feature)
}
