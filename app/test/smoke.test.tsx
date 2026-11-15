import { renderToString } from 'react-dom/server'
import { expect, test } from 'vitest'
import { App } from '../src/App'

test('App renders its name', () => {
  expect(renderToString(<App />)).toContain('Collide')
})
