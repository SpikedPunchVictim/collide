const HEADER = ['name', 'amount', 'unit', 'shelf', 'expires']

export function toCsv(items) {
  const rows = items.map((i) => [i.name, i.amount, i.unit, i.shelf, i.expires ? i.expires.toISOString().slice(0, 10) : ''])
  return [HEADER, ...rows].map((r) => r.join(',')).join('\n') + '\n'
}
