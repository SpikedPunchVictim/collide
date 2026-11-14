import { writeFile } from 'node:fs/promises'

export const CURRENT_VERSION = 3

const steps = {
  1: (data) => ({ ...data, items: data.items.map((i) => ({ ...i, shelf: i.shelf ?? 'unsorted' })), version: 2 }),
  2: (data) => ({ ...data, items: data.items.map((i) => ({ ...i, unit: i.unit ?? 'each' })), version: 3 }),
}

export async function migrate(data, path) {
  let current = data
  while (current.version < CURRENT_VERSION) {
    current = steps[current.version](current)
  }
  if (current !== data) await writeFile(path, JSON.stringify(current, null, 2))
  return current
}
