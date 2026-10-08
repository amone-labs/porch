// While a PDF is printed, parts drawn in screen colors (the diagrams) redraw
// for white paper. The print stylesheet handles everything else by itself.

type Redraw = (paper: boolean) => Promise<unknown>;
const listeners = new Set<Redraw>();

export function onPaper(l: Redraw): () => void {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** Switch every listener and wait until each has redrawn. */
export async function setPaper(paper: boolean): Promise<void> {
  await Promise.all([...listeners].map((l) => l(paper).catch(() => {})));
  // One frame for React to put the redrawn SVGs on the page.
  await new Promise((r) => requestAnimationFrame(() => r(null)));
}
