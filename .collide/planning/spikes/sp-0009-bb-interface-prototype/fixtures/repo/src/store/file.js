import { readFile, writeFile } from 'node:fs/promises'

export async function readData(path) {
  try {
    return JSON.parse(await readFile(path, 'utf8'), reviveDates)
  } catch (err) {
    if (err.code === 'ENOENT') return null
    throw err
  }
}

export async function writeData(path, data) {
  await writeFile(path, JSON.stringify(data, null, 2))
}

function reviveDates(key, value) {
  return key === 'expires' && value ? new Date(value) : value
}
