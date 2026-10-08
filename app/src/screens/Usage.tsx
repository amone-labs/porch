// Usage: tokens and list-price cost (API-equivalent cost) over a span, usage limits
import { useKept } from "../kept";
// per account, and the budget. Numbers in lines and thin bars, no cards.
import { useEffect, useId, useState } from "react";
import { api, type AccountLimits, type LimitWindow, type UsageReport } from "../api";
import { cacheHit, duration, longDate, money, resetTime, tokenTotal, tokens } from "../format";
import { Empty, Headline, Section, StatLine } from "../components/ui";
import { Icon } from "../components/Icon";
import { useLang, useT } from "../i18n";
import type { Dict } from "../i18n/ko";

type Span = 1 | 7 | 30;

/** "2026-09-30" moved by `n` local calendar days. */
function shift(date: string, n: number): string {
  const [y, m, d] = date.split("-").map(Number);
  const x = new Date(y, m - 1, d + n);
  return `${x.getFullYear()}-${String(x.getMonth() + 1).padStart(2, "0")}-${String(x.getDate()).padStart(2, "0")}`;
}

/** "9월 24일~30일", or "9월 24일~10월 1일" across a month; "Sep 24–30", "Sep 24–Oct 1". */
function rangeTitle(t: Dict, start: string, end: string): string {
  const [, sm, sd] = start.split("-").map(Number);
  const [, em, ed] = end.split("-").map(Number);
  return sm === em ? t.usage.rangeSameMonth(sm, sd, ed) : t.usage.rangeAcrossMonths(sm, sd, em, ed);
}

function Bar({ share, strong = false }: { share: number; strong?: boolean }) {
  return (
    <div className="h-1 rounded-full bg-fg/[0.06]">
      <div className={"h-1 rounded-full " + (strong ? "bg-fg/80" : "bg-fg/50")} style={{ width: `${Math.max(0, Math.min(1, share)) * 100}%` }} />
    </div>
  );
}

function LimitRow({ name, w, now }: { name: string; w: LimitWindow | null; now: number }) {
  const t = useT();
  if (!w) return null;
  const hot = w.used_percent >= 80;
  return (
    <div className="grid grid-cols-[9rem_1fr_3.5rem_10rem] items-center gap-3 py-1.5">
      <span className="text-muted">{name}</span>
      <Bar share={w.used_percent / 100} strong={hot} />
      <span className={"text-right " + (hot ? "font-medium text-fg" : "text-muted")}>{t.usage.usedPct(Math.round(w.used_percent))}</span>
      <span className="text-[11.5px] text-faint">{w.resets_at ? t.usage.resetsAt(resetTime(w.resets_at, now)) : t.usage.refilled}</span>
    </div>
  );
}

function Limits({ list, now, statusline, unwrapped }: { list: AccountLimits[] | null; now: number; statusline: string | undefined; unwrapped: string[] }) {
  const t = useT();
  const claude = list?.find((l) => l.agent === "claude");
  return (
    <Section title={t.usage.limits}>
      {list === null ? (
        <p className="py-2 text-muted">{t.common.loading}</p>
      ) : (
        <>
          {list.map((l) => (
            <div key={l.label} className="py-1">
              <div className="text-[12px] text-fg/90">{l.label}</div>
              {l.agent === "codex" && l.label.includes("(Orca") && <p className="mt-1 text-[11px] text-faint">{t.usage.orcaNote}</p>}
              <LimitRow name={t.usage.fiveHour} w={l.five_hour} now={now} />
              <LimitRow name={t.usage.weekly} w={l.weekly} now={now} />
              <LimitRow name={t.usage.spend} w={l.spend} now={now} />
            </div>
          ))}
          {!claude && (
            <p className="py-2 text-[12px] text-muted">
              {statusline === "disconnected"
                ? t.usage.statusDisconnected
                : statusline === "on" && unwrapped.length > 0
                  ? t.usage.statusUnwrapped(unwrapped[0] + (unwrapped.length > 1 ? t.usage.statusProjects(unwrapped.length) : ""))
                  : statusline === "on"
                  ? t.usage.statusPending
                  : t.usage.statusOff}
            </p>
          )}
        </>
      )}
    </Section>
  );
}

