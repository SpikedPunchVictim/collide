import { load } from '../store/index.js'
import { formatAmount } from '../report/format.js'

export async function runList() {
  const { items } = await load()
  for (const item of items) {
    const when = item.expires ? item.expires.toISOString().slice(0, 10) : '-'
    console.log(`${item.name.padEnd(24)} ${formatAmount(item.amount, item.unit).padEnd(12)} ${item.shelf.padEnd(10)} ${when}`)
  }
}
