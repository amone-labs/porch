import type { Locale } from "./i18n/types";

/** Sample day for the hero and summary cards (copy appendix §1). Shares are computed, never typed. */
export const MOCK = {
  totalMin: 312,
  projects: 3,
  sessions: 7,
  requests: 41,
  commits: 9,
  tasks: [
    { min: 74, cost: 8.2 },
    { min: 52, cost: 3.1 },
  ],
  // One row per project; start/len in minutes from 09:00; task = index into tasks, -1 = other work.
  timeline: [
    { bars: [{ start: 10, len: 22, task: 0 }, { start: 40, len: 18, task: 0 }, { start: 150, len: 34, task: 0 }, { start: 300, len: 20, task: -1 }] },
    { bars: [{ start: 70, len: 26, task: 1 }, { start: 110, len: 26, task: 1 }, { start: 360, len: 30, task: -1 }] },
    { bars: [{ start: 200, len: 24, task: -1 }, { start: 240, len: 40, task: -1 }, { start: 420, len: 26, task: -1 }] },
  ],
  timelineSpan: 480,
};

export function fmtDuration(min: number, locale: Locale): string {
  const h = Math.floor(min / 60);
  const m = min % 60;
  if (locale === "ko") return [h ? `${h}시간` : "", m ? `${m}분` : ""].filter(Boolean).join(" ");
  return [h ? `${h}h` : "", m ? `${m}m` : ""].filter(Boolean).join(" ");
}

export function sharePct(min: number, total: number): number {
  return Math.round((min / total) * 100);
}

export function fmtCost(usd: number): string {
  return `$${usd.toFixed(2)}`;
}
