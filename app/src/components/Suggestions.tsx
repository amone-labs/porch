// Suggestions Claude Code wrote from where a project kept getting held up.
// porch never edits the person's config: each card has the text to paste, a
// copy button and "Applied" so the before/after count can start from that day.
import { useEffect, useRef, useState } from "react";
import { api, type Effect, type Suggestion } from "../api";
import { longDate } from "../format";
import { startJob, useJob } from "../jobs";
import { track } from "../analytics";
import { Button, Disclosure, Empty, Section, useElapsed } from "./ui";
import { useT } from "../i18n";
import type { Dict } from "../i18n/ko";

function effectLine(t: Dict, s: Suggestion, e: Effect): string {
  const unit = s.target.startsWith("command:") ? t.projects.unitTimes : t.projects.unitItems;
  switch (e.state) {
    case "early":
      return t.projects.effectEarly(e.days);
    case "no_records":
      return t.projects.effectNoRecords(e.days);
    case "uncomparable":
      return t.projects.effectUncomparable(e.days);
    default:
      return t.projects.effectMeasured(e.days, s.target_label, e.after.items, unit, e.before.items);
  }
}

function Card({ s, onChanged, onOpenDay }: { s: Suggestion; onChanged: () => void; onOpenDay: (date: string) => void }) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const set = (status: Suggestion["status"]) =>
    api.setSuggestionStatus(s.id, status).then(() => {
      track("suggestion_status", { status });
      onChanged();
    }, (e) => setError(String(e)));
  // Mono only for Latin and digits; model text with Hangul (가-힣) stays in the sans stack.
  const latin = !/[\uAC00-\uD7A3]/.test(s.text);
  if (s.status === "applied") {
    return (
      <li className="py-3">
        <div className="flex items-baseline justify-between gap-4">
          <span className="text-[13px] text-fg">{s.title}</span>
          <button type="button" onClick={() => set("new")} className="h-7 shrink-0 rounded-md px-2 text-[12px] text-faint hover:bg-fg/5 hover:text-fg">
            {t.projects.undoApply}
          </button>
        </div>
        {s.effect && <p className="mt-1 text-[12.5px] text-muted">{effectLine(t, s, s.effect)}</p>}
        <Disclosure label={t.projects.appliedText}>
          <pre className={`whitespace-pre-wrap text-[12.5px] text-muted ${latin ? "font-mono" : "font-sans"}`}>{s.text}</pre>
        </Disclosure>
        {error && <p className="mt-1 text-[12px] text-muted">{error}</p>}
      </li>
    );
  }
  return (
    <li className="py-4">
      <div className="text-[13px] text-fg">{s.title}</div>
      <p className="mt-1 text-[12.5px] text-muted">
        {s.why}{" "}
        {s.evidence.map((d) => (
          <button key={d} type="button" onClick={() => onOpenDay(d)} className="ml-1 text-faint underline-offset-2 hover:text-fg hover:underline">
            {longDate(d)}
          </button>
        ))}
      </p>
      <div className="mt-2 flex items-start gap-3 rounded-md border border-line-soft bg-fg/[0.03] px-3 py-2">
        <pre className={`min-w-0 flex-1 whitespace-pre-wrap text-[12.5px] text-fg/90 ${latin ? "font-mono" : "font-sans"}`}>{s.text}</pre>
        <Button
          onClick={() => {
            if (!navigator.clipboard) {
              setError(t.projects.copyFailed);
              return;
            }
            navigator.clipboard.writeText(s.text).then(
              () => {
                setCopied(true);
                setTimeout(() => setCopied(false), 1200);
                track("suggestion_copied", {});
              },
              () => setError(t.projects.copyFailed),
            );
          }}
        >
          {copied ? t.projects.copied : t.projects.copy}
        </Button>
      </div>
      <div className="mt-2 flex items-center justify-between text-[12px] text-faint">
        <span>
          {t.projects.baseline(t.projects.action[s.action], s.target_label, s.baseline.items, (s.target.startsWith("command:") ? t.projects.unitTimes : t.projects.unitItems)(s.baseline.items), longDate(s.created))}
        </span>
        <span className="flex gap-2">
          {s.status === "dismissed" ? (
            <Button onClick={() => set("new")}>{t.projects.restore}</Button>
          ) : (
            <>
              <Button onClick={() => set("applied")}>{t.projects.apply}</Button>
              <Button onClick={() => set("dismissed")}>{t.projects.dismiss}</Button>
            </>
          )}
        </span>
      </div>
      {error && <p className="mt-1 text-[12px] text-muted">{error}</p>}
    </li>
  );
}

