import { useEffect } from "react";
import type { DayReport } from "../api";
import { cacheHit, commitsLabel, duration, hhmm, homeRelative, longDate, money, tokenTotal, tokens, writtenBy } from "../format";
import { useShowCost } from "../prefs";
import { Bullets, Button, Disclosure, Headline, HoursStrip, Section, Spinner, StatLine, useElapsed } from "./ui";
import { Icon } from "./Icon";
import { ExportMenu } from "./ExportMenu";
import { DayOpenWork } from "./OpenWork";
import { Blockers, Diagrams, TimeSpent, Timeline, useFocus } from "./Insight";
import { useEnv } from "../env";
import { useLang, useT } from "../i18n";

/** ⇧⌘R regenerates the summary on whichever report view is showing. It runs the
 * model (time and money), so it is not plain ⌘R, the reload habit; that one does nothing
 * here rather than reload the window. */
export function useRefreshKey(enabled: boolean, run: () => void) {
  useEffect(() => {
    if (!enabled) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.metaKey && e.key.toLowerCase() === "r") {
        e.preventDefault();
        if (e.shiftKey) run();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [enabled, run]);
}

/** The shortcut, shown in the button: the action runs the model, so it should be found, not guessed. */
function Shortcut() {
  return (
    <kbd aria-hidden className="ml-1 font-mono text-[11px] font-normal opacity-60">
      ⇧⌘R
    </kbd>
  );
}

export function GenerateButton({ has, busy, onGenerate, label }: { has: boolean; busy: number | null; onGenerate: (refresh: boolean) => void; label: string }) {
  const secs = useElapsed(busy);
  const env = useEnv();
  const t = useT();
  if (env && !env.claude_bin && !env.codex_bin)
    return (
      <span className="max-w-[18rem] text-right text-[11.5px] text-muted" title={t.common.noWriter}>
        {t.summary.noWriterHint}
      </span>
    );
  if (busy !== null) return <span className="inline-flex items-center gap-1.5 text-[12px] text-muted print:hidden">
        <Spinner />
        {t.summary.making(secs)}
      </span>;
  return has ? (
    <span className="print:hidden">
      <Button onClick={() => onGenerate(true)} title="⇧⌘R">
        <Icon name="refresh" size={13} /> {t.summary.rewrite}
        <Shortcut />
      </Button>
    </span>
  ) : (
    <span className="print:hidden">
      <Button primary onClick={() => onGenerate(false)} title="⇧⌘R">
        {label}
        <Shortcut />
      </Button>
    </span>
  );
}

export function DayView({ report, busy, onGenerate }: { report: DayReport; busy: number | null; onGenerate: (refresh: boolean) => void }) {
  const t = useT();
  const lang = useLang();
  const d = report.digest;
  const m = d.metrics;
  const s = report.summary;
  const empty = d.projects.length === 0;
  useRefreshKey(!empty && busy === null, () => onGenerate(!!s));
  const { focus, on } = useFocus();
  const showCost = useShowCost();
  // What each turn spent, for the time blocks that group turns.
  const spent = new Map(d.projects.flatMap((p) => p.sessions.flatMap((x) => (x.turns ?? []).map((turn) => [turn.id, turn] as const))));
  const hasTurns = d.projects.some((p) => p.sessions.some((x) => (x.turns ?? []).length > 0));

  return (
    <div>
      <Headline
        eyebrow={
          <>
            <span className="print:hidden">{t.summary.dayEyebrow}</span>
            <span className="hidden print:inline">{longDate(report.date)}</span>
          </>
        }
        action={
          !empty && (
            <div className="flex items-center gap-2">
              <GenerateButton has={!!s} busy={busy} onGenerate={onGenerate} label={t.summary.make} />
              <ExportMenu kind="day" date={report.date} disabled={!s} />
            </div>
          )
        }
      >
        {s ? s.headline : empty ? t.summary.noWorkDay : t.summary.notMadeDay}
      </Headline>

      {s && (s.lang ?? "ko") !== lang && (
        <p className="text-[12px] text-faint print:hidden">
          {t.summary.otherLanguage((s.lang ?? "ko") === "ko" ? t.lang.koName : t.lang.enName)}{" "}
          <button className="underline-offset-2 hover:underline" onClick={() => onGenerate(true)} disabled={busy !== null}>
            {t.summary.rewrite}
          </button>
        </p>
      )}

      {!empty && (
        <div className="space-y-3">
          <StatLine
            items={[
              duration(m.active_minutes),
              t.summary.projects(d.projects.length),
              t.summary.sessions(m.sessions),
              t.summary.requests(m.prompts),
              commitsLabel(m.commits, m.my_commits),
              m.commits > 0 && `+${m.insertions} −${m.deletions}`,
              m.usage && showCost && m.usage.cost > 0 && t.summary.cost(money(m.usage.cost)),
              m.usage && t.summary.tokens(tokens(tokenTotal(m.usage.tokens))),
              m.usage && cacheHit(m.usage.tokens) !== null && t.summary.cacheHit(cacheHit(m.usage.tokens) ?? 0),
            ]}
          />
          {!hasTurns && <HoursStrip byHour={m.by_hour} />}
        </div>
      )}
      {hasTurns && <Timeline projects={d.projects} blockers={s?.blockers ?? []} focus={focus} />}

      {report.summary_error && (
        <p className="mt-6 text-muted">
          {report.summary ? t.summary.keptPrevious : t.summary.failed}: {report.summary_error}
        </p>
      )}

      {s && <TimeSpent blocks={s.time ?? []} on={on} usual={report.usual_minutes} total={m.active_minutes} spent={spent} showCost={showCost} />}
      {s && <Blockers items={s.blockers ?? []} on={on} />}
      {s && <DayOpenWork date={report.date} />}
      {s && <Diagrams items={s.visuals ?? []} />}

      {!empty && (
        <Section title={t.summary.byProject}>
          <div className="divide-y divide-line-soft">
            {d.projects.map((p) => {
              const ps = s?.projects.find((x) => x.name === p.name);
              return (
                <article key={p.root} className="py-5 first:pt-2">
                  <div className="flex items-baseline gap-3" title={p.root}>
                    <h3 className="text-[15px] font-medium">{p.name}</h3>
                    <span className="text-[11.5px] text-faint">
                      {duration(p.metrics.active_minutes)} · {t.summary.sessions(p.metrics.sessions)} · {commitsLabel(p.metrics.commits, p.metrics.my_commits)}
                    </span>
                  </div>
                  {ps && (
                    <>
                      <p className="mt-2 max-w-[80ch] text-fg/90">{ps.summary}</p>
                      <div className="mt-3 max-w-[80ch]">
                        <Bullets title={t.summary.done} items={ps.done} />
                        <Bullets title={t.summary.inProgress} items={ps.in_progress} />
                        <Bullets title={t.summary.next} items={ps.next} />
                      </div>
                    </>
                  )}
                  <div className="mt-2 flex gap-5">
                    {p.commits.length > 0 && (
                      <Disclosure label={t.summary.commits(p.commits.length)}>
                        <ul className="space-y-0.5">
                          {p.commits.map((c) => (
                            <li key={(c.repo ?? "") + c.hash}>
                              <code className="font-mono text-[11px] text-faint">
                                {c.repo ? `${c.repo} ` : ""}
                                {c.hash}
                              </code>{" "}
                              {c.subject} <span className="font-mono text-[11px] text-faint">+{c.insertions} −{c.deletions}</span>
                              {c.mine === false && c.author && <span className="text-[11px] text-faint"> · {c.author}</span>}
                            </li>
                          ))}
                        </ul>
                      </Disclosure>
                    )}
                    <Disclosure label={t.summary.sessions(p.sessions.length)}>
                      <ul className="space-y-0.5">
                        {p.sessions.map((x) => (
                          <li key={x.session}>
                            <span className="font-mono text-[11px] text-faint">
                              {hhmm(x.first_at)}–{hhmm(x.last_at)}
                            </span>{" "}
                            {x.title ?? t.summary.untitled}
                            {x.branch && <span className="font-mono text-[11px] text-faint"> {x.branch}</span>}
                          </li>
                        ))}
                      </ul>
                      <p className="mt-1 font-mono text-[10.5px] text-faint">{homeRelative(p.root)}</p>
                    </Disclosure>
                  </div>
                </article>
              );
            })}
          </div>
        </Section>
      )}

      {s && (s.analysis.observations.length > 0 || s.analysis.suggestions.length > 0) && (
        <Section
          title={t.summary.notesSuggestions}
          aside={<span className="text-[11px] text-faint">{t.summary.toolsAside(m.tool_calls, m.tool_errors, m.denials)}</span>}
        >
          <div className="max-w-[80ch] pt-1">
            <Bullets title={t.summary.notes} items={s.analysis.observations} />
            <Bullets title={t.summary.suggestions} items={s.analysis.suggestions} />
          </div>
        </Section>
      )}

      {!empty && (
        <p className="mt-12 text-[11.5px] text-faint">
          {report.model && `${t.summary.writtenDay(writtenBy(report.provider, report.model))} `}
          {t.summary.workTimeNote}
        </p>
      )}
    </div>
  );
}
