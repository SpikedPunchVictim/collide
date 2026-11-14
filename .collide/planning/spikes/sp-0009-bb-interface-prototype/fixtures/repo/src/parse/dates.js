export function parseDate(text) {
  const d = new Date(`${text}T00:00:00Z`)
  if (Number.isNaN(d.getTime())) {
    const err = new Error(text)
    err.code = 'bad-date'
    throw err
  }
  return d
}

export function daysBetween(from, to) {
  return Math.floor((to.getTime() - from.getTime()) / 86_400_000)
}
