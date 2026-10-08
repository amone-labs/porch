// Insights: a week of hook records replayed into time and errors, the checkpoints
// picked from them by rule, and below them the model-written evaluation (ADR 0012)
// and next week's goal.
import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type EvalItem, type InsightView, type Observed } from "../api";
import { Icon } from "../components/Icon";
import { Columns, HBars, KEY, ShareBar, type Column } from "../components/InsightCharts";
import { EvalArea, LastGoalLine, NextGoal } from "../components/InsightEval";
import { Disclosure, Empty, Section, Spinner } from "../components/ui";
import { hm, monthDay } from "../format";
import { useLang, useT } from "../i18n";
import { startJob, useJob } from "../jobs";
import { useKept } from "../kept";

/** "2026-09-28" moved by `n` days, in calendar terms. */
export function shiftDate(date: string, n: number): string {
  const [y, m, d] = date.split("-").map(Number);
  const t = new Date(Date.UTC(y, m - 1, d + n));
  return t.toISOString().slice(0, 10);
}

function pct(part: number, whole: number): string {
  return whole > 0 ? `${Math.round((part / whole) * 100)}%` : "0%";
}

const running = (o: Observed) => o.days.reduce((a, d) => a + d.running_ms, 0);
const waitingOf = (d: Observed["days"][number]) => d.turn_done_ms + d.question_ms + d.failed_ms + d.permission_ms;
const waiting = (o: Observed) => o.days.reduce((a, d) => a + waitingOf(d), 0);
const errorsOf = (d: Observed["days"][number]) => Object.values(d.tool_errors).reduce((a, n) => a + n, 0);

function Kpi({ label, value, unit, sub, columns }: { label: string; value: string; unit: string; sub: string; columns: Column[] }) {
  const t = useT();
  return (
    <div className="min-w-0 px-4 py-3.5">
      <p className="text-[12px] text-muted">{label}</p>
      <p className="mt-0.5 text-[26px] font-semibold leading-tight text-fg">{value}<span className={`${unit.startsWith(" ") ? "" : "ml-0.5 "}text-[14px] font-medium text-muted`}>{unit}</span></p>
      <p className="text-[12px] text-faint tabular-nums">{sub}</p>
      <div className="mt-2 max-w-36"><Columns columns={columns} height={36} label={t.insight.byWeekday(label)} ticks={false} /></div>
    </div>
  );
}

function Kpis({ o }: { o: Observed }) {
  const t = useT();
  const run = running(o);
  const wait = waiting(o);
  const stops = o.days.reduce((a, d) => a + d.stop_failures, 0);
  const turnDone = o.days.reduce((a, d) => a + d.turn_done_ms, 0);
  const failed = o.days.reduce((a, d) => a + d.failed_ms, 0);
  const calls = o.days.reduce((a, d) => a + d.tool_calls, 0);
  const errors = o.days.reduce((a, d) => a + errorsOf(d), 0);
  const peakStop = o.days.reduce((p, d, i) => (d.stop_failures > o.days[p].stop_failures ? i : p), 0);
  const day = (i: number) => `${monthDay(o.days[i].date)} (${t.summary.weekdays[i]})`;
  const col = (i: number, value: number, rows: { value: string; label: string }[], tone: "hi" | "mid" = "mid"): Column =>
    ({ key: o.days[i].date, segments: [{ value, tone }], tip: { title: day(i), rows } });
  return (
    <div className="mt-5 grid grid-cols-2 divide-x divide-line border-y border-line md:grid-cols-4">
      <Kpi label={t.insight.workingTime} value={(run / 3_600_000).toFixed(1)} unit={t.insight.unitHours} sub={t.insight.perSessionTotal}
        columns={o.days.map((d, i) => col(i, d.running_ms, [{ value: hm(d.running_ms), label: t.insight.workingTime }, { value: pct(d.running_ms, run), label: t.insight.shareOfWeek }]))} />
      <Kpi label={t.insight.idleTime} value={(wait / 3_600_000).toFixed(1)} unit={t.insight.unitHours} sub={t.insight.idleSub(pct(wait, run + wait), hm(turnDone))}
        columns={o.days.map((d, i) => col(i, waitingOf(d), [{ value: hm(waitingOf(d)), label: t.insight.idleTime }, { value: hm(d.turn_done_ms + d.question_ms + d.permission_ms), label: t.insight.stateDone }, { value: hm(d.failed_ms), label: t.insight.stateFailed }]))} />
      <Kpi label={t.insight.stateFailed} value={String(stops)} unit={t.insight.unitTimes(stops)} sub={stops > 0 ? t.insight.stopSub(hm(failed), monthDay(o.days[peakStop].date), o.days[peakStop].stop_failures) : t.insight.none}
        columns={o.days.map((d, i) => col(i, d.stop_failures, [{ value: t.insight.times(d.stop_failures), label: t.insight.stateFailed }, { value: hm(d.failed_ms), label: t.insight.stopTime }], stops > 0 && i === peakStop ? "hi" : "mid"))} />
      <Kpi label={t.insight.toolErrorRate} value={calls > 0 ? ((errors / calls) * 100).toFixed(1) : "0"} unit="%" sub={t.insight.outOf(calls.toLocaleString(), String(errors))}
        columns={o.days.map((d, i) => col(i, d.tool_calls > 0 ? errorsOf(d) / d.tool_calls : 0, [{ value: d.tool_calls > 0 ? `${((errorsOf(d) / d.tool_calls) * 100).toFixed(1)}%` : "0%", label: t.insight.toolErrorRate }, { value: t.insight.outOf(d.tool_calls.toLocaleString(), String(errorsOf(d))), label: t.insight.toolCallErrors }]))} />
    </div>
  );
}

