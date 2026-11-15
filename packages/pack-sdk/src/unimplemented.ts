/**
 * The body of every stub. The tag names the work that fills it (`<area>: <name>`), and `stub-tags.json` maps
 * each tag to the stage that does that work (0.15). It throws, so a stub that is reached fails loudly.
 */
export function unimplemented(tag: string): never {
  throw new Error(`unimplemented: ${tag}`)
}