/** Full-height hit areas keep small and zero values easy to inspect. */
function Chart({ r, span, showCost, onOpenDay }: { r: UsageReport; span: Span; showCost: boolean; onOpenDay: (date: string) => void }) {
  const t = useT();
  const [hovered, setHovered] = useState<number | null>(null);
  const [focused, setFocused] = useState<number | null>(null);
  // Pointer position inside the plot; the tooltip follows it. Keyboard focus has none and pins it to the bar.
  const [pointer, setPointer] = useState<{ x: number; y: number; w: number } | null>(null);
  const [metric, setMetric] = useKept<"cost" | "tokens">("usage.metric", "cost");
  const tooltipId = useId();
  const cost = showCost && (span === 1 || metric === "cost");
  const dates = [...new Set([...r.days.map(d => d.date), ...r.unknown_days])].sort();
  const range = span === 1 ? r.end : dates.length ? `${dates[0]} — ${dates[dates.length - 1]}` : t.usage.noRecords;
  const points = span === 1
    ? Array.from({ length: 24 }, (_, h) => ({
        label: t.usage.hourLabel(range, h), tick: String(h), date: undefined as string | undefined,
        cost: r.total.by_hour[h] ?? 0, tokens: null as number | null, minutes: null as number | null,
        unknown: r.unknown_days.length > 0,
      }))
    : dates.map(date => {
        const day = r.days.find(d => d.date === date);
        return { label: date, tick: date.slice(5), date, cost: day?.usage.cost ?? 0,
          tokens: day ? tokenTotal(day.usage.tokens) : null, minutes: day?.minutes ?? null,
          unknown: r.unknown_days.includes(date) || !day };
      });
  const value = (p: typeof points[number]) => cost ? p.cost : p.tokens ?? 0;
  const max = Math.max(...points.filter(p => !p.unknown).map(value), cost ? 0.01 : 1);
  const active = hovered ?? focused;
  const selected = active === null ? undefined : points[active];
  const describe = (p: typeof points[number]) => p.unknown ? t.usage.noRecordsLabel(p.label) :
    `${p.label}${showCost ? t.usage.costSuffix(money(p.cost)) : ""}${p.tokens !== null ? t.usage.tokensSuffix(p.tokens.toLocaleString()) : ""}${p.minutes !== null ? t.usage.workSuffix(duration(p.minutes)) : ""}`;

  return (
    <Section title={span === 1 ? t.usage.hourly : t.usage.daily} aside={
      span > 1 && showCost ? <div className="flex gap-1" role="group" aria-label={t.usage.metricAria}>
        {([['cost', t.usage.cost], ['tokens', t.usage.tokens]] as const).map(([key, label]) => (
          <button key={key} type="button" aria-pressed={metric === key} onClick={() => setMetric(key)}
            className={"h-7 rounded-md px-2 text-[12px] motion-safe:transition-colors motion-safe:duration-fast " + (metric === key ? "bg-fg/10 text-fg" : "text-muted hover:bg-fg/5")}>{label}</button>
        ))}
      </div> : <span className="text-[11px] text-faint">{cost ? t.usage.costUsd : t.usage.tokens}</span>
    }>
      <p className="mb-4 text-[12px] text-muted">{range}</p>
      {span === 1 && !showCost ? <Empty>{t.usage.noHourlyTokens}</Empty> : points.length === 0 ? <Empty>{t.usage.emptyRange}</Empty> : <>
        <div className="relative pl-14">
          <div className="pointer-events-none absolute inset-x-0 top-0 h-[180px]" aria-hidden>
            {[1, 0.5, 0].map(fraction => <div key={fraction} className="absolute right-0 left-0 flex items-center gap-2" style={{ top: `${(1 - fraction) * 100}%` }}>
              <span className="w-12 shrink-0 -translate-y-1/2 text-right font-mono text-[10px] text-faint">{cost ? money(max * fraction) : tokens(max * fraction)}</span>
              <span className="flex-1 border-t border-line-soft" />
            </div>)}
          </div>
          <div className="relative flex h-[180px] gap-1"
            onMouseMove={e => { const b = e.currentTarget.getBoundingClientRect(); setPointer({ x: e.clientX - b.left, y: e.clientY - b.top, w: b.width }); }}
            onMouseLeave={() => { setHovered(null); setPointer(null); }}>
            {points.map((p, i) => <button key={p.label} type="button"
              aria-label={describe(p) + (p.date && !p.unknown ? t.usage.viewDay : "")}
              aria-describedby={active === i ? tooltipId : undefined}
              onMouseEnter={() => setHovered(i)} onFocus={() => setFocused(i)} onBlur={() => setFocused(null)}
              onKeyDown={e => { if (e.key === "Escape") { setFocused(null); setHovered(null); } }}
              onClick={() => { if (p.date && !p.unknown) onOpenDay(p.date); }}
              className={"relative flex min-w-0 flex-1 items-end justify-center rounded-sm outline-none focus-visible:ring-1 focus-visible:ring-fg/60 motion-safe:transition-colors motion-safe:duration-fast " + (active === i ? "bg-fg/5 " : "") + (p.date && !p.unknown ? "cursor-pointer" : "cursor-default")}>
              {/* Opaque base: the gridlines behind must not show through the translucent fill. */}
              <span aria-hidden className={"w-full max-w-12 overflow-hidden rounded-t-sm " + (p.unknown ? "border border-dashed border-faint" : "bg-canvas")}
                style={{ height: p.unknown ? "8px" : value(p) > 0 ? `${Math.max(1, value(p) / max * 100)}%` : "1px" }}>
                {!p.unknown && <span className={"block h-full w-full motion-safe:transition-colors motion-safe:duration-fast " + (active === i ? "bg-fg/80" : "bg-fg/[0.35]")} />}
              </span>
            </button>)}
            {selected && active !== null && <div id={tooltipId} role="tooltip"
              className="pointer-events-none absolute top-2 z-10 w-52 rounded-md border border-line bg-elevated p-3 text-[12px] shadow-lg"
              // Follow the pointer with transform, not left/top: when only left/top of this fixed-width box
              // changes, WebKit moves it without laying out its text again and the first hovered bar's text stays.
              style={hovered !== null && pointer
                ? { left: 0, top: 0, transform: `translate(${Math.min(Math.max(0, pointer.x + 14), pointer.w - 208)}px, ${Math.max(0, pointer.y - 16)}px)` }
                : { left: `clamp(0px, ${(active + 0.5) / points.length * 100}% - 104px, calc(100% - 208px))` }}>
              <p className="mb-2 font-medium text-fg">{selected.label}</p>
              {selected.unknown ? <p className="text-muted">{t.usage.unknownUsage}</p> : <div className="space-y-1 text-muted">
                {showCost && <p className="flex justify-between"><span>{t.usage.cost}</span><span className="text-fg">{money(selected.cost)}</span></p>}
                <p className="flex justify-between"><span>{t.usage.tokens}</span><span className="text-fg">{selected.tokens === null ? t.usage.noHourlyRecord : selected.tokens.toLocaleString()}</span></p>
                {selected.minutes !== null && <p className="flex justify-between"><span>{t.usage.workTimeLabel}</span><span className="text-fg">{duration(selected.minutes)}</span></p>}
                {selected.date && <p className="pt-2 text-[11px] text-faint">{t.usage.clickForDay}</p>}
              </div>}
            </div>}
          </div>
          <div className="mt-2 flex gap-1" aria-hidden>
            {points.map((p, i) => <span key={p.label} className="min-w-0 flex-1 text-center font-mono text-[10px] text-faint">
              {(span === 1 ? i % 6 === 0 || i === 23 : points.length <= 7 || i === 0 || i === points.length - 1 || i % 7 === 0) ? p.tick : ""}
            </span>)}
          </div>
        </div>
        <p className="mt-3 text-[11px] text-faint">{span === 1 ? t.usage.hourBarNote : t.usage.dayBarNote}</p>
      </>}
    </Section>
  );
}

