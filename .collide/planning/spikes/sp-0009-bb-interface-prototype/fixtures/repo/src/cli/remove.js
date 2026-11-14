import { load, save } from '../store/index.js'

export async function runRemove([name]) {
  const data = await load()
  const before = data.items.length
  data.items = data.items.filter((i) => i.name !== name)
  if (data.items.length === before) {
    const err = new Error(name)
    err.code = 'not-found'
    throw err
  }
  await save(data)
  console.log(`removed ${name}`)
}
