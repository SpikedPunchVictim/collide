import { load } from '../store/index.js'
import { lowStock, totals } from '../report/summary.js'
import { toCsv } from '../report/csv.js'

export async function runReport([kind]) {
  const { items } = await load()
  if (kind === 'low-stock') {
    for (const row of lowStock(items)) console.log(`${row.name}: ${row.amount} ${row.unit}`)
  } else if (kind === 'totals') {
    for (const [unit, sum] of Object.entries(totals(items))) console.log(`${unit}: ${sum}`)
  } else if (kind === 'csv') {
    process.stdout.write(toCsv(items))
  } else {
    const err = new Error(kind)
    err.code = 'unknown-report'
    throw err
  }
}