function SessionStats({ o }: { o: Observed }) {
  const t = useT();
  const [only, setOnly] = useState<string | null>(null);
  const run = running(o);
  const wait = waiting(o);
  const c = o.concurrency;
  const wall = c.one_ms + c.two_ms + c.three_plus_ms;
  const states = [
    { key: "run", label: t.insight.stateRunning, tone: "lo" as const },
    { key: "done", label: t.insight.stateDone, tone: "mid" as const },
    { key: "fail", label: t.insight.stateFailed, tone: "hi" as const },
  ];
  const dayTick = (date: string, i: number) => `${Number(date.slice(8))} ${t.summary.weekdays[i]}`;
  const columns: Column[] = o.days.map((d, i) => {
    const done = d.turn_done_ms + d.question_ms + d.permission_ms;
    const total = d.running_ms + done + d.failed_ms;
    return {
      key: d.date,
      tick: dayTick(d.date, i),
      top: total > 0 ? (total / 3_600_000).toFixed(1) : undefined,
      segments: [
        { value: d.running_ms, tone: "lo", series: "run" },
        { value: done, tone: "mid", series: "done" },
        { value: d.failed_ms, tone: "hi", series: "fail" },
      ],
      tip: {
        title: t.insight.dayTotal(monthDay(d.date), t.summary.weekdays[i], hm(total)),
        rows: [
          { value: hm(d.running_ms), label: t.insight.stateRunning, tone: "lo" },
          { value: hm(done), label: t.insight.stateDone, tone: "mid" },
          { value: d.stop_failures > 0 ? `${hm(d.failed_ms)} · ${t.insight.times(d.stop_failures)}` : hm(d.failed_ms), label: t.insight.stateFailed, tone: "hi" },
        ],
      },
    };
  });
  return (
    <Section title={t.insight.sessionStats} aside={<span className="text-[12px] text-faint tabular-nums">{t.insight.sessionStatsAside(hm(run + wait), hm(wall))}</span>}>
      <div className="grid gap-8 md:grid-cols-[minmax(0,3fr)_minmax(0,2fr)]">
        <div className="min-w-0">
          <p className="mb-2 text-[12px] text-muted">{t.insight.sessionTimeByDay}</p>
          <ul className="mb-3 flex flex-wrap gap-x-3.5 gap-y-1 text-[12px] text-muted" aria-label={t.insight.legendAria}>
            {states.map((s) => (
              <li key={s.key} tabIndex={0} onMouseEnter={() => setOnly(s.key)} onMouseLeave={() => setOnly(null)} onFocus={() => setOnly(s.key)} onBlur={() => setOnly(null)}
                className="flex cursor-default items-center gap-1.5 rounded px-1 -mx-1 outline-none hover:bg-fg/5 hover:text-fg focus-visible:bg-fg/5 motion-safe:transition-colors motion-safe:duration-fast">
                <span aria-hidden className={"h-2.5 w-2.5 rounded-sm " + KEY[s.tone]} />{s.label}
              </li>
            ))}
          </ul>
          <Columns columns={columns} height={200} label={t.insight.sessionTimeByDay} emphasis={only} />
        </div>
        <div className="grid min-w-0 content-start gap-6">
          <div>
            <p className="mb-2 text-[12px] text-muted">{t.insight.projectTime}</p>
            <HBars label={t.insight.projectTime} rows={o.projects.slice(0, 6).map((p) => ({
              name: p.name === "" ? t.insight.otherProject : p.name, value: p.running_ms, text: hm(p.running_ms),
              tip: [{ value: hm(p.running_ms), label: t.insight.workingTime }, { value: run > 0 ? `${Math.round((p.running_ms / run) * 100)}%` : "0%", label: t.insight.shareOfTotal }],
            }))} />
          </div>
          <div>
            <p className="mb-2 text-[12px] text-muted">{t.insight.concurrency(hm(wall))}</p>
            <ShareBar label={t.insight.concurrentSessions} parts={[
              { name: t.insight.concurrent1, value: c.one_ms, tone: "lo", tip: [{ value: hm(c.one_ms), label: t.insight.dedupTime }, { value: wall > 0 ? `${Math.round((c.one_ms / wall) * 100)}%` : "0%", label: t.insight.ofDedupTime }] },
              { name: t.insight.concurrent2, value: c.two_ms, tone: "mid", tip: [{ value: hm(c.two_ms), label: t.insight.dedupTime }, { value: wall > 0 ? `${Math.round((c.two_ms / wall) * 100)}%` : "0%", label: t.insight.ofDedupTime }] },
              { name: t.insight.concurrent3, value: c.three_plus_ms, tone: "hi", tip: [{ value: hm(c.three_plus_ms), label: t.insight.dedupTime }, { value: wall > 0 ? `${Math.round((c.three_plus_ms / wall) * 100)}%` : "0%", label: t.insight.ofDedupTime }] },
            ]} />
            <p className="mt-2 text-[12px] text-faint">{t.insight.overlap(hm(c.overlap_ms))}</p>
          </div>
        </div>
      </div>
      <div className="mt-4">
        <Disclosure label={t.insight.showTable}>
          <div className="overflow-x-auto">
            <table className="w-full text-[12px] tabular-nums">
              <thead className="text-faint"><tr>{[t.insight.colDate, t.insight.stateRunning, t.insight.stateDone, t.insight.stateFailed, t.insight.colStops, t.insight.colToolErrors].map((h) => <th key={h} className="whitespace-nowrap border-b border-line px-2 py-1.5 text-right font-normal first:text-left">{h}</th>)}</tr></thead>
              <tbody>{o.days.map((d, i) => (
                <tr key={d.date} className="hover:bg-fg/5 motion-safe:transition-colors motion-safe:duration-fast">
                  <td className="border-b border-line-soft px-2 py-1.5">{monthDay(d.date)} ({t.summary.weekdays[i]})</td>
                  <td className="border-b border-line-soft px-2 py-1.5 text-right">{hm(d.running_ms)}</td>
                  <td className="border-b border-line-soft px-2 py-1.5 text-right">{hm(d.turn_done_ms + d.question_ms + d.permission_ms)}</td>
                  <td className="border-b border-line-soft px-2 py-1.5 text-right">{hm(d.failed_ms)}</td>
                  <td className="border-b border-line-soft px-2 py-1.5 text-right">{d.stop_failures}</td>
                  <td className="border-b border-line-soft px-2 py-1.5 text-right">{errorsOf(d)}</td>
                </tr>
              ))}</tbody>
            </table>
          </div>
        </Disclosure>
      </div>
      <p className="mt-3 text-[12px] text-faint">{t.insight.idleNote}</p>
    </Section>
  );
}