type Group = UsageReport["by_project"][number];

function groupCost(t: Dict, cost: number, unpriced: number, total: number): string {
  if (unpriced > 0 && unpriced >= total) return t.usage.noPrice;
  return money(cost) + (unpriced > 0 ? t.usage.partial : "");
}

/** A 2px line over the span, scaled to its own peak; flat and faint when empty. */
function Spark({ values, label }: { values: number[]; label: string }) {
  const W = 96;
  const H = 20;
  if (values.length < 2) return <span className="w-24" />;
  const max = Math.max(...values);
  const pts = values.map((v, i) => `${(i / (values.length - 1)) * W},${max > 0 ? H - 2 - (v / max) * (H - 4) : H - 2}`).join(" ");
  return (
    <svg width={W} height={H} viewBox={`0 0 ${W} ${H}`} className="shrink-0 text-muted" role="img" aria-label={label}>
      <title>{label}</title>
      <polyline points={pts} fill="none" stroke="currentColor" strokeWidth={max > 0 ? 1.5 : 1} strokeLinejoin="round" strokeLinecap="round" opacity={max > 0 ? 1 : 0.4} />
    </svg>
  );
}

const SHOWN = 6;
const COLS = "grid grid-cols-[1.25rem_minmax(0,1fr)_6rem_5.5rem_3.5rem_6rem] items-center gap-3";
/** Today has no trend column: one day's hours are noise, not a trend. */
const COLS_DAY = "grid grid-cols-[1.25rem_minmax(0,1fr)_6rem_5.5rem_3.5rem] items-center gap-3";

