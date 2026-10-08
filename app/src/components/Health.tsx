// Where one project kept getting held up in the last 30 days. Minutes are
// counted from recorded turns, each turn once; the kind of each blocker is
// what the day's summary called it. Evidence opens the day it came from.
import { useEffect, useRef, useState } from "react";
import { api, type Evidence, type KindStat, type ProjectHealth } from "../api";
import { duration, longDate } from "../format";
import { startJob, useJob } from "../jobs";
import { Button, Disclosure, Empty, Section, StatLine, useElapsed } from "./ui";
import { useT } from "../i18n";

const WINDOW_DAYS = 30;

/** The last 30 local dates, oldest first, as YYYY-MM-DD. */
function windowDates(): string[] {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  return Array.from({ length: WINDOW_DAYS }, (_, i) => {
    const d = new Date(now.getFullYear(), now.getMonth(), now.getDate() - (WINDOW_DAYS - 1 - i));
    return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
  });
}

/** One day in a kind's row. Hover shows the date and what held the work up; a day with blockers opens its summary. */
function DayCell({ date, titles, edge, onOpenDay }: { date: string; titles: string[] | undefined; edge: "start" | "middle" | "end"; onOpenDay: (date: string) => void }) {
  const t = useT();
  // Cells near the ends anchor their tip inward so it never leaves the window.
  const place = edge === "start" ? "left-0" : edge === "end" ? "right-0" : "left-1/2 -translate-x-1/2";
  const tip = (
    <span className={`pointer-events-none absolute bottom-full z-10 mb-1.5 hidden w-max max-w-[18rem] rounded-md border border-line-soft bg-elevated px-2 py-1 text-left text-[12px] group-hover:block ${place}`}>
      <span className="block text-fg">{longDate(date)}</span>
      {titles ? titles.map((title, i) => <span key={i} className="block text-muted">{title}</span>) : <span className="block text-faint">{t.projects.noBlockers}</span>}
    </span>
  );
  if (!titles) {
    return (
      <span className="group relative h-2.5 rounded-[2px] bg-fg/[0.06]">
        {tip}
      </span>
    );
  }
  return (
    <button
      type="button"
      onClick={() => onOpenDay(date)}
      className={`group relative h-2.5 rounded-[2px] motion-safe:transition-colors motion-safe:duration-fast hover:bg-fg ${titles.length > 1 ? "bg-fg/90" : "bg-fg/55"}`}
    >
      {tip}
    </button>
  );
}

/** One row per blocker kind: a dot on each day it held the work up, so repeats and clusters show at a glance. */
function KindDays({ kinds, onOpenDay }: { kinds: KindStat[]; onOpenDay: (date: string) => void }) {
  const t = useT();
  const dates = windowDates();
  return (
    <div className="mt-6">
      <div className="mb-2 text-[11.5px] text-faint">
        {t.projects.kindDays(dates[0].slice(5), dates[dates.length - 1].slice(5))}
      </div>
      <ul className="space-y-3">
        {kinds.map((k) => {
          const byDate = new Map<string, string[]>();
          for (const e of k.evidence) byDate.set(e.date, [...(byDate.get(e.date) ?? []), e.label]);
          const latest = [...k.evidence].sort((a, b) => b.date.localeCompare(a.date))[0];
          return (
            <li key={k.kind} className="grid grid-cols-[8.5rem_3rem_1fr] items-center gap-x-3 gap-y-1 text-[12.5px]">
              <span className="text-muted">{k.label}</span>
              <span className="text-faint">{t.projects.blockerCount(k.blockers)}</span>
              <span className="grid grid-cols-[repeat(30,minmax(0,1fr))] items-center gap-[3px]">
                {dates.map((d, i) => (
                  <DayCell key={d} date={d} titles={byDate.get(d)} edge={i < 4 ? "start" : i > dates.length - 5 ? "end" : "middle"} onOpenDay={onOpenDay} />
                ))}
              </span>
              {latest && <span className="col-start-3 truncate text-[11.5px] text-faint">{t.projects.latest(latest.label)}</span>}
            </li>
          );
        })}
      </ul>
    </div>
  );
}

