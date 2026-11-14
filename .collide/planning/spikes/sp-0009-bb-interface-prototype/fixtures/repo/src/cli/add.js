import { parseQuantity } from '../parse/quantity.js'
import { parseDate } from '../parse/dates.js'
import { createItem } from '../store/item.js'
import { load, save } from '../store/index.js'

export async function runAdd([text, ...flags]) {
  const { amount, unit, name } = parseQuantity(text)
  const opts = readFlags(flags)
  const item = createItem({
    name,
    amount,
    unit,
    shelf: opts.shelf ?? 'unsorted',
    expires: opts.expires ? parseDate(opts.expires) : null,
  })
  const data = await load()
  data.items.push(item)
  await save(data)
  console.log(`added ${item.name}`)
}

function readFlags(flags) {
  const out = {}
  for (let i = 0; i < flags.length; i += 2) out[flags[i].replace(/^--/, '')] = flags[i + 1]
  return out
}
