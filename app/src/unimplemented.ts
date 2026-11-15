/**
 * The body of every stub in the app. The tag names the work that fills it (`<area>: <name>`), and the repository's
 * `stub-tags.json` maps each tag to the stage that does that work (0.15). It throws, so a stub that is reached fails
 * loudly. It is the same function as the SDK's: the app reaches the SDK package for types only, never for code.
 */
export function unimplemented(tag: string): never {
  throw new Error(`unimplemented: ${tag}`)
}
