// State a screen keeps while the window is open: leaving Usage for Summary and
// coming back shows the same range and grouping, as Summary keeps its period.
// It lives outside the component, so unmounting the screen does not lose it,
// and only for this run of the app.
import { useState } from "react";

const kept = new Map<string, unknown>();

/** `useState` whose value outlives the component, under `key`. */
export function useKept<T>(key: string, initial: T): [T, (v: T | ((prev: T) => T)) => void] {
  const [value, setValue] = useState<T>(() => (kept.has(key) ? (kept.get(key) as T) : initial));
  const set = (v: T | ((prev: T) => T)) =>
    setValue((prev) => {
      const next = typeof v === "function" ? (v as (prev: T) => T)(prev) : v;
      kept.set(key, next);
      return next;
    });
  return [value, set];
}

/** Set what a `useKept` under `key` starts from the next time its screen mounts. */
export function keep<T>(key: string, value: T) {
  kept.set(key, value);
}
