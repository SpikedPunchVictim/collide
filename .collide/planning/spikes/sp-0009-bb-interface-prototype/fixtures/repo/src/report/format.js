export function formatAmount(amount, unit) {
  return unit === 'each' ? String(amount) : `${amount} ${unit}`
}

export function formatTotal(value) {
  return String(value)
}
