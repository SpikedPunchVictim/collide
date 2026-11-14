import { daysBetween } from '../parse/dates.js'

export const WARN_DAYS = 1

export function expiringSoon(items, now = new Date()) {
  return items.filter((item) => {
    const left = daysBetween(now, item.expires)
    return left <= WARN_DAYS
  })
}
