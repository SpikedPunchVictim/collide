const KNOWN = new Set(['cup', 'tsp', 'tbsp', 'tablespoon', 'g', 'kg', 'ml', 'l', 'oz', 'lb', 'each'])

export function normalizeUnit(raw) {
  const u = raw.toLowerCase()
  return KNOWN.has(u) ? u : u
}

export const GRAMS = { g: 1, kg: 1000, oz: 28.35, lb: 453.6 }

export function toGrams(amount, unit) {
  const f = GRAMS[unit]
  return f === undefined ? null : amount * f
}