/** Blockers summarized before kinds existed get one from a single Claude Code call. */
function ClassifyBlockers({ count, onDone }: { count: number; onDone: () => void }) {
  const t = useT();
  const [note, setNote] = useState<string | null>(null);
  const job = useJob("classify-blockers");
  const secs = useElapsed(job?.startedAt ?? null);
  // Checked by mount, not by effect: the job leaves the store before its result arrives.
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => void (mounted.current = false);
  }, []);
  useEffect(() => {
    if (!job) return;
    (job.promise as Promise<number>)
      .then((n) => mounted.current && setNote(t.projects.classifyDone(n)), (e) => mounted.current && setNote(String(e)))
      .finally(() => mounted.current && onDone());
  }, [job]);
  if (job) return <p className="mt-3 text-[12px] text-muted">{t.projects.classifying(secs)}</p>;
  return (
    <div className="mt-3 flex items-center gap-3 text-[12px] text-faint">
      {count > 0 && (
        <>
          <span>{t.projects.unclassified(count)}</span>
          <Button onClick={() => startJob("classify-blockers", (tt) => tt.projects.classify, () => api.classifyBlockers())} title={t.projects.classifyTitle}>
            {t.projects.classifyButton}
          </Button>
        </>
      )}
      {note && <span>{note}</span>}
    </div>
  );
}

function EvidenceList({ items, onOpenDay }: { items: Evidence[]; onOpenDay: (date: string) => void }) {
  return (
    <ul className="mt-1 space-y-1">
      {items.map((e, i) => (
        <li key={i}>
          <button type="button" onClick={() => onOpenDay(e.date)} className="text-left text-[12.5px] text-muted motion-safe:transition-colors motion-safe:duration-fast hover:text-fg">
            <span className="font-mono text-faint">{e.date.slice(5)}</span> {e.label}
          </button>
        </li>
      ))}
    </ul>
  );
}

export function HealthSection({ health, onOpenDay, onChanged }: { health: ProjectHealth; onOpenDay: (date: string) => void; onChanged: () => void }) {
  const t = useT();
  const unclassified = health.kinds.find((k) => k.kind === "unclassified")?.blockers ?? 0;
  return (
    <Section title={t.projects.health} aside={<span className="text-[11.5px] text-faint">{t.projects.healthAside}</span>}>
      <StatLine
        items={[
          t.projects.work30(duration(health.active_minutes)),
          t.projects.blockedTime(duration(health.blocked_minutes)),
          health.interrupted > 0 && t.projects.interrupted(health.interrupted),
          health.denials > 0 && t.projects.denials(health.denials),
        ]}
      />
      {health.checks.length === 0 ? (
        <Empty>{t.projects.noRepeats}</Empty>
      ) : (
        <ul className="mt-3 divide-y divide-line-soft">
          {health.checks.map((c, i) => (
            <li key={i} className="py-2">
              <div className="text-[13px] text-fg">{c.title}</div>
              <Disclosure label={t.projects.evidenceCount(c.evidence.length)}>
                <EvidenceList items={c.evidence} onOpenDay={onOpenDay} />
              </Disclosure>
            </li>
          ))}
        </ul>
      )}
      {health.kinds.length > 0 && <KindDays kinds={health.kinds} onOpenDay={onOpenDay} />}
      <ClassifyBlockers count={unclassified} onDone={onChanged} />
      {health.repeated_errors.length > 0 && (
        <Disclosure label={t.projects.failedCommands(health.repeated_errors.length)}>
          <ul className="space-y-1">
            {health.repeated_errors.map((e) => (
              <li key={e.command}>
                <span className="font-mono text-fg/90">{e.command}</span>
                <span className="text-faint">
                  {" "}
                  {t.projects.daysTimes(e.days, e.failures)}
                </span>
              </li>
            ))}
          </ul>
        </Disclosure>
      )}
      {(health.session_attributed_days > 0 || health.issues_tracked_since) && (
        <p className="mt-4 text-[11.5px] text-faint">
          {health.session_attributed_days > 0 && t.projects.sessionAttributed(health.session_attributed_days)}
          {health.issues_tracked_since && t.projects.issuesSince(health.issues_tracked_since)}
        </p>
      )}
    </Section>
  );
}
