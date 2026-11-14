const sent = []

export async function send(text, at) {
  sent.push({ text, at })
  console.log(`[reminder ${at.toISOString()}] ${text}`)
}

export const history = () => [...sent]
