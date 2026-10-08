// Display preferences read once and shared: whether list-price cost is shown.
// Settings updates call setShowCost so every screen follows at once.
import { useSyncExternalStore } from "react";
import { api } from "./api";

let showCost = true;
const listeners = new Set<() => void>();

export function setShowCost(v: boolean) {
  showCost = v;
  listeners.forEach((l) => l());
}

let loaded = false;
function load() {
  if (loaded) return;
  loaded = true;
  api.settings().then((s) => setShowCost(s.settings.show_cost), () => {});
}

export function useShowCost(): boolean {
  load();
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => showCost,
  );
}
