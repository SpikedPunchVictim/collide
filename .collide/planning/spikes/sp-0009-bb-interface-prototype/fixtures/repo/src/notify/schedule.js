import { expiringSoon } from './expiry.js'
import { send } from './channels.js'

/** Called every hour by whatever runs the tool. */
export async function sendReminders(items, now = new Date()) {
  for (const item of expiringSoon(items, now)) {
    await send(`${item.name} expires on ${item.expires.toISOString().slice(0, 10)}`, now)
  }
}
