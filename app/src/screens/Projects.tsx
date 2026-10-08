import { useEffect, useRef, useState } from "react";
import { useKept } from "../kept";
import { api, type ProjectDay, type ProjectHealth, type ProjectOverview, type Suggestion } from "../api";
import { duration, homeRelative, longDate } from "../format";
import { Bullets, Button, Disclosure, Empty, Headline, StatLine, useElapsed } from "../components/ui";
import { HealthSection } from "../components/Health";
import { SuggestionsSection } from "../components/Suggestions";
import { Icon } from "../components/Icon";
import { startJob, useJob } from "../jobs";
import { useLang, useT } from "../i18n";

function Backfill({ onDone }: { onDone: () => void }) {
  const t = useT();
  const [note, setNote] = useState<string | null>(null);
  const job = useJob("backfill");
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
      .then((n) => mounted.current && setNote(n > 0 ? t.projects.backfillAdded(n) : t.projects.backfillNone), (e) => mounted.current && setNote(String(e)))
      .finally(() => mounted.current && onDone());
  }, [job]);
  if (job) return <span className="text-[12px] text-muted">{t.projects.backfillRunning(secs)}</span>;
  return (
    <span className="flex items-center gap-3">
      {note && <span className="text-[12px] text-faint">{note}</span>}
      <Button onClick={() => startJob("backfill", (tt) => tt.projects.backfill, () => api.backfill(30))} title={t.projects.backfillTitle}>
        {t.projects.backfill}
      </Button>
    </span>
  );
}

function ProjectDetail({
  project,
  health,
  suggestions,
  onBack,
  onOpenDay,
  onChanged,
}: {
  project: ProjectOverview;
  health: ProjectHealth | undefined;
  suggestions: Suggestion[];
  onBack: () => void;
  onOpenDay: (date: string) => void;
  onChanged: () => void;
}) {
  const t = useT();
  const [days, setDays] = useState<ProjectDay[] | null>(null);
  useEffect(() => {
    api.projectDays(project.root).then(setDays);
  }, [project.root]);
  const max = Math.max(1, ...(days ?? []).map((d) => d.minutes));

  return (
    <div>
      <button type="button" onClick={onBack} className="mt-2 inline-flex h-7 items-center gap-1 rounded-md px-1.5 text-[12.5px] text-muted hover:bg-fg/5 hover:text-fg">
        <Icon name="chevronLeft" size={13} /> {t.nav.projects}
      </button>
      <Headline eyebrow={homeRelative(project.root)}>{project.name}</Headline>
      <StatLine
        items={[
          duration(project.minutes),
          t.summary.daysRecorded(project.days),
          project.last_date && t.summary.commits(project.commits),
          project.last_date && `${project.first_date} ~ ${project.last_date}`,
        ]}
      />

      {project.root !== "" && <SuggestionsSection scope={project.root} items={suggestions} title={t.projects.suggestionsTitle} onChanged={onChanged} onOpenDay={onOpenDay} />}

      {health && <HealthSection health={health} onOpenDay={onOpenDay} onChanged={onChanged} />}

      {!days && <Empty>{t.common.loading}</Empty>}
      {days?.length === 0 && project.last_date === "" && (
        <Empty>{project.root === "" ? t.projects.issueNameMismatch : t.projects.noSessions}</Empty>
      )}
      <ol className="mt-8">
        {days?.map((d) => (
          <li key={d.date} className="grid grid-cols-[7.5rem_1fr] gap-5 border-t border-line-soft py-4 first:border-t-0">
            <div>
              <div className="text-[12.5px]">{longDate(d.date)}</div>
              <div className="mt-1 flex items-center gap-2">
                <span className="h-1 flex-1 rounded-full bg-fg/10">
                  <span className="block h-1 rounded-full bg-fg/40" style={{ width: `${(d.minutes / max) * 100}%` }} />
                </span>
              </div>
              <div className="mt-1 text-[11.5px] text-faint">
                {duration(d.minutes)} · {t.summary.commits(d.commits.length)}
              </div>
            </div>
            <div className="min-w-0 max-w-[80ch]">
              {d.summary ? <p className="text-fg/90">{d.summary}</p> : <p className="text-muted">{t.projects.noSummaryDay}</p>}
              {d.next.length > 0 && (
                <div className="mt-2">
                  <Bullets title={t.summary.next} items={d.next} />
                </div>
              )}
              {(d.done.length > 0 || d.commits.length > 0) && (
                <div className="flex gap-5">
                  {d.done.length > 0 && (
                    <Disclosure label={t.projects.done(d.done.length)}>
                      <Bullets title="" items={d.done} />
                    </Disclosure>
                  )}
                  {d.commits.length > 0 && (
                    <Disclosure label={t.summary.commits(d.commits.length)}>
                      <Bullets title="" items={d.commits} />
                    </Disclosure>
                  )}
                </div>
              )}
            </div>
          </li>
        ))}
      </ol>
    </div>
  );
}

