export function createItem({ name, amount, unit, shelf, expires }) {
  if (!name) throw new Error('an item needs a name')
  return { name, amount, unit, shelf, expires, addedAt: new Date() }
}

export const LOW_STOCK_LIMIT = { each: 2, g: 200, kg: 0.2, ml: 200, l: 0.2, cup: 1 }
