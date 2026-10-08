import { useSyncExternalStore } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

let percent = 100;
const listeners = new Set<() => void>();
let applyZoom: ((next: number) => void) | undefined;

export function useZoom() {
  return useSyncExternalStore(
    (listener) => { listeners.add(listener); return () => { listeners.delete(listener); }; },
    () => percent,
  );
}

export function setZoom(next: number) {
  if (Number.isFinite(next)) applyZoom?.(Math.max(70, Math.min(200, next)));
}

/** Scale text and layout together with the native WebView zoom. */
export function installZoomShortcuts() {
  const webview = getCurrentWebview();
  const storageKey = `porch.zoom.${webview.label}`;
  try {
    const saved = Number(localStorage.getItem(storageKey));
    if (Number.isFinite(saved) && saved >= 70 && saved <= 200) percent = saved;
  } catch {
    // Zoom still works when preference storage is unavailable.
  }

  let requested = percent;
  let pending = Promise.resolve();
  applyZoom = (next) => {
    requested = next;
    pending = pending.then(async () => {
      await webview.setZoom(next / 100);
      percent = next;
      listeners.forEach((listener) => listener());
      try {
        localStorage.setItem(storageKey, String(next));
      } catch {
        // Persistence is optional.
      }
    }).catch((error: unknown) => {
      requested = percent;
      console.error("Could not set page zoom", error);
    });
  };
  applyZoom(percent);

  const onKey = (event: KeyboardEvent) => {
    if (!event.metaKey || event.ctrlKey || event.altKey || event.isComposing) return;
    let next: number;
    switch (event.key) {
      case "+":
      case "=":
        next = requested + 10;
        break;
      case "-":
        next = requested - 10;
        break;
      case "0":
        next = 100;
        break;
      default:
        return;
    }
    event.preventDefault();
    setZoom(next);
  };
  window.addEventListener("keydown", onKey);
  return () => {
    window.removeEventListener("keydown", onKey);
    applyZoom = undefined;
  };
}
