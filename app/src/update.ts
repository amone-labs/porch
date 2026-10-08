// Self-update. The app asks a signed manifest whether a newer version exists
// and, when the person says so, replaces itself and restarts.
//
// Rules:
//   - Checked at launch and once a day while the app stays open. A check only
//     arms passive UI; it never downloads.
//   - The check fails open: offline, a timeout, a bad manifest show nothing.
//     Only a check the person started says it could not reach the server.
//   - Nothing is downloaded until the person asks.
//   - Once they have asked, failure is shown, with a way to get the installer.
import { useSyncExternalStore } from "react";
import { api } from "./api";

export type UpdateStatus = "idle" | "available" | "working" | "restarting" | "failed";

export interface UpdateState {
  status: UpdateStatus;
  /** The prompt is showing (it can be dismissed and brought back). */
  open: boolean;
  version: string | null;
  notes: string | null;
  /** English notes (`notes_en` in latest.json); null when the release has none. */
  notesEn: string | null;
  /** 0..1 through the download, or null when the size is unknown. */
  progress: number | null;
}

interface Found {
  version: string;
  notes: string | null;
  notesEn: string | null;
  install: (onProgress: (done: number, total: number | null) => void) => Promise<void>;
}

const CHECK_TIMEOUT_MS = 5000;
const CHECK_EVERY_MS = 24 * 60 * 60 * 1000;

let state: UpdateState = { status: "idle", open: false, version: null, notes: null, notesEn: null, progress: null };
let found: Found | null = null;
const listeners = new Set<() => void>();

function set(patch: Partial<UpdateState>) {
  state = { ...state, ...patch };
  listeners.forEach((l) => l());
}

export function useUpdate(): UpdateState {
  return useSyncExternalStore(
    (l) => {
      listeners.add(l);
      return () => listeners.delete(l);
    },
    () => state,
  );
}

export const showUpdate = () => set({ open: state.status !== "idle" });
export const dismissUpdate = () => set({ open: false });

async function lookUp(): Promise<Found | null> {
  const { check } = await import("@tauri-apps/plugin-updater");
  const u = await check();
  if (!u) return null;
  return {
    version: u.version,
    notes: u.body?.trim() || null,
    // Apps before English support read only `notes` and ignore this field.
    notesEn: typeof u.rawJson?.notes_en === "string" ? u.rawJson.notes_en.trim() || null : null,
    install: (onProgress) => {
      let done = 0;
      let total: number | null = null;
      return u.downloadAndInstall((e) => {
        if (e.event === "Started") total = e.data.contentLength ?? null;
        else if (e.event === "Progress") {
          done += e.data.chunkLength;
          onProgress(done, total);
        }
      });
    },
  };
}

/**
 * Check once. Resolves to "found", "latest" or "unreachable"; never rejects.
 * A found update opens the prompt unless `quiet`.
 */
export async function checkUpdate(quiet = false): Promise<"found" | "latest" | "unreachable"> {
  if (state.status === "working" || state.status === "restarting") return "found";
  let f: Found | null;
  try {
    f = await Promise.race([lookUp(), new Promise<never>((_, no) => setTimeout(() => no(new Error("timeout")), CHECK_TIMEOUT_MS))]);
  } catch {
    return "unreachable";
  }
  if (!f) return "latest";
  found = f;
  set({ status: "available", open: !quiet, version: f.version, notes: f.notes, notesEn: f.notesEn, progress: null });
  return "found";
}

/** Check now, then once a day while nothing is waiting. */
export function watchUpdates(): () => void {
  if (import.meta.env.DEV) return () => {};
  void checkUpdate();
  const id = setInterval(() => {
    if (state.status === "idle") void checkUpdate();
  }, CHECK_EVERY_MS);
  return () => clearInterval(id);
}

// A 5MB update finishes in a blink, which reads as a glitch. The bar takes at
// least this long to fill, and the restart screen stays up this long.
const MIN_WORK_MS = 2500;
const RESTART_HOLD_MS = 1200;
const wait = (ms: number) => new Promise((r) => setTimeout(r, ms));

export async function applyUpdate(): Promise<void> {
  // The button stays up during the download; a second install would fetch it all again.
  if (state.status !== "available" || !found) return;
  set({ status: "working", progress: 0, open: true });
  const started = Date.now();
  let real = 0;
  // Shown progress is the real share, held back to the time elapsed.
  const pace = () => set({ progress: Math.min(real, (Date.now() - started) / MIN_WORK_MS) });
  const ticker = setInterval(pace, 50);
  try {
    await found.install((done, total) => {
      real = total && total > 0 ? Math.min(1, done / total) : real;
    });
  } catch {
    clearInterval(ticker);
    set({ status: "failed", progress: null, open: true });
    return;
  }
  real = 1;
  await wait(Math.max(0, MIN_WORK_MS - (Date.now() - started)) + 100);
  clearInterval(ticker);
  set({ status: "restarting", progress: 1 });
  await wait(RESTART_HOLD_MS);
  try {
    const { relaunch } = await import("@tauri-apps/plugin-process");
    await relaunch();
  } catch {
    // Installed but still running the old code; the next launch is the new one.
    set({ status: "idle", open: false });
  }
}

/** The way out when installing failed: the latest installer, opened in the browser. */
export const openDownload = () => api.openDownload();

/** Manifest notes as list items: lines starting with "-", "•" or "*". */
export function noteItems(notes: string | null): { items: string[]; rest: string | null } {
  if (!notes) return { items: [], rest: null };
  const lines = notes.split("\n").map((l) => l.trim()).filter(Boolean);
  const items = lines.filter((l) => /^[-•*]\s/.test(l)).map((l) => l.replace(/^[-•*]\s+/, ""));
  const rest = lines.filter((l) => !/^[-•*]\s/.test(l)).join(" ");
  return { items, rest: rest || null };
}