function Checkpoints({ list, onOpenDay, onOpenProjects }: { list: InsightView["checkpoints"]; onOpenDay: (date: string) => void; onOpenProjects: () => void }) {
  const t = useT();
  if (list.length === 0) return null;
  return (
    <Section title={t.insight.checkpoints}>
      <div className="divide-y divide-line">
        {list.map((c) => {
          const hours = c.kind === "hour_band";
          // Hours with nothing either side of the records stay out of the chart.
          const from = hours ? Math.max(0, c.series.findIndex((p) => p.value > 0)) : 0;
          const to = hours ? 24 - [...c.series].reverse().findIndex((p) => p.value > 0) : c.series.length;
          const columns: Column[] = c.series.slice(from, to).map((p, k) => {
            const i = from + k;
            return {
              key: p.label,
              tick: hours ? (i % 3 === 0 ? p.label : "") : p.label,
              top: c.highlight.includes(i) ? `${Math.round(p.value)}${c.unit}` : undefined,
              segments: [{ value: p.value, tone: c.highlight.includes(i) ? "hi" : "mid" }],
              tip: { title: p.tip.split(" · ")[0], rows: p.tip.split(" · ").slice(1).map((tt) => ({ value: tt, label: "" })) },
            };
          });
          return (
            <div key={c.kind} className="grid items-center gap-6 py-5 first:pt-0 md:grid-cols-2">
              <div className="min-w-0">
                <h3 className="mb-2 text-[15px] font-semibold text-balance">{c.title}</h3>
                <ul className="grid list-disc gap-0.5 pl-4 text-muted">{c.details.map((d) => <li key={d}>{d}</li>)}</ul>
                <div className="mt-3 flex flex-wrap gap-1.5">
                  {c.kind === "day_focus" && c.date && (
                    <button type="button" onClick={() => onOpenDay(c.date!)}
                      className="h-7 rounded-md bg-fg/10 px-2.5 text-[12px] text-fg hover:bg-fg/15 motion-safe:transition-colors motion-safe:duration-fast">{t.insight.viewSummary(monthDay(c.date))}</button>
                  )}
                  {c.kind === "tool_streak" && (
                    <button type="button" onClick={onOpenProjects}
                      className="h-7 rounded-md bg-fg/10 px-2.5 text-[12px] text-fg hover:bg-fg/15 motion-safe:transition-colors motion-safe:duration-fast">{t.insight.viewProjectChecks}</button>
                  )}
                </div>
              </div>
              <div className="min-w-0">
                <Columns columns={columns} height={120} label={c.title} />
                <div className="mt-3">
                  <Disclosure label={t.insight.showTable}>
                    <div className="overflow-x-auto">
                      <table className="w-full text-[12px] tabular-nums" aria-label={t.insight.tableAria(c.title)}>
                        <tbody>{c.series.filter((p) => !hours || p.value > 0).map((p, i) => (
                          <tr key={i} className="hover:bg-fg/5 motion-safe:transition-colors motion-safe:duration-fast">
                            <td className="border-b border-line-soft px-2 py-1.5">{p.tip}</td>
                          </tr>
                        ))}</tbody>
                      </table>
                    </div>
                  </Disclosure>
                </div>
              </div>
            </div>
          );
        })}
      </div>
    </Section>
  );
}

