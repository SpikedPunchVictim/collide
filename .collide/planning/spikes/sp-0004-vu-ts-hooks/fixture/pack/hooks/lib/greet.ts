export interface Greeting {
  text: string;
  count: number;
}

export function greet(greeting: string, ids: readonly string[]): Greeting {
  return { text: `${greeting}, ${ids.join(" and ")}`, count: ids.length };
}
