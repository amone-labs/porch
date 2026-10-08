import { useCallback, useEffect, useRef, useState } from "react";
import { api, type DayReport, type Listing, type Mode, type MonthReport, type WeekReport } from "../api";
import { DayView } from "../components/DayView";
import { WeekView } from "../components/WeekView";
import { Empty, Spinner, StatLine } from "../components/ui";
import { Icon } from "../components/Icon";
import { duration, longDate, shortDuration } from "../format";
import { startJob, useJob, useJobs } from "../jobs";
import { OpenWork } from "../components/OpenWork";
import { track } from "../analytics";
import { useT } from "../i18n";
import type { Dict } from "../i18n/ko";

/**
 * Load a report in "records" mode, then let the view ask for a summary. The
 * summary runs as a job outside this component, so leaving the screen neither
 * loses it nor starts a second one on return.
 */
/** Calls `load` when the window comes back to the front: a summary written from the
 * terminal (`porch today`) shows without leaving the screen. Saved data only, never the model. */
function useOnReturn(load: () => void) {
  const latest = useRef(load);
  latest.current = load;
  useEffect(() => {
    // Today's records are rebuilt from the transcripts: not on every quick switch.
    let last = Date.now();
    const onFocus = () => {
      if (Date.now() - last < 10_000) return;
      last = Date.now();
      latest.current();
    };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, []);
}

function useReport<T extends { generated_at: number }>(key: string, label: (t: Dict) => string, fetch: (mode: Mode) => Promise<T>) {
  const [report, setReport] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const job = useJob(key);
  // Whichever answer was built last wins: the records load and a running job race.
  const take = useCallback((r: T) => setReport((prev) => (!prev || r.generated_at >= prev.generated_at ? r : prev)), []);

  useEffect(() => {
    setReport(null);
    setError(null);
    fetch("records").then(take, (e) => setError(String(e)));
  }, [key]);
  useOnReturn(() => void fetch("records").then(take, () => {}));

  // The job leaves the store in the same tick it settles, which unmounts this
  // effect before its result arrives; so the result is checked against what
  // this screen shows now, not against the effect that asked.
  const current = useRef(key);
  useEffect(() => {
    current.current = key;
    return () => void (current.current = "");
  }, [key]);
  useEffect(() => {
    if (!job) return;
    (job.promise as Promise<T>).then(
      (r) => current.current === key && take(r),
      (e) => current.current === key && setError(String(e)),
    );
  }, [job]);

  const generate = useCallback((refresh: boolean) => startJob(key, label, () => fetch(refresh ? "refresh" : "generate"), { refresh }), [key, label]);
  return { report, busy: job?.startedAt ?? null, error, generate };
}

export function Day({ date, today }: { date: string; today?: string }) {
  const t = useT();
  const label = (tt: Dict) => (date === today ? tt.summary.todaySummary : tt.summary.dayOf(longDate(date)));
  const { report, busy, error, generate } = useReport<DayReport>(`day:${date}`, label, (m) => api.day(date, m));
  if (error) return <Empty>{t.summary.loadFailed(error)}</Empty>;
  if (!report) return <Empty>{t.summary.collecting}</Empty>;
  return <DayView report={report} busy={busy} onGenerate={generate} />;
}

export function Week({ date, onOpenDay, strip = true, onOpenInsight }: { date: string; onOpenDay: (d: string) => void; strip?: boolean; onOpenInsight?: (week: string) => void }) {
  const t = useT();
  const [, m, d] = date.split("-").map(Number);
  const { report, busy, error, generate } = useReport<WeekReport>(`week:${date}`, (tt) => tt.summary.weekOf(m, d), (mode) => api.week(date, mode));
  // The week's own numbers for the card at the end; read again when the week changes.
  const weekStart = report?.week_start ?? null;
  const [insight, setInsight] = useState<{ checkpoints: number; scored: number } | null>(null);
  useEffect(() => {
    if (!weekStart) {
      setInsight(null);
      return;
    }
    let live = true;
    setInsight(null);
    api.insight(weekStart).then(
      (v) => live && setInsight({ checkpoints: v.checkpoints.length, scored: v.eval_on ? (v.evaluation?.scores.length ?? 0) : 0 }),
      () => {},
    );
    return () => { live = false; };
  }, [weekStart]);
  if (error) return <Empty>{t.summary.loadFailed(error)}</Empty>;
  if (!report) return <Empty>{t.summary.collecting}</Empty>;
  return <WeekView report={report} busy={busy} onGenerate={generate} onOpenDay={onOpenDay} strip={strip} insight={insight} onOpenInsight={onOpenInsight ? () => onOpenInsight(report.week_start) : undefined} />;
}

/** The month's report under the calendar: same layout as a week, read from its weeks. */
export function Month({ date }: { date: string }) {
  const t = useT();
  const m = Number(date.split("-")[1]);
  const { report, busy, error, generate } = useReport<MonthReport>(`month:${date.slice(0, 7)}`, (tt) => tt.summary.monthOf(m), (mode) => api.month(date, mode));
  if (error) return <Empty>{t.summary.loadFailed(error)}</Empty>;
  if (!report) return <Empty>{t.summary.collecting}</Empty>;
  return (
    <WeekView
      report={{ ...report, week_start: `${report.month}-01` }}
      busy={busy}
      onGenerate={generate}
      onOpenDay={() => {}}
      strip={false}
      span="month"
    />
  );
}

function ymd(d: Date): string {
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
}

/** Monday on or before `d`. */
function monday(d: Date): Date {
  const x = new Date(d.getFullYear(), d.getMonth(), d.getDate());
  x.setDate(x.getDate() - ((x.getDay() + 6) % 7));
  return x;
}

export type SummaryOpen = { kind: "day" | "week" | "month"; date: string };
type View = "day" | "week" | "month";

function parse(date: string): Date {
  const [y, m, d] = date.split("-").map(Number);
  return new Date(y, m - 1, d);
}

function addDays(d: Date, n: number): Date {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate() + n);
}

