// Quiet lines at the bottom of the window. Any screen can raise one; the shell
// renders them as a stack, newest in front. Each leaves on its own timer, and
// none leaves while the pointer is over the stack.
import { useSyncExternalStore } from "react";

export interface Notice {
  text: string;
  /** Label and handler for the one action it offers, if any. */
  action?: { label: string; run: () => void };
  /** Stays until acted on or closed. */
  sticky?: boolean;
  /** A notice with the same key replaces the one showing instead of stacking. */
  key?: string;
}

export interface ShownNotice extends Notice {
  id: number;
}

/** More than this and the oldest goes. */
const MAX = 5;

let shown: ShownNotice[] = [];
let seq = 0;
let paused = false;
const timers = new Map<number, ReturnType<typeof setTimeout>>();
const listeners = new Set<() => void>();

function set(next: ShownNotice[]) {
  shown = next;
  listeners.forEach((l) => l());
}

function life(n: Notice): number {
  return n.action ? 8000 : 5000;
}

function arm(n: ShownNotice, ms: number) {
  clearTimeout(timers.get(n.id));
  timers.set(
    n.id,
    setTimeout(() => {
      // Being read: look again shortly instead of pulling it away.
      if (paused) arm(n, 1500);
      else dismiss(n.id);
    }, ms),
  );
}

export function notify(n: Notice): number {
  const id = ++seq;
  const item: ShownNotice = { ...n, id };
  const replaced = n.key ? shown.filter((s) => s.key === n.key) : [];
  replaced.forEach((s) => clearTimeout(timers.get(s.id)));
  let next = [...shown.filter((s) => !replaced.includes(s)), item];
  while (next.length > MAX) {
    clearTimeout(timers.get(next[0].id));
    next = next.slice(1);
  }
  set(next);
  if (!n.sticky) arm(item, life(n));
  return id;
}

/** Close one notice, or all of them. */
export function dismiss(id?: number) {
  const gone = id === undefined ? shown : shown.filter((s) => s.id === id);
  gone.forEach((s) => {
    clearTimeout(timers.get(s.id));
    timers.delete(s.id);
  });
  set(id === undefined ? [] : shown.filter((s) => s.id !== id));
}

/** While the pointer is over the stack, nothing times out. */
export function pauseNotices(on: boolean) {
  paused = on;
}

/** Oldest first; the last one is in front. */
export function useNotices(): ShownNotice[] {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => shown,
  );
}
