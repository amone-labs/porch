// Summaries take a minute or more. They run in the backend and write their
// report to disk no matter what the window shows, so the job lives here, not in
// the screen that asked for it: leaving a tab and coming back picks the running
// job up again instead of starting over or forgetting it.
import { useSyncExternalStore } from "react";
import type { Dict } from "./i18n/ko";

export interface Job {
  key: string;
  /** What the sidebar and the leave notice call it, in the language now in use: "오늘 요약", "Summary for Sep 22". */
  label: (t: Dict) => string;
  startedAt: number;
  /** A summary written over the one already saved. */
  refresh?: boolean;
  promise: Promise<unknown>;
}

const jobs = new Map<string, Job>();
const listeners = new Set<() => void>();
const doneListeners = new Set<(job: Job, ok: boolean, error?: string, value?: unknown) => void>();
let snapshot: Job[] = [];

function emit() {
  snapshot = [...jobs.values()];
  listeners.forEach((l) => l());
}

function subscribe(l: () => void) {
  listeners.add(l);
  return () => listeners.delete(l);
}

/** Start `run` under `key`, or hand back the job already running under it. `label` is
 *  called with the dictionary in use when the job is shown, so a language switch
 *  mid-run changes the notice too. */
export function startJob<T>(key: string, label: (t: Dict) => string, run: () => Promise<T>, opts: { refresh?: boolean } = {}): Job & { promise: Promise<T> } {
  const running = jobs.get(key);
  if (running) return running as Job & { promise: Promise<T> };
  const finish = (ok: boolean, error?: string, value?: T) => {
    jobs.delete(key);
    emit();
    doneListeners.forEach((l) => l(job, ok, error, value));
  };
  const promise = run().then(
    // A summary the model could not write still resolves, carrying the reason.
    (v) => {
      const error = (v as { summary_error?: string | null })?.summary_error ?? undefined;
      finish(!error, error, v);
      return v;
    },
    (e) => {
      finish(false, String(e));
      throw e;
    },
  );
  const job: Job & { promise: Promise<T> } = { key, label, startedAt: Date.now(), refresh: opts.refresh, promise };
  // Nobody may be watching when it fails (the screen was left); that is fine.
  promise.catch(() => {});
  jobs.set(key, job);
  emit();
  return job;
}

export function useJobs(): Job[] {
  return useSyncExternalStore(subscribe, () => snapshot);
}

export function useJob(key: string): Job | undefined {
  return useJobs().find((j) => j.key === key);
}

/** Called once per job as it settles, wherever the window is at the time, with what it resolved to. */
export function onJobDone(l: (job: Job, ok: boolean, error?: string, value?: unknown) => void): () => void {
  doneListeners.add(l);
  return () => doneListeners.delete(l);
}