/**
 * Where the span's usage went, as a ranked table: rank, name, cost, tokens,
 * share, and a line of its cost per day (or per hour for today). Numbers do
 * the comparing; the line shows when.
 */
function Groups({ title, rows, showCost, span, aside }: { title: string; rows: Group[]; showCost: boolean; span: Span; aside?: React.ReactNode }) {
  const t = useT();
  if (rows.length === 0) return <Section title={title} aside={aside}><Empty>{t.usage.emptyRange}</Empty></Section>;
  const byCost = showCost && rows.some((g) => g.usage.cost > 0);
  const value = (g: Group) => (byCost ? g.usage.cost : tokenTotal(g.usage.tokens));
  const total = rows.reduce((a, g) => a + value(g), 0);
  const shown = rows.slice(0, rows.length > SHOWN + 1 ? SHOWN : rows.length);
  const rest = rows.slice(shown.length);
  const pct = (v: number) => {
    const p = total > 0 ? (v / total) * 100 : 0;
    return p > 0 && p < 1 ? "<1%" : `${Math.round(p)}%`;
  };
  const series = (g: Group) => (byCost ? g.series_cost : g.series_tokens);
  const trend = span > 1;
  const cols = trend ? COLS : COLS_DAY;
  const trendLabel = (name: string) => t.usage.trendLabel(name, byCost ? t.usage.cost : t.usage.tokens);
  const restCost = rest.reduce((a, g) => a + g.usage.cost, 0);
  const restTokens = rest.reduce((a, g) => a + tokenTotal(g.usage.tokens), 0);
  const restUnpriced = rest.reduce((a, g) => a + g.usage.unpriced, 0);

  return (
    <Section title={title} aside={aside}>
      <div className={cols + " -mx-2 px-2 pb-1 text-[10.5px] text-faint"}>
        <span />
        <span />
        <span className="text-right">{showCost ? t.usage.costShort : ""}</span>
        <span className="text-right">{t.usage.tokens}</span>
        <span className="text-right">{byCost ? t.usage.costShare : t.usage.tokenShare}</span>
        {trend && <span>{t.usage.trend}</span>}
      </div>
      <ol>
        {shown.map((g, i) => (
          <li key={g.name} className={cols + " -mx-2 rounded-md border-t border-line-soft px-2 py-2 motion-safe:transition-colors motion-safe:duration-fast hover:bg-fg/[0.04]"}>
            <span className="text-right font-mono text-[11px] text-faint">{i + 1}</span>
            <span className={"truncate " + (i === 0 ? "text-fg" : "text-fg/90")} title={g.name}>
              {g.name}
            </span>
            <span className="text-right text-[12.5px] text-fg/90">{showCost ? groupCost(t, g.usage.cost, g.usage.unpriced, tokenTotal(g.usage.tokens)) : ""}</span>
            <span className="text-right text-[12px] text-muted">{tokens(tokenTotal(g.usage.tokens))}</span>
            <span className="space-y-1 text-right text-[12px] text-muted">
              <span className="block">{byCost && g.usage.unpriced > 0 && g.usage.unpriced >= tokenTotal(g.usage.tokens) ? "—" : pct(value(g))}</span>
              <Bar share={total > 0 ? value(g) / total : 0} />
            </span>
            {trend && (byCost && g.usage.unpriced > 0 && g.usage.unpriced >= tokenTotal(g.usage.tokens) ? <span className="text-[11px] text-faint">{t.usage.noPrice}</span> : series(g).length > 1 ? <Spark values={series(g)} label={trendLabel(g.name)} /> : <span />)}
          </li>
        ))}
        {rest.length > 0 && (
          <li className={cols + " -mx-2 rounded-md border-t border-line-soft px-2 py-2 motion-safe:transition-colors motion-safe:duration-fast hover:bg-fg/[0.04]"} title={rest.map((g) => g.name).join(", ")}>
            <span />
            <span className="truncate text-muted">{t.usage.rest(rest.length)}</span>
            <span className="text-right text-[12.5px] text-muted">{showCost ? groupCost(t, restCost, restUnpriced, restTokens) : ""}</span>
            <span className="text-right text-[12px] text-muted">{tokens(restTokens)}</span>
            <span className="text-right text-[12px] text-muted">{pct(rest.reduce((a, g) => a + value(g), 0))}</span>
            {trend && <span />}
          </li>
        )}
      </ol>
      {showCost && rows.some(g => g.usage.unpriced > 0) && <p className="mt-2 text-[11px] text-faint">{t.usage.noPriceNote}</p>}
    </Section>
  );
}

