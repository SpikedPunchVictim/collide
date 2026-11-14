const TEXT = {
  'unknown-command': ({ name }) => `Unknown command: ${name}`,
  'unknown-report': ({ detail }) => `There is no report called ${detail}`,
  'not-found': ({ detail }) => `Nothing called ${detail} is in the larder`,
  'bad-quantity': () => 'Invalid input',
  'bad-date': ({ detail }) => `Could not read the date ${detail}`,
  unexpected: ({ detail }) => `Something went wrong: ${detail}`,
}

export function message(code, vars = {}) {
  return (TEXT[code] ?? TEXT.unexpected)(vars)
}
