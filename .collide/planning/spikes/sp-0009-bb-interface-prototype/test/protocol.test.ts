import { readFileSync } from 'node:fs'
import { describe, expect, it } from 'vitest'
import { SCENARIOS } from '../src/scenarios'

// SESSION-PROTOCOL.md must hold every card exactly as the app shows it, so the facilitator reads what the app says.
const protocol = readFileSync('SESSION-PROTOCOL.md', 'utf8')

describe('SESSION-PROTOCOL.md', () => {
  for (const sc of SCENARIOS) {
    it(`contains the card for ${sc.id} verbatim`, () => {
      expect(protocol).toContain(sc.card(sc.setup()))
    })
  }
})
