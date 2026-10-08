import { getLang } from "./i18n";

export function duration(min: number): string {
  const h = Math.floor(min / 60);
  const m = min % 60;
  if (getLang() === "en") {
    if (h === 0) return `${m}m`;
    if (m === 0) return `${h}h`;
    return `${h}h ${m}m`;
  }
  if (h === 0) return `${m}분`;
  if (m === 0) return `${h}시간`;
  return `${h}시간 ${m}분`;
}

/** "59분", then "1.3시간"; "59 min", then "1.3 h": the same rounding as `insight::hm`. */
export function hm(ms: number): string {
  const min = Math.round(ms / 60_000);
  if (getLang() === "en") return min < 60 ? `${min} min` : `${(ms / 3_600_000).toFixed(1)} h`;
  return min < 60 ? `${min}분` : `${(ms / 3_600_000).toFixed(1)}시간`;
}

/** Calendar cells: "1h 25m", "45m", "2h". */
export function shortDuration(min: number): string {
  const h = Math.floor(min / 60);
  const m = min % 60;
  if (h === 0) return `${m}m`;
  return m === 0 ? `${h}h` : `${h}h ${m}m`;
}

/** How long something has been going on: "방금", "12분째", "1시간 5분째"; "just now", "12 min", "1h 5m". */
export function lasting(now: number, since: number): string {
  const m = Math.floor(Math.max(0, now - since) / 60_000);
  const en = getLang() === "en";
  if (m === 0) return en ? "just now" : "방금";
  if (m < 60) return en ? `${m} min` : `${m}분째`;
  const h = Math.floor(m / 60);
  return en ? `${h}h${m % 60 ? ` ${m % 60}m` : ""}` : `${h}시간${m % 60 ? ` ${m % 60}분` : ""}째`;
}

/** "방금", "12분 전", "3시간 전", "2일 전"; "just now", "12 min ago", "3 h ago", "2 d ago". */
export function ago(now: number, t: number): string {
  const m = Math.floor(Math.max(0, now - t) / 60_000);
  const en = getLang() === "en";
  if (m === 0) return en ? "just now" : "방금";
  if (m < 60) return en ? `${m} min ago` : `${m}분 전`;
  if (m < 1440) return en ? `${Math.floor(m / 60)} h ago` : `${Math.floor(m / 60)}시간 전`;
  return en ? `${Math.floor(m / 1440)} d ago` : `${Math.floor(m / 1440)}일 전`;
}

export function hhmm(t: number): string {
  const d = new Date(t);
  return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
}

const WEEKDAYS = ["일", "월", "화", "수", "목", "금", "토"];
const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

/** "2026-09-28" -> "9월 28일" or "Sep 28", the same day as `longDate` without the weekday. */
export function monthDay(date: string): string {
  const [, m, d] = date.split("-").map(Number);
  if (getLang() === "en") return `${MONTHS[m - 1]} ${d}`;
  return `${m}월 ${d}일`;
}

/** "2026-09-29" -> "9월 29일 (화)" or "Tue, Sep 29", using the local calendar. */
export function longDate(date: string): string {
  const [y, m, d] = date.split("-").map(Number);
  const day = new Date(y, m - 1, d);
  if (getLang() === "en") return day.toLocaleDateString("en-US", { weekday: "short", month: "short", day: "numeric" });
  return `${m}월 ${d}일 (${WEEKDAYS[day.getDay()]})`;
}

export function homeRelative(path: string): string {
  return path.replace(/^\/Users\/[^/]+/, "~");
}

/** Korean particle for `word`: josa("오늘 요약", "은", "는") -> "오늘 요약은". */
export function josa(word: string, withFinal: string, withoutFinal: string): string {
  const c = word.charCodeAt(word.length - 1);
  const hangul = c >= 0xac00 && c <= 0xd7a3;
  return word + (hangul && (c - 0xac00) % 28 !== 0 ? withFinal : withoutFinal);
}

/** "커밋 33", or "커밋 33 · 내 커밋 12" when some were written by others; "33 commits", "33 commits · 12 mine". */
export function commitsLabel(total: number, mine?: number | null): string {
  const all = mine == null || mine === total;
  if (getLang() === "en") return all ? `${total} commits` : `${total} commits · ${mine} mine`;
  return all ? `커밋 ${total}` : `커밋 ${total} · 내 커밋 ${mine}`;
}

/** List-price dollars: "$0.42", "$23.40", "$2,703". */
export function money(usd: number): string {
  if (usd >= 1000) return `$${Math.round(usd).toLocaleString("en-US")}`;
  if (usd >= 100) return `$${Math.round(usd)}`;
  return `$${usd.toFixed(2)}`;
}

/** Token counts: "812", "412K", "18.2M", "4.35B". */
export function tokens(n: number): string {
  if (n < 1000) return `${n}`;
  if (n < 1_000_000) return `${Math.round(n / 1000)}K`;
  if (n < 1_000_000_000) return `${(n / 1_000_000).toFixed(n < 10_000_000 ? 2 : 1).replace(/\.0+$/, "")}M`;
  return `${(n / 1_000_000_000).toFixed(2).replace(/\.0+$/, "")}B`;
}

export function tokenTotal(t: { input: number; output: number; cache_read: number; cache_write_5m: number; cache_write_1h: number }): number {
  return t.input + t.output + t.cache_read + t.cache_write_5m + t.cache_write_1h;
}

/** Share of input read from cache, 0..100, or null with no input. */
export function cacheHit(t: { input: number; cache_read: number; cache_write_5m: number; cache_write_1h: number }): number | null {
  const read = t.input + t.cache_read + t.cache_write_5m + t.cache_write_1h;
  return read > 0 ? Math.round((t.cache_read / read) * 100) : null;
}

/** "claude-opus-5-5" -> "opus 5.5", "MiniMax-M3[1m]" -> "minimax-m3", like the core's short_model. */
/** "Claude Code(sonnet)", "Codex(기본 모델)"; "Claude Code (sonnet)", "Codex (default model)": who wrote a saved summary. */
export function writtenBy(provider: "claude" | "codex" | null | undefined, model: string): string {
  const name = provider === "codex" ? "Codex" : "Claude Code";
  if (getLang() === "en") return `${name} (${model === "default" ? "default model" : model})`;
  return `${name}(${model === "default" ? "기본 모델" : model})`;
}

export function shortModel(model: string): string {
  const m = model.trim().toLowerCase().replace(/\[[^\]]*\]$/, "");
  const c = m.match(/^claude-([a-z]+)-([0-9-]+?)(?:-\d{8})?$/);
  if (c) return `${c[1]} ${c[2].replace(/-/g, ".")}`;
  const g = m.match(/^(gpt-[0-9.]+)-([a-z]+)$/);
  return g && g[1].includes(".") ? `${g[1]} ${g[2]}` : m;
}

/** "오후 3:10", or "10월 3일 오후 3:10" when not today; "3:10 PM", or "Oct 3, 3:10 PM" when not today. */
export function resetTime(ms: number, now: number): string {
  const d = new Date(ms);
  const today = new Date(now).toDateString() === d.toDateString();
  if (getLang() === "en") {
    const t = d.toLocaleTimeString("en-US", { hour: "numeric", minute: "2-digit" });
    return today ? t : `${d.toLocaleDateString("en-US", { month: "short", day: "numeric" })}, ${t}`;
  }
  const t = d.toLocaleTimeString("ko-KR", { hour: "numeric", minute: "2-digit" });
  return today ? t : `${d.getMonth() + 1}월 ${d.getDate()}일 ${t}`;
}
