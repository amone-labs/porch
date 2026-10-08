import type { WeekReport } from "../api";
import { commitsLabel, duration, money, shortDuration, tokens, writtenBy } from "../format";
import { useShowCost } from "../prefs";
import { GenerateButton, useRefreshKey } from "./DayView";
import { Bullets, Headline, Section, StatLine } from "./ui";
import { ExportMenu } from "./ExportMenu";
import { Icon } from "./Icon";
import { Blockers, Diagrams, TimeSpent, useFocus } from "./Insight";
import { useLang, useT } from "../i18n";

function addDays(date: string, n: number): string {
  const [y, m, d] = date.split("-").map(Number);
  const x = new Date(y, m - 1, d + n);
  return `${x.getFullYear()}-${String(x.getMonth() + 1).padStart(2, "0")}-${String(x.getDate()).padStart(2, "0")}`;
}


export function WeekView({
  report,
  busy,
  onGenerate,
  onOpenDay,
  strip = true,
  span = "week",
  insight,
  onOpenInsight,
}: {
  report: WeekReport;
  /** "month" renders a month report in the same layout (no strip; month words). */
  span?: "week" | "month";
  /** The seven-day strip; off where a calendar above already shows the days. */
  strip?: boolean;
  busy: number | null;
  onGenerate: (refresh: boolean) => void;
  onOpenDay: (date: string) => void;
  /** This week's insight counts, for the card at the end (weeks only). */
  insight?: { checkpoints: number; scored: number } | null;
  onOpenInsight?: () => void;
}) {
  const s = report.summary;
  const t = useT();
  const lang = useLang();
  const total = report.days.reduce((a, d) => a + d.minutes, 0);
  const commits = report.days.reduce((a, d) => a + d.commits, 0);
  // Only when every day knows whose commits were whose.
  const mine = report.days.every((d) => d.my_commits != null) ? report.days.reduce((a, d) => a + (d.my_commits ?? 0), 0) : null;
  const showCost = useShowCost();
  const cost = report.days.reduce((a, d) => a + (d.cost ?? 0), 0);
  const toks = report.days.reduce((a, d) => a + (d.tokens ?? 0), 0);
  const perProject = new Map<string, number>();
  for (const d of report.days) for (const [p, m] of Object.entries(d.projects)) perProject.set(p, (perProject.get(p) ?? 0) + m);
  const projects = [...perProject.entries()].sort((a, b) => b[1] - a[1]);
  useRefreshKey(report.days.length > 0 && busy === null, () => onGenerate(!!s));
  const { on } = useFocus();

  return (
    <div>
      <Headline
        eyebrow={
          <>
            <span className="print:hidden">{span === "month" ? t.summary.monthEyebrow : t.summary.weekEyebrow}</span>
            <span className="hidden print:inline">
              {span === "month"
                ? t.summary.monthTitle(Number(report.week_start.slice(0, 4)), Number(report.week_start.slice(5, 7)))
                : t.summary.weekPrint(report.week_start)}
            </span>
          </>
        }
        action={
          report.days.length > 0 && (
            <div className="flex items-center gap-2">
              <GenerateButton has={!!s} busy={busy} onGenerate={onGenerate} label={span === "month" ? t.summary.makeMonth : t.summary.makeWeek} />
              <ExportMenu kind={span} date={report.week_start} disabled={!s} />
            </div>
          )
        }
      >
        {s
          ? s.headline
          : report.days.length === 0
            ? span === "month" ? t.summary.noWorkMonth : t.summary.noWorkWeek
            : span === "month" ? t.summary.notMadeMonth : t.summary.notMadeWeek}
      </Headline>

      {s && (s.lang ?? "ko") !== lang && (
        <p className="text-[12px] text-faint print:hidden">
          {t.summary.otherLanguage((s.lang ?? "ko") === "ko" ? t.lang.koName : t.lang.enName)}{" "}
          <button className="underline-offset-2 hover:underline" onClick={() => onGenerate(true)} disabled={busy !== null}>
            {t.summary.rewrite}
          </button>
        </p>
      )}

      <StatLine
        items={[
          duration(total),
          t.summary.daysRecorded(report.days.length),
          t.summary.projects(projects.length),
          commitsLabel(commits, mine),
          showCost && cost > 0 && t.summary.cost(money(cost)),
          toks > 0 && t.summary.tokens(tokens(toks)),
        ]}
      />
      {strip && span === "week" && <div className="mt-4 grid grid-cols-7 overflow-hidden rounded-lg border border-line-soft">
        {t.summary.weekdays.map((name, i) => {
          const date = addDays(report.week_start, i);
          const d = report.days.find((x) => x.date === date);
          return (
            <button
              key={date}
              type="button"
              disabled={!d}
              onClick={() => d && onOpenDay(date)}
              className={
                "flex h-[100px] flex-col gap-0.5 px-1.5 py-1 text-left motion-safe:transition-colors motion-safe:duration-fast disabled:cursor-default " +
                (i < 6 ? "border-r border-line-soft " : "") +
                (d ? "hover:bg-fg/[0.05]" : "")
              }
            >
              <span className="flex items-baseline justify-between">
                <span className={"text-[12px] " + (d ? "text-fg/90" : "text-faint")}>
                  {name} {Number(date.slice(8))}
                </span>
                {d && <span className="text-[11px] text-muted">{shortDuration(d.minutes)}</span>}
              </span>
              {d?.headline && <span className="line-clamp-4 text-[11.5px] leading-snug text-muted">{d.headline}</span>}
            </button>
          );
        })}
      </div>}

      {report.summary_error && (
        <p className="mt-6 text-muted">
          {report.summary ? t.summary.keptPrevious : t.summary.failed}: {report.summary_error}
        </p>
      )}

      {s && <TimeSpent blocks={s.time ?? []} on={on} total={total} />}
      {s && <Blockers items={s.blockers ?? []} on={on} />}
      {s && <Diagrams items={s.visuals ?? []} />}

      {projects.length > 0 && (
        <Section title={t.summary.byProject}>
          <div className="divide-y divide-line-soft">
            {projects.map(([name, minutes]) => {
              const p = s?.projects.find((x) => x.name === name);
              return (
                <article key={name} className="py-5 first:pt-2">
                  <div className="flex items-baseline gap-3">
                    <h3 className="text-[15px] font-medium">{name}</h3>
                    <span className="text-[11.5px] text-faint">{duration(minutes)}</span>
                  </div>
                  {p && (
                    <>
                      <p className="mt-2 max-w-[80ch] text-fg/90">{p.summary}</p>
                      <div className="mt-3 max-w-[80ch]">
                        <Bullets title={t.summary.done} items={p.shipped} />
                        <Bullets title={t.summary.inProgress} items={p.ongoing} />
                        <Bullets title={t.summary.next} items={p.next} />
                      </div>
                    </>
                  )}
                </article>
              );
            })}
          </div>
        </Section>
      )}

      {s && (s.analysis.observations.length > 0 || s.analysis.suggestions.length > 0) && (
        <Section title={t.summary.notesSuggestions}>
          <div className="max-w-[80ch] pt-1">
            <Bullets title={t.summary.notes} items={s.analysis.observations} />
            <Bullets title={t.summary.suggestions} items={s.analysis.suggestions} />
          </div>
        </Section>
      )}
      {span === "week" && insight && onOpenInsight && (
        <button type="button" onClick={onOpenInsight}
          className="mt-12 flex w-full items-center gap-3 rounded-lg border border-line px-4 py-3 text-left hover:bg-fg/5 motion-safe:transition-colors motion-safe:duration-fast">
          <Icon name="insight" size={15} />
          <span className="flex-1">{t.summary.insightCard(insight.checkpoints, insight.scored)}</span>
          <Icon name="chevronRight" size={14} />
        </button>
      )}
      {s && report.model && (
        <p className="mt-12 text-[11.5px] text-faint">
          {span === "month"
            ? t.summary.writtenMonth(writtenBy(report.provider, report.model))
            : t.summary.writtenWeek(writtenBy(report.provider, report.model))}
        </p>
      )}
    </div>
  );
}