/** What to say after a make: nothing found, all dropped, or both counts. */
function madeNote(t: Dict, r: { added: number; dropped: number }): string {
  if (r.added === 0 && r.dropped > 0) return t.projects.madeDroppedOnly(r.dropped);
  if (r.added > 0 && r.dropped > 0) return t.projects.madeBoth(r.added, r.dropped);
  return r.added > 0 ? t.projects.madeAdded(r.added) : t.projects.madeNone;
}

function MakeButton({ hasAny, onDone }: { hasAny: boolean; onDone: () => void }) {
  const t = useT();
  const [note, setNote] = useState<string | null>(null);
  const job = useJob("suggestions");
  const secs = useElapsed(job?.startedAt ?? null);
  // Checked by mount, not by effect: the job leaves the store before its result arrives.
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => void (mounted.current = false);
  }, []);
  useEffect(() => {
    if (!job) return;
    (job.promise as Promise<{ added: number; dropped: number }>)
      .then((r) => mounted.current && setNote(madeNote(t, r)), (e) => mounted.current && setNote(String(e)))
      .finally(() => mounted.current && onDone());
  }, [job]);
  if (job) return <span className="text-[12px] text-muted">{t.projects.making(secs)}</span>;
  return (
    <span className="flex items-center gap-3">
      {note && <span className="text-[12px] text-faint">{note}</span>}
      <Button onClick={() => startJob("suggestions", (tt) => tt.projects.make, () => api.makeSuggestions())} title={t.projects.makeTitle}>
        {hasAny ? t.summary.rewrite : t.projects.make}
      </Button>
    </span>
  );
}

export function SuggestionsSection({
  scope,
  items,
  title,
  onChanged,
  onOpenDay,
}: {
  scope: string;
  items: Suggestion[];
  title: string;
  onChanged: () => void;
  onOpenDay: (date: string) => void;
}) {
  const t = useT();
  const mine = items.filter((s) => s.scope === scope);
  const open = mine.filter((s) => s.status === "new");
  const applied = mine.filter((s) => s.status === "applied");
  const dismissed = mine.filter((s) => s.status === "dismissed");
  const made = open[0]?.created;
  return (
    <Section title={title} aside={<MakeButton hasAny={mine.length > 0} onDone={onChanged} />}>
      {mine.length === 0 && <Empty>{t.projects.empty}</Empty>}
      {made && <p className="text-[11.5px] text-faint">{t.projects.madeOn(longDate(made))}</p>}
      <ul className="divide-y divide-line-soft">
        {[...open, ...applied].map((s) => (
          <Card key={s.id} s={s} onChanged={onChanged} onOpenDay={onOpenDay} />
        ))}
      </ul>
      {applied.length > 0 && <p className="mt-2 text-[11.5px] text-faint">{t.projects.appliedNote}</p>}
      {dismissed.length > 0 && (
        <Disclosure label={t.projects.dismissedCount(dismissed.length)}>
          <ul className="divide-y divide-line-soft">
            {dismissed.map((s) => (
              <Card key={s.id} s={s} onChanged={onChanged} onOpenDay={onOpenDay} />
            ))}
          </ul>
        </Disclosure>
      )}
    </Section>
  );
}