/** Rows for the repos the health checks know that no session folder started in:
 *  a repo only ever edited from a session started elsewhere, and the
 *  "Unknown project" bucket. Their dates stay blank and their commits 0, and rows
 *  skip those parts rather than print them. */
function withHealthOnlyRows(list: ProjectOverview[], health: ProjectHealth[]): ProjectOverview[] {
  const known = new Set(list.map((p) => p.root));
  const extra = health
    .filter((h) => !known.has(h.root))
    .sort((a, b) => b.active_minutes - a.active_minutes)
    .map((h) => ({
      name: h.name,
      root: h.root,
      days: h.days,
      minutes: h.active_minutes,
      commits: 0,
      first_date: "",
      last_date: "",
      latest_summary: null,
    }));
  return [...list, ...extra];
}

export function Projects({ onOpenDay }: { onOpenDay: (date: string) => void }) {
  const t = useT();
  const lang = useLang();
  const [list, setList] = useState<ProjectOverview[] | null>(null);
  const [health, setHealth] = useState<Map<string, ProjectHealth>>(new Map());
  const [suggestions, setSuggestions] = useState<Suggestion[]>([]);
  const [open, setOpen] = useKept<ProjectOverview | null>("projects.open", null);
  const load = () => {
    api.projects().then(setList);
    api.projectsHealth().then((h) => setHealth(new Map(h.map((p) => [p.root, p]))));
    api.suggestions().then(setSuggestions, () => setSuggestions([]));
  };
  // Rust writes check titles, kind names and target labels in the current
  // language, so the data is read again when the language changes.
  useEffect(() => {
    load();
  }, [lang]);

  if (open)
    return <ProjectDetail project={open} health={health.get(open.root)} suggestions={suggestions} onBack={() => setOpen(null)} onOpenDay={onOpenDay} onChanged={load} />;

  const rows = list && withHealthOnlyRows(list, [...health.values()]);

  return (
    <div>
      <Headline eyebrow={t.nav.projects} action={<Backfill onDone={load} />}>
        {rows && rows.length > 0 ? t.projects.count(rows.length) : t.projects.none}
      </Headline>
      <StatLine items={[t.projects.statFrom, t.projects.statBar]} />

      <SuggestionsSection scope="common" items={suggestions} title={t.projects.commonSuggestions} onChanged={load} onOpenDay={onOpenDay} />

      {!list && <Empty>{t.common.loading}</Empty>}
      {rows?.length === 0 && <Empty>{t.projects.emptyHint}</Empty>}
      <ul className="mt-6 divide-y divide-line-soft">
        {rows?.map((p) => (
          <li key={p.root}>
            <button
              type="button"
              onClick={() => setOpen(p)}
              className="grid w-full grid-cols-[1fr_auto] items-baseline gap-x-6 gap-y-1 rounded-md px-2 py-3 text-left motion-safe:transition-colors motion-safe:duration-fast hover:bg-fg/5"
              title={p.root || p.name}
            >
              <span className="flex items-baseline gap-3">
                <span className="text-[14px] font-medium">{p.name}</span>
                <span className="text-[11.5px] text-faint">
                  {duration(p.minutes)} · {t.projects.days(p.days)}
                  {p.last_date && t.projects.commits(p.commits)}
                  {(health.get(p.root)?.checks.length ?? 0) > 0 && t.projects.checks(health.get(p.root)!.checks.length)}
                  {(() => {
                    const n = suggestions.filter((s) => s.scope === p.root && s.status === "new").length;
                    return n > 0 && t.projects.suggestions(n);
                  })()}
                </span>
              </span>
              {p.last_date && <span className="text-[11.5px] text-faint">{t.projects.last(longDate(p.last_date))}</span>}
              <span className="col-span-2 line-clamp-2 max-w-[80ch] text-[12.5px] text-muted">
                {p.latest_summary ?? t.projects.noSummary}
              </span>
              <span className="col-span-2 mt-1 h-1 rounded-full bg-fg/20" style={{ width: `${Math.max(2, (p.minutes / Math.max(1, ...rows.map((x) => x.minutes))) * 100)}%` }} />
            </button>
          </li>
        ))}
      </ul>
    </div>
  );
}
