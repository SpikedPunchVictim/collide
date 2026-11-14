import { readData, writeData } from './file.js'
import { migrate, CURRENT_VERSION } from './migrate.js'

const PATH = process.env.LARDER_FILE ?? 'larder.json'

export async function load() {
  const raw = await readData(PATH)
  return raw ? migrate(raw, PATH) : { version: CURRENT_VERSION, items: [] }
}

export async function save(data) {
  await writeData(PATH, { ...data, version: CURRENT_VERSION })
}
