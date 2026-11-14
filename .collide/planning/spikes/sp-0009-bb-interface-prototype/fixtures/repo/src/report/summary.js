import { LOW_STOCK_LIMIT } from '../store/item.js'

export function lowStock(items) {
  return items.filter((i) => i.amount <= (LOW_STOCK_LIMIT[i.unit] ?? 1)).map((i) => ({ name: i.name, amount: i.amount, unit: i.unit }))
}

export function totals(items) {
  const out = {}
  for (const i of items) out[i.unit] = (out[i.unit] ?? 0) + i.amount
  return out
}
