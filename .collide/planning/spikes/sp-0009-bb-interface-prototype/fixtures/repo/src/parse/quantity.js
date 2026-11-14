import { normalizeUnit } from './units.js'

const PATTERN = /^\s*(\d+(?:\.\d+)?)\s*([a-zA-Z]+)?\s+(.+)$/

/** "2 cups flour" -> { amount: 2, unit: "cup", name: "flour" } */
export function parseQuantity(text) {
  const m = PATTERN.exec(text ?? '')
  if (!m) {
    const err = new Error('quantity')
    err.code = 'bad-quantity'
    throw err
  }
  return { amount: Number(m[1]), unit: m[2] ? normalizeUnit(m[2]) : 'each', name: m[3].trim() }
}
