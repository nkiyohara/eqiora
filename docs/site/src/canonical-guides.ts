import type { MarkdownInstance } from 'astro';

// Import the maintained Markdown through Astro's configured renderer, not a copy
// or an alternate Markdown parser. The filename also determines its guide route.
const guides = import.meta.glob<MarkdownInstance<Record<string, unknown>>>(
  '../../python/{modeling,execution-and-arrays,differentiation}.md',
  { eager: true },
);

export function canonicalGuide(name: string) {
  return guides[`../../python/${name}.md`];
}