export function InsightScreen({ today, onOpenDay, onOpenProjects, onChangeAgent }: { today: string; onOpenDay: (date: string) => void; onOpenProjects: () => void; onChangeAgent?: () => void }) {
  const t = useT();
  // Rust writes the checkpoint lines in the current language, so the week is read
  // again when the language changes.
  const lang = useLang();
  const [date, setDate] = useKept<string>("insight.date", today);
  const [view, setView] = useState<InsightView | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    setError(null);
    api.insight(date).then((v) => live && setView(v), (e) => live && setError(String(e)));
    return () => { live = false; };
  }, [date, lang]);

  // Follow the week on screen: a finished evaluation or goal change for another week is not shown here.
  const weekRef = useRef<string | null>(null);
  weekRef.current = view?.observed.week_start ?? null;
  const show = (v: InsightView) => {
    if (weekRef.current === null || weekRef.current === v.observed.week_start) setView(v);
  };
  const [actionError, setActionError] = useState<string | null>(null);
  const act = (p: Promise<InsightView>) => {
    setActionError(null);
    p.then(show, (e) => setActionError(String(e)));
  };
  const jobKey = view ? `insight:${view.observed.week_start}` : "insight:";
  const job = useJob(jobKey);
  const make = () => {
    if (!view) return;
    const week = view.observed.week_start;
    startJob(`insight:${week}`, (tt) => tt.insight.evalJob(monthDay(week)), () => api.makeInsight(week)).promise.then(show, () =>
      api.insight(week).then(show, () => {}),
    );
  };

  // The app's own evaluation after the weekly summary lands here too.
  useEffect(() => {
    const un = listen("insight-updated", () => api.insight(date).then(show, () => {}));
    return () => { un.then((f) => f()); };
  }, [date]);

  const o = view?.observed;
  const week = o ? `${monthDay(o.week_start)} – ${monthDay(shiftDate(o.week_start, 6))}` : "";
  const inProgress = o ? o.through < o.week_end : false;
  const empty = o ? o.requests === 0 && o.sessions === 0 : false;
  const thisWeek = o ? today >= o.week_start && today < shiftDate(o.week_start, 7) : true;

  return (
    <div className="mx-auto max-w-[1040px] px-6 pb-16 pt-6">
      <header className="flex flex-wrap items-center justify-between gap-x-4 gap-y-2">
        <h1 className="text-[20px] font-semibold">{t.insight.title}</h1>
        <div className="flex items-center gap-1 text-muted">
          <button type="button" aria-label={t.insight.prevWeek} onClick={() => setDate(shiftDate(o?.week_start ?? date, -7))}
            className="flex h-7 w-7 items-center justify-center rounded-md hover:bg-fg/5 motion-safe:transition-colors motion-safe:duration-fast"><Icon name="chevronLeft" size={14} /></button>
          <span className="min-w-36 text-center text-fg tabular-nums">{week}</span>
          <button type="button" aria-label={t.insight.nextWeek} disabled={thisWeek} onClick={() => setDate(shiftDate(o?.week_start ?? date, 7))}
            className="flex h-7 w-7 items-center justify-center rounded-md hover:bg-fg/5 disabled:opacity-30 motion-safe:transition-colors motion-safe:duration-fast"><Icon name="chevronRight" size={14} /></button>
          <button type="button" disabled={thisWeek} onClick={() => setDate(today)}
            className="ml-1 h-7 rounded-md px-2.5 text-[12px] hover:bg-fg/5 disabled:opacity-30 motion-safe:transition-colors motion-safe:duration-fast">{t.insight.thisWeek}</button>
        </div>
        {o && (
          <p className="w-full text-[12px] text-faint tabular-nums">
            {t.insight.totals(o.requests, o.sessions)}
            {inProgress ? t.insight.inProgress : ""}
            {o.recorded_from ? t.insight.recordedFrom(monthDay(o.recorded_from)) : ""}
          </p>
        )}
      </header>

      {view && view.observed.requests + view.observed.sessions > 0 && <div className="mt-5"><LastGoalLine view={view} onKeep={(key) => act(api.setGoal(view.observed.week_start, key))} /></div>}

      {error ? <div className="mt-8"><Empty>{t.insight.loadFailed(error)}</Empty></div>
        : !view || !o ? <div className="mt-10 flex justify-center text-muted"><Spinner /></div>
        : empty ? <div className="mt-8"><Empty>{t.insight.noRecords}</Empty></div>
        : <div className="mt-8 grid gap-12">
            <section aria-labelledby="ins-points">
              <h2 id="ins-points" className="mb-3 text-[15px] font-semibold">{t.insight.highlights}</h2>
              {view.checkpoints.length === 0
                ? <p className="text-muted">{t.insight.noCheckpoints}</p>
                : <ul className="grid gap-2">{view.checkpoints.map((c) => (
                    <li key={c.kind} className="grid grid-cols-[14px_minmax(0,1fr)] gap-1.5 text-[15px] leading-normal">
                      <span aria-hidden className="mt-2.5 h-1.5 w-1.5 rounded-full bg-fg/40" />{c.line}
                    </li>
                  ))}</ul>}
              <Kpis o={o} />
            </section>
            <SessionStats o={o} />
            <Checkpoints list={view.checkpoints} onOpenDay={onOpenDay} onOpenProjects={onOpenProjects} />
            {view.eval_on && (
              <EvalArea view={view} busy={!!job} onMake={make}
                onDispute={(item: EvalItem) => act(api.insightFeedback(view.observed.week_start, item))}
                onGoal={(key) => act(api.setGoal(view.observed.week_start, key))}
                onChangeAgent={onChangeAgent} />
            )}
            <NextGoal view={view} onPick={(key) => act(api.setGoal(view.observed.week_start, key))} onClear={() => act(api.clearGoal(view.observed.week_start))} />
            {actionError && <p className="text-[12px] text-muted">{actionError}</p>}
          </div>}
    </div>
  );
}