/**
 * The range ends at `end`: Day shows that one day, 7 days/30 days the days up
 * to it. The arrows and ←/→ move by the span, Today comes back, and a day bar
 * on 7 days/30 days opens that day's usage.
 */
export function Usage({ today, onOpenDay }: { today: string; onOpenDay: (date: string) => void }) {
  const t = useT();
  // Rust writes the "Temporary folders" row in the current language, so the
  // report is read again when the language changes.
  const lang = useLang();
  const [group, setGroup] = useKept<"by_project" | "by_model" | "by_agent">("usage.group", "by_project");
  const [span, setSpan] = useKept<Span>("usage.span", 1);
  const [end, setEnd] = useKept("usage.end", today);
  const [report, setReport] = useState<UsageReport | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [limits, setLimits] = useState<AccountLimits[] | null>(null);
  const [statusline, setStatusline] = useState<string>();
  const [unwrapped, setUnwrapped] = useState<string[]>([]);
  const [showCost, setShowCost] = useState(true);
  const now = Date.now();

  useEffect(() => {
    setReport(null);
    setError(null);
    let current = true;
    api.usage(span, end).then(r => { if (current) setReport(r); }, e => { if (current) setError(String(e)); });
    return () => { current = false; };
  }, [span, end, lang]);

  const atToday = end >= today;
  const move = (dir: -1 | 1) => {
    if (dir > 0 && atToday) return;
    const next = shift(end, dir * span);
    setEnd(next > today ? today : next);
  };
  const openDay = (date: string) => {
    setSpan(1);
    setEnd(date);
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

  useEffect(() => {
    const load = () => api.limits().then(setLimits, () => setLimits([]));
    load();
    api.settings().then((s) => {
      setStatusline(s.statusline);
      setUnwrapped(s.statusline_projects.filter((p) => !p.wrapped).map((p) => p.root.split("/").filter(Boolean).pop() ?? p.root));
      setShowCost(s.settings.show_cost);
    });
    const id = setInterval(load, 60_000);
    return () => clearInterval(id);
  }, []);

  const spans: { days: Span; label: string }[] = [
    { days: 1, label: t.usage.spanDay },
    { days: 7, label: t.usage.span7 },
    { days: 30, label: t.usage.span30 },
  ];
  const unit = span === 1 ? t.usage.unitDay : t.usage.unitDays(span);
  const title = span === 1 ? `${longDate(end)}${end === today ? t.summary.todayMark : ""}` : rangeTitle(t, shift(end, 1 - span), end);
  const iconButton = (dir: -1 | 1) => (
    <button
      type="button"
      aria-label={dir < 0 ? t.summary.prevSpan(unit) : t.summary.nextSpan(unit)}
      disabled={dir > 0 && atToday}
      onClick={() => move(dir)}
      className="inline-flex h-7 w-7 items-center justify-center rounded-md text-muted hover:bg-fg/5 hover:text-fg disabled:opacity-30 disabled:hover:bg-transparent"
    >
      <Icon name={dir < 0 ? "chevronLeft" : "chevronRight"} size={14} />
    </button>
  );

  const tabs = (
    <div role="tablist" aria-label={t.usage.period} className="ml-auto flex shrink-0 rounded-lg border border-line p-0.5">
      {spans.map((s) => (
        <button
          key={s.days}
          type="button"
          role="tab"
          aria-selected={span === s.days}
          onClick={() => setSpan(s.days)}
          className={"h-6 rounded-md px-3 text-[12px] " + (span === s.days ? "bg-fg/10 font-medium text-fg" : "text-muted hover:text-fg")}
        >
          {s.label}
        </button>
      ))}
    </div>
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
        disabled={atToday}
        onClick={() => setEnd(today)}
        className="h-7 shrink-0 rounded-md border border-line px-2.5 text-[12px] text-muted hover:bg-fg/5 hover:text-fg disabled:opacity-40 disabled:hover:bg-transparent"
      >
        {t.summary.today}
      </button>
      {span === 1 && (
        <button
          type="button"
          onClick={() => onOpenDay(end)}
          className="h-7 shrink-0 rounded-md px-2.5 text-[12px] text-muted hover:bg-fg/5 hover:text-fg"
        >
          {t.usage.daySummary}
        </button>
      )}
      {tabs}
    </div>
  );

  const total = report?.total;
  const hit = total ? cacheHit(total.tokens) : null;
  const perHour = total && report && report.active_minutes > 0 ? total.cost / (report.active_minutes / 60) : null;

  return (
    <div>
      {toolbar}
      <Headline eyebrow={t.nav.usage}>
        {!total ? t.usage.collecting : showCost ? t.usage.costAndTokens(money(total.cost), tokens(tokenTotal(total.tokens))) : t.usage.onlyTokens(tokens(tokenTotal(total.tokens)))}
      </Headline>
      {error && <Empty>{t.summary.loadFailed(error)}</Empty>}
      {!report && !error && (
        <p className="text-muted">{t.usage.firstRead}</p>
      )}
      {report && total && (
        <>
          <StatLine
            items={[
              hit !== null && t.usage.cacheHits(hit),
              report.active_minutes > 0 && t.usage.workTime(duration(report.active_minutes)),
              showCost && perHour !== null && t.usage.perHour(money(perHour)),
              total.unpriced > 0 && t.usage.unpricedTokens(tokens(total.unpriced)),
            ]}
          />
          <Chart key={`${span}-${end}`} r={report} span={span} showCost={showCost} onOpenDay={openDay} />
          {showCost && <p className="mt-2 text-[11px] text-faint">{t.usage.costNote}</p>}
          <Groups title={t.usage.breakdown} rows={report[group]} showCost={showCost} span={span} aside={
            <div role="group" aria-label={t.usage.groupAria} className="flex gap-1">
              {([["by_project", t.usage.byProject], ["by_model", t.usage.byModel], ["by_agent", t.usage.byAgent]] as const).map(([key, label]) => (
                <button key={key} type="button" aria-pressed={group === key} onClick={() => setGroup(key)}
                  className={"h-7 rounded-md px-2 text-[12px] motion-safe:transition-colors motion-safe:duration-fast " + (group === key ? "bg-fg/10 text-fg" : "text-muted hover:bg-fg/5")}>{label}</button>
              ))}
            </div>
          } />
          <Limits list={limits} now={now} statusline={statusline} unwrapped={unwrapped} />
          {showCost && !!report.monthly_budget && (
            <Section title={t.usage.budget}>
              <div className="grid grid-cols-[1fr_14rem] items-center gap-3 py-1.5">
                <Bar share={report.month_cost / report.monthly_budget} strong={report.month_cost >= report.monthly_budget * 0.8} />
                <span className="text-right text-[12px] text-muted">
                  {t.usage.budgetLine(money(report.month_cost), money(report.monthly_budget), tokens(report.month_tokens))}
                </span>
              </div>
            </Section>
          )}

          <p className="mt-10 text-[11.5px] text-faint">
            {t.usage.footnote(report.prices_as_of)}
            {report.by_model.some(g => g.name.includes("/") && g.usage.cost > 0) && t.usage.openRouterNote}
            {report.unknown_days.length > 0 && t.usage.missingDays(report.unknown_days.length)}
          </p>
        </>
      )}
    </div>
  );
}
