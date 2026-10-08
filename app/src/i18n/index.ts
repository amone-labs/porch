// The app's language, decided by Rust (Lang::resolve) and shared by every screen.
// Both windows (main and popover) load it once and follow `language-changed`.
import { useSyncExternalStore } from "react";
import { listen } from "@tauri-apps/api/event";
import { api } from "../api";
import { ko, type Dict } from "./ko";
import { en } from "./en";

export type Lang = "ko" | "en";

const DICTS: Record<Lang, Dict> = { ko, en };
// Until Rust answers, Korean: that is how porch behaved before languages existed.
let lang: Lang = "ko";
const listeners = new Set<() => void>();

export function setLang(l: Lang) {
  if (l === lang) return;
  lang = l;
  document.documentElement.lang = l;
  listeners.forEach((f) => f());
}

/** Rust sends "ko" or "en"; anything else is treated as Korean before it reaches DICTS. */
export function asLang(v: unknown): Lang {
  return v === "en" ? "en" : "ko";
}

/** The language Rust resolved, awaited before the first paint. Korean if it cannot be read. */
export async function loadLang(): Promise<void> {
  try {
    setLang(asLang(await api.language()));
  } catch {
    setLang("ko");
  }
}

let loaded = false;
function load() {
  if (loaded) return;
  loaded = true;
  document.documentElement.lang = lang;
  // Before the shell sends resolved_language (Task 2) the field is missing: keep the current one.
  api.settings().then((s) => setLang(s.resolved_language ? asLang(s.resolved_language) : lang), () => {});
  listen<Lang>("language-changed", (e) => setLang(asLang(e.payload))).catch(() => {});
}

/** The language now, for plain functions (format.ts) that are not hooks. */
export function getLang(): Lang {
  return lang;
}

const subscribe = (f: () => void) => {
  listeners.add(f);
  return () => listeners.delete(f);
};

export function useLang(): Lang {
  load();
  return useSyncExternalStore(subscribe, () => lang);
}

export function useT(): Dict {
  return DICTS[useLang()];
}
