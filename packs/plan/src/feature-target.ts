/**
 * A link target that names a feature of a proposal: `<item id>#F<n>`, `n` from 1 with no leading zero (revision 14; 0.10,
 * "A proposal's features"). The same rule as core's `parse_feature_target`.
 */
export type FeatureTargetResult = { ok: true; item: string; feature: string } | { ok: false; reason: string }

const FEATURE = /^F[1-9][0-9]*$/

/** The example every message gives, so that a person sees the form. */
export const FEATURE_TARGET_EXAMPLE = 'pr-0003-km#F2'

export function parseFeatureTarget(to: string): FeatureTargetResult {
  const parts = to.split('#')
  if (parts.length === 1) return { ok: false, reason: `a feature target names a feature, as ${FEATURE_TARGET_EXAMPLE}` }
  if (parts.length > 2) return { ok: false, reason: `a feature target has one "#", as ${FEATURE_TARGET_EXAMPLE}` }
  const [item, fragment] = parts as [string, string]
  if (item === '') return { ok: false, reason: `a feature target starts with the id of a proposal, as ${FEATURE_TARGET_EXAMPLE}` }
  if (!FEATURE.test(fragment)) return { ok: false, reason: `the feature "#${fragment}" must be F and a number from 1 with no leading zero, as #F2` }
  return { ok: true, item, feature: fragment }
}

/** Whether a link target is `id`, or a feature of `id`. `reid` rewrites exactly these. */
export const namesItem = (to: string, id: string): boolean => to === id || to.startsWith(`${id}#`)