/**
 * Summary: one place for every day, laid out like a calendar app. It opens on
 * today's report; Day/Week/Month switch the span, the arrows and ←/→ move by
 * that span, Today comes back. A day picked on the month or week opens as Day.
 */
export function Summary({ today, open, onChange, onOpenInsight }: { today: string; open: SummaryOpen | null; onChange: (open: SummaryOpen) => void; onOpenInsight: (week: string) => void }) {
  const t = useT();
  const [listing, setListing] = useState<Listing | null>(null);
  const view: View = open?.kind ?? "day";
  const anchor = parse(open?.date ?? today);
  const setView = (kind: View) => onChange({ kind, date: ymd(anchor) });
  const setAnchor = (date: Date) => onChange({ kind: view, date: ymd(date) });
  const jobs = useJobs();
  const running = new Set(jobs.map((j) => j.key));
  const todayDate = parse(today);
  useEffect(() => track("tab_viewed", { screen: "summary", tab: view }), [view]);

  // Reload when a summary finishes, so the month shows it without leaving.
  useEffect(() => {
    if (view !== "day") api.list().then(setListing);
  }, [view, jobs.length]);
  useOnReturn(() => {
    if (view !== "day") api.list().then(setListing);
  });

  const atToday = ymd(anchor) >= today;
  const move = (dir: -1 | 1) => {
    if (view === "day" && dir > 0 && atToday) return;
    setAnchor(view === "month"
      ? new Date(anchor.getFullYear(), anchor.getMonth() + dir, 1)
      : addDays(anchor, dir * (view === "week" ? 7 : 1)));
  };
  const openDay = (date: string) => {
    onChange({ kind: "day", date });
  };

  // ←/→ move by the shown span.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey || e.altKey || e.ctrlKey) return;
      if (e.target instanceof HTMLElement && ["INPUT", "TEXTAREA", "SELECT"].includes(e.target.tagName)) return;
      if (e.key === "ArrowLeft") move(-1);
      else if (e.key === "ArrowRight") move(1);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  const byDate = new Map(listing?.days.map((d) => [d.date, d]));
  const weekStart = monday(anchor);
  const title =
    view === "month"
      ? t.summary.monthTitle(anchor.getFullYear(), anchor.getMonth() + 1)
      : view === "week"
        ? t.summary.weekRange(weekStart, addDays(weekStart, 6))
        : `${longDate(ymd(anchor))}${ymd(anchor) === today ? t.summary.todayMark : ""}`;
  const showingToday =
    view === "month"
      ? anchor.getFullYear() === todayDate.getFullYear() && anchor.getMonth() === todayDate.getMonth()
      : view === "week"
        ? ymd(weekStart) === ymd(monday(todayDate))
        : ymd(anchor) === today;
  const unit = t.summary.units[view];

  const iconButton = (dir: -1 | 1) => (
    <button
      type="button"
      aria-label={dir < 0 ? t.summary.prevSpan(unit) : t.summary.nextSpan(unit)}
      disabled={view === "day" && dir > 0 && atToday}
      onClick={() => move(dir)}
      className="inline-flex h-7 w-7 items-center justify-center rounded-md text-muted hover:bg-fg/5 hover:text-fg disabled:opacity-30 disabled:hover:bg-transparent"
    >
      <Icon name={dir < 0 ? "chevronLeft" : "chevronRight"} size={14} />
    </button>
  );

  const toolbar = (
    <div className="sticky top-9 z-10 -mx-2 flex items-center gap-2 bg-canvas/90 px-2 pb-3 pt-4 backdrop-blur print:hidden">
      <h2 className="min-w-0 truncate text-[19px] font-medium tracking-[-0.02em]">{title}</h2>
      <div className="ml-2 flex shrink-0 items-center">
        {iconButton(-1)}
        {iconButton(1)}
      </div>
      <button
        type="button"
        disabled={showingToday}
        onClick={() => setAnchor(todayDate)}
        className="h-7 shrink-0 rounded-md border border-line px-2.5 text-[12px] text-muted hover:bg-fg/5 hover:text-fg disabled:opacity-40 disabled:hover:bg-transparent"
      >
        {t.summary.today}
      </button>
      <div role="tablist" aria-label={t.summary.viewAria} className="ml-auto flex shrink-0 rounded-lg border border-line p-0.5">
        {(["day", "week", "month"] as const).map((m) => (
          <button
            key={m}
            type="button"
            role="tab"
            aria-selected={view === m}
            onClick={() => setView(m)}
            className={
              "h-6 rounded-md px-3 text-[12px] motion-safe:transition-colors motion-safe:duration-fast " +
              (view === m ? "bg-fg/10 font-medium text-fg" : "text-muted hover:text-fg")
            }
          >
            {t.summary.tabs[m]}
          </button>
        ))}
      </div>
    </div>
  );

  if (view === "day") {
    return (
      <div>
        {toolbar}
        <Day key={ymd(anchor)} date={ymd(anchor)} today={today} />
      </div>
    );
  }

  if (view === "week") {
    // The month the chosen week sits in, one row per week; the chosen row is
    // lit and its report follows. Clicking a row picks that week.
    const first = new Date(anchor.getFullYear(), anchor.getMonth(), 1);
    const last = new Date(anchor.getFullYear(), anchor.getMonth() + 1, 0);
    const rows: Date[][] = [];
    for (let d = monday(first); d <= last; d = addDays(d, 7)) rows.push(Array.from({ length: 7 }, (_, i) => addDays(d, i)));
    const chosen = ymd(weekStart);
    return (
      <div>
        {toolbar}
        <div className="overflow-hidden rounded-lg border border-line-soft">
          <div className="grid grid-cols-7 border-b border-line-soft">
            {t.summary.weekdays.map((n, i) => (
              <span key={n} className={"px-2 py-1 text-left text-[11px] " + (i >= 5 ? "text-faint/60" : "text-faint")}>
                {n}
              </span>
            ))}
          </div>
          {rows.map((row, r) => {
            const ws = ymd(row[0]);
            const on = ws === chosen;
            const future = ws > today;
            const busy = running.has(`week:${ws}`);
            return (
              <button
                key={ws}
                type="button"
                disabled={future}
                aria-pressed={on}
                onClick={() => setAnchor(row[0])}
                className={
                  "relative grid w-full grid-cols-7 text-left motion-safe:transition-colors motion-safe:duration-fast disabled:cursor-default " +
                  (r < rows.length - 1 ? "border-b border-line-soft " : "") +
                  (on ? "bg-fg/[0.08] " : future ? "" : "hover:bg-fg/[0.04] ")
                }
              >
                {on && <span aria-hidden className="absolute inset-y-0 left-0 w-0.5 bg-accent" />}
                {row.map((d, i) => {
                  const key = ymd(d);
                  const rec = byDate.get(key);
                  const out = d.getMonth() !== anchor.getMonth();
                  return (
                    <span
                      key={key}
                      className={
                        "flex h-[52px] flex-col gap-0.5 px-1.5 py-1 " +
                        (i < 6 ? "border-r border-line-soft " : "") +
                        (out ? "bg-surface/60" : key === today ? "bg-fg/[0.04]" : "")
                      }
                    >
                      <span className="flex items-center justify-between">
                        <span
                          className={
                            "inline-flex h-5 min-w-5 items-center justify-center rounded-full px-1 text-[12px] " +
                            (key === today ? "bg-accent font-medium text-on-accent" : out || key > today ? "text-faint" : "text-fg/90")
                          }
                        >
                          {d.getDate()}
                        </span>
                        {rec && !out && (
                          <span className={"text-[11px] " + (rec.headline ? "text-muted" : "text-faint")}>{shortDuration(rec.minutes)}</span>
                        )}
                      </span>
                      {rec && !out && (rec.issues > 0 || rec.waiting > 0) && (
                        <span className="mt-auto flex flex-wrap gap-1 text-[10px]">
                          {rec.issues > 0 && <span className="whitespace-nowrap rounded border border-muted px-1 text-fg/90">{t.openWork.blocker} {rec.issues}</span>}
                          {rec.waiting > 0 && <span className="whitespace-nowrap rounded border border-line px-1 text-faint">{t.openWork.pending} {rec.waiting}</span>}
                        </span>
                      )}
                    </span>
                  );
                })}
                {busy && (
                  <span className="absolute bottom-1.5 right-2 flex items-center gap-1 text-[10.5px] text-muted">
                    <Spinner size={10} />
                    {t.summary.makingWeek}
                  </span>
                )}
              </button>
            );
          })}
        </div>
        <p className="mt-2 text-[11.5px] text-faint">{t.summary.weekNote}</p>
        <Week key={chosen} date={chosen} onOpenDay={openDay} onOpenInsight={onOpenInsight} />
      </div>
    );
  }

  // Month grid: whole weeks from the Monday on or before the 1st to the Sunday after the last day.
  const first = new Date(anchor.getFullYear(), anchor.getMonth(), 1);
  const last = new Date(anchor.getFullYear(), anchor.getMonth() + 1, 0);
  const cells: Date[] = [];
  for (let d = monday(first); d <= last || cells.length % 7 !== 0; d = addDays(d, 1)) cells.push(d);
  const inMonth = cells.filter((d) => d.getMonth() === anchor.getMonth()).map((d) => byDate.get(ymd(d))).filter(Boolean);
  const monthMinutes = inMonth.reduce((a, d) => a + (d?.minutes ?? 0), 0);
  const summarized = inMonth.filter((d) => d?.headline).length;

  return (
    <div>
      {toolbar}
      <StatLine items={[t.summary.daysRecorded(inMonth.length), duration(monthMinutes), t.summary.daysSummarized(summarized)]} />

      <div className="mt-4 overflow-hidden rounded-lg border border-line-soft">
        <div className="grid grid-cols-7 border-b border-line-soft">
          {t.summary.weekdays.map((n, i) => (
            <span key={n} className={"px-2 py-1 text-left text-[11px] " + (i >= 5 ? "text-faint/60" : "text-faint")}>
              {n}
            </span>
          ))}
        </div>
        <div className="grid grid-cols-7">
          {cells.map((d, i) => {
            const key = ymd(d);
            const rec = byDate.get(key);
            const out = d.getMonth() !== anchor.getMonth();
            const future = key > today;
            const isToday = key === today;
            const busy = running.has(`day:${key}`);
            return (
              <button
                key={key}
                type="button"
                disabled={future || out}
                onClick={() => openDay(key)}
                className={
                  "flex h-[96px] flex-col items-stretch gap-0.5 border-line-soft px-1.5 py-1 text-left motion-safe:transition-colors motion-safe:duration-fast disabled:cursor-default " +
                  (i % 7 !== 6 ? "border-r " : "") +
                  (i < cells.length - 7 ? "border-b " : "") +
                  (future || out ? "" : "hover:bg-fg/[0.05] ") +
                  (out ? "bg-surface/60 " : isToday ? "bg-fg/[0.04] " : "")
                }
              >
                <span className="flex items-center justify-between">
                  <span
                    className={
                      "inline-flex h-5 min-w-5 items-center justify-center rounded-full px-1 text-[12px] " +
                      (isToday ? "bg-accent font-medium text-on-accent" : out || future ? "text-faint" : "text-fg/90")
                    }
                  >
                    {d.getDate()}
                  </span>
                  {rec && !out && (
                    <span className={"text-[11px] " + (rec.headline ? "text-muted" : "text-faint")}>{shortDuration(rec.minutes)}</span>
                  )}
                </span>
                {out ? null : busy ? (
                  <span className="flex items-center gap-1.5 text-[11px] text-muted">
                    <Spinner size={10} />
                    {t.summary.makingDay}
                  </span>
                ) : rec?.headline ? (
                  <span className="line-clamp-2 text-[11.5px] leading-snug text-muted">{rec.headline}</span>
                ) : null}
                {rec && !out && (rec.issues > 0 || rec.waiting > 0) && (
                  <span className="mt-auto flex flex-wrap gap-1.5 text-[10.5px]">
                    {rec.issues > 0 && <span className="whitespace-nowrap rounded border border-muted px-1 text-fg/90">{t.openWork.blocker} {rec.issues}</span>}
                    {rec.waiting > 0 && <span className="whitespace-nowrap rounded border border-line px-1 text-faint">{t.openWork.pending} {rec.waiting}</span>}
                  </span>
                )}
              </button>
            );
          })}
        </div>
      </div>
      <p className="mt-3 text-[11.5px] text-faint">{t.summary.monthNote}</p>
      <Month key={ymd(first)} date={ymd(first)} />
      <OpenWork />
    </div>
  );
}
