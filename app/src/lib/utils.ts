import { type ClassValue, clsx } from 'clsx'
import { twMerge } from 'tailwind-merge'

/** shadcn's class helper: `clsx` joins the classes and `tailwind-merge` drops a Tailwind class a later one overrides. */
export function cn(...inputs: ClassValue[]): string {
  return twMerge(clsx(inputs))
}
