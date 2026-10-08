// The model-written part of Insights (ADR 0012): scores against the rubric,
// the week's direction changes and request rewrites. Every quote here matched
// a request on record; the lines saying what each score means come from the rubric in Rust.
import { useState, type ReactNode } from "react";
import type { EvalItem, Evaluation, InsightView, Rewrite } from "../api";
import { failureReason } from "../analytics";
import { hm, monthDay, writtenBy } from "../format";
import { useT } from "../i18n";
import type { Dict } from "../i18n/ko";

const ITEMS: EvalItem[] = ["verify", "delegate", "clarity", "rationale"];

/** A request as it was typed: a prompt mark, then the words (Hangul stays in the sans stack). */
export function Typed({ text }: { text: string }) {
  return (
    <p className="flex gap-2 rounded-md bg-fg/5 px-3 py-2 text-[12.5px] leading-relaxed">
      <span aria-hidden className="select-none font-mono text-faint">❯</span>
      <span className="min-w-0 break-words">{text}</span>
    </p>
  );
}

export function SmallButton({ onClick, disabled, children }: { onClick: () => void; disabled?: boolean; children: ReactNode }) {
  return (
    <button type="button" disabled={disabled} onClick={onClick}
      className="h-7 rounded-md px-2.5 text-[12px] text-muted hover:bg-fg/5 hover:text-fg disabled:opacity-40 motion-safe:transition-colors motion-safe:duration-fast">
      {children}
    </button>
  );
}

function Scores({ e, last, disputed, finished, onDispute, onGoal }: {
  e: Evaluation; last: InsightView["last_scores"]; disputed: string[]; finished: boolean;
  onDispute: (item: EvalItem) => void; onGoal: (key: string) => void;
}) {
  const t = useT();
  const [focus, setFocus] = useState<EvalItem | null>(null);
  const hover = (id: EvalItem) => ({
    onMouseEnter: () => setFocus(id), onMouseLeave: () => setFocus(null),
    onFocus: () => setFocus(id), onBlur: () => setFocus(null),
  });
  const lit = (id: EvalItem) => (focus === id ? " bg-fg/5" : "");
  return (
    <section aria-labelledby="ins-eval">
      <div className="mb-3 flex flex-wrap items-baseline justify-between gap-2">
        <h2 id="ins-eval" className="text-[15px] font-semibold">{t.insight.evalTitle}</h2>
        <span className="text-[12px] text-faint">{t.insight.evalScale}</span>
      </div>
      <div className="grid grid-cols-2 gap-px overflow-hidden rounded-lg border border-line bg-line sm:grid-cols-4">
        {ITEMS.map((id) => {
          const s = e.scores.find((x) => x.item === id);
          return (
            <button key={id} type="button" {...hover(id)} onClick={() => document.getElementById(`ins-row-${id}`)?.scrollIntoView({ block: "nearest" })}
              className={"bg-canvas px-4 py-3 text-left motion-safe:transition-colors motion-safe:duration-fast" + lit(id)}>
              <div className="text-[12px] text-muted">{t.insight.items[id].name}</div>
              <div className="mt-1 text-[22px] font-semibold tabular-nums">
                {s ? <>{s.score}<span className="text-[13px] font-normal text-faint">/5</span></> : <span className="text-[13px] font-normal text-faint">{t.insight.noEvidence}</span>}
              </div>
              {s && <div className="mt-0.5 text-[11.5px] text-faint">{t.insight.grade(s.score)} · {t.insight.lastWeekScore(last[id])}</div>}
            </button>
          );
        })}
      </div>
      <div className="mt-4 overflow-x-auto">
        <table className="w-full text-left text-[13px]">
          <thead className="text-[11.5px] text-faint">
            <tr><th className="py-1.5 pr-4 font-normal">{t.insight.colItem}</th><th className="py-1.5 pr-4 font-normal">{t.insight.colScore}</th><th className="py-1.5 font-normal">{t.insight.colEvidence}</th></tr>
          </thead>
          <tbody>
            {ITEMS.map((id) => {
              const s = e.scores.find((x) => x.item === id);
              return (
                <tr key={id} id={`ins-row-${id}`} {...hover(id)} className={"border-t border-line-soft align-top motion-safe:transition-colors motion-safe:duration-fast" + lit(id)}>
                  <td className="w-[26%] py-3 pr-4">
                    <div className="font-medium">{t.insight.items[id].name}</div>
                    <div className="mt-0.5 text-[12px] text-faint">{t.insight.items[id].question}</div>
                  </td>
                  <td className="w-[12%] py-3 pr-4 tabular-nums">
                    {s ? <><div>{t.insight.scoreN(s.score)}</div><div className="text-[12px] text-faint">{t.insight.grade(s.score)}</div></>
                      : <span className="text-[12px] text-faint">{t.insight.noEvidence}</span>}
                  </td>
                  <td className="py-3">
                    {s && (
                      <div className="grid gap-2">
                        {s.quote ? <Typed text={s.quote} /> : <p className="text-[12px] text-faint">{t.insight.noTurn}</p>}
                        <p>{s.why}</p>
                        <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-3 gap-y-0.5 text-[12px]">
                          <dt className="text-faint">{t.insight.criterionAt(s.score)}</dt><dd className="text-muted">{s.criterion}</dd>
                          {s.next_criterion && <><dt className="text-faint">{t.insight.criterionAt(s.score + 1)}</dt><dd className="text-muted">{s.next_criterion}</dd></>}
                        </dl>
                        <div className="flex flex-wrap items-center gap-1.5">
                          {finished && s.score <= 3 && <SmallButton onClick={() => onGoal(`score:${id}`)}>{t.insight.toGoal}</SmallButton>}
                          {disputed.includes(id)
                            ? <span className="text-[12px] text-faint">{t.insight.disputed}</span>
                            : <SmallButton onClick={() => onDispute(id)}>{t.insight.dispute}</SmallButton>}
                        </div>
                      </div>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
    </section>
  );
}

function Pivots({ e, finished, onGoal }: { e: Evaluation; finished: boolean; onGoal: (key: string) => void }) {
  const t = useT();
  const c = e.pivot_counts;
  return (
    <section aria-labelledby="ins-pivots">
      <h2 id="ins-pivots" className="mb-1 text-[15px] font-semibold">{t.insight.pivotsTitle}</h2>
      <p className="mb-3 text-[12px] text-faint tabular-nums">{t.insight.pivotCounts(c.total, c.with_reason, c.redo)}</p>
      {e.pivots.length === 0 ? <p className="text-muted">{t.insight.noPivots}</p> : (
        <ol className="grid gap-4">
          {e.pivots.map((p) => {
            const note = p.note ? ` · ${p.type === "redo" && !p.reason_given ? `${t.insight.readAs} ${p.note}` : p.note}` : "";
            return (
              <li key={p.turn} className="grid grid-cols-[64px_minmax(0,1fr)] gap-3">
                <span className="pt-0.5 text-[12px] text-faint tabular-nums">{monthDay(p.date)}</span>
                <div className="grid gap-1.5">
                  <div className="flex flex-wrap items-baseline gap-x-2">
                    {(p.before || p.after) && <span className="font-medium">{p.before} → {p.after}</span>}
                    <span className="text-[12px] text-faint">{t.insight.pivotType[p.type]}</span>
                  </div>
                  <Typed text={p.quote} />
                  <p className="text-[12px] text-muted">{p.reason_given ? t.insight.reasonGiven : t.insight.noReason}{note}</p>
                  {finished && p.type === "redo" && <div><SmallButton onClick={() => onGoal("pivots:redo")}>{t.insight.toGoal}</SmallButton></div>}
                </div>
              </li>
            );
          })}
        </ol>
      )}
    </section>
  );
}

function Rewrites({ e }: { e: Evaluation }) {
  const t = useT();
  const [copied, setCopied] = useState<string | null>(null);
  const copy = (w: Rewrite) =>
    navigator.clipboard.writeText(w.rewritten).then(() => {
      setCopied(w.turn);
      setTimeout(() => setCopied(null), 1500);
    }, () => {});
  if (e.rewrites.length === 0) return null;
  return (
    <section aria-labelledby="ins-rewrites">
      <h2 id="ins-rewrites" className="mb-3 text-[15px] font-semibold">{t.insight.rewritesTitle}</h2>
      <div className="grid gap-4 lg:grid-cols-2">
        {e.rewrites.map((w) => {
          const at = w.added ? w.rewritten.indexOf(w.added) : -1;
          return (
            <article key={w.turn} className="overflow-hidden rounded-lg border border-line">
              <header className="flex items-center gap-2 border-b border-line-soft px-3 py-2 text-[11.5px] text-faint">
                <span aria-hidden className="font-mono tracking-[2px]">●●●</span><span>{monthDay(w.date)}</span>
              </header>
              <div className="grid gap-3 p-3">
                <div className="text-muted"><Typed text={w.quote} /></div>
                <div>
                  <div className="mb-1 flex items-center justify-between gap-2">
                    <span className="text-[12px] text-faint">＋ {t.insight.rewriteLabel}</span>
                    <SmallButton onClick={() => copy(w)}>{copied === w.turn ? t.insight.copied : t.insight.copyRewrite}</SmallButton>
                  </div>
                  <p className="rounded-md bg-fg/5 px-3 py-2 text-[13px] leading-relaxed">
                    {at < 0 ? w.rewritten : <>{w.rewritten.slice(0, at)}<mark className="rounded-sm bg-fg/15 px-0.5 text-fg">{w.added}</mark>{w.rewritten.slice(at + w.added.length)}</>}
                  </p>
                </div>
                {w.missing.length > 0 && (
                  <ul className="flex flex-wrap gap-1.5 text-[12px]">
                    {w.missing.map((m) => <li key={m} className="rounded-full border border-line px-2 py-0.5 text-muted">＋ {m}</li>)}
                  </ul>
                )}
                {w.note && <p className="text-[12px] text-muted">{w.note}</p>}
              </div>
            </article>
          );
        })}
      </div>
      <p className="mt-3 text-[12px] text-faint">{t.insight.rewriteNote}</p>
    </section>
  );
}

/** Everything under the boundary line: the state, or the evaluation itself. */
export function EvalArea({ view, busy, onMake, onDispute, onGoal, onChangeAgent }: {
  view: InsightView; busy: boolean; onMake: () => void;
  onDispute: (item: EvalItem) => void; onGoal: (key: string) => void; onChangeAgent?: () => void;
}) {
  const t = useT();
  const e = view.evaluation;
  const state = !view.finished ? t.insight.evalInProgress
    : !view.has_week_summary ? t.insight.evalNeedWeekSummary
    : view.error ? t.insight.evalFailed
    : e ? null
    : view.skipped === "few_requests" ? t.insight.evalFew
    : view.skipped === "no_day_summaries" ? t.insight.evalNoDays
    : view.skipped === "off" ? t.insight.evalWasOff
    : t.insight.evalNotYet;
  const canMake = view.finished && view.has_week_summary;
  const make = <SmallButton disabled={busy} onClick={onMake}>{busy ? t.insight.evalMaking : e || view.error ? t.insight.evalRemake : t.insight.evalMake}</SmallButton>;
  return (
    <div className="grid gap-12">
      <div className="border-t border-line pt-4 text-[12px] text-faint">
        <p>{t.insight.evalBoundary}</p>
        {e && view.model && (
          <p className="mt-0.5">
            {t.insight.evalWrittenBy(writtenBy(view.provider, view.model))}
            {view.days_without_summary.length > 0 ? ` · ${t.insight.daysWithoutSummary(view.days_without_summary.length)}` : ""}
          </p>
        )}
      </div>
      {state && (
        <div className="-mt-6 flex flex-wrap items-center gap-3">
          <p className="text-muted">{state}{view.error ? ` ${view.error}` : ""}</p>
          {canMake && make}
          {view.error && failureReason(view.error) === "limit" && onChangeAgent && <SmallButton onClick={onChangeAgent}>{t.insight.changeAgent}</SmallButton>}
        </div>
      )}
      {e && (
        <>
          <Scores e={e} last={view.last_scores} disputed={view.disputed} finished={view.finished} onDispute={onDispute} onGoal={onGoal} />
          <Pivots e={e} finished={view.finished} onGoal={onGoal} />
          <Rewrites e={e} />
          {!state && <div className="-mt-6">{make}</div>}
        </>
      )}
    </div>
  );
}

/** A goal's name from its key, in the language on screen. */
export function goalLabel(t: Dict, key: string): string {
  const g = t.insight.goalLabel;
  if (key === "stop_failures") return g.stop_failures;
  if (key === "waiting_ms") return g.waiting_ms;
  if (key === "pivots:redo") return g.redo;
  if (key.startsWith("tool_errors:")) return g.tool_errors(key.slice("tool_errors:".length));
  if (key.startsWith("score:")) {
    const item = key.slice("score:".length) as EvalItem;
    return g.score(t.insight.items[item]?.name ?? item);
  }
  return key;
}

/** A goal's number the way its key counts it. */
export function goalValue(t: Dict, key: string, n: number): string {
  if (key === "waiting_ms") return hm(n);
  if (key.startsWith("score:")) return t.insight.scoreN(n);
  if (key === "stop_failures") return t.insight.times(n);
  if (key.startsWith("tool_errors:")) return t.insight.errorsN(n);
  return t.insight.requestsN(n);
}

/** The top line: last week's goal and how this week compares (never written as cause, ADR 0012). */
export function LastGoalLine({ view, onKeep }: { view: InsightView; onKeep: (key: string) => void }) {
  const t = useT();
  const l = view.last_goal;
  if (!l) return <p className="text-[12px] text-faint">{view.finished ? t.insight.noLastGoal : t.insight.noLastGoalRunning}</p>;
  const v = (n: number) => goalValue(t, l.goal.key, n);
  const r = l.result;
  const body = r.comparable && r.after !== null ? t.insight.goalCompared(v(r.before), v(r.after))
    : r.reason === "in_progress" && r.so_far !== null ? t.insight.goalSoFar(v(r.so_far))
    : t.insight.goalNotComparable(t.insight.goalReason[r.reason ?? "coverage"]);
  return (
    <div className="flex flex-wrap items-center gap-x-3 gap-y-1 rounded-lg border border-line px-4 py-2.5">
      <span className="text-[12px] text-faint">{t.insight.lastGoal}</span>
      <span className="font-medium">{goalLabel(t, l.goal.key)}</span>
      <span className="text-muted tabular-nums">{body}</span>
      {view.finished && view.goal?.key !== l.goal.key && <span className="ml-auto"><SmallButton onClick={() => onKeep(l.goal.key)}>{t.insight.keepGoal}</SmallButton></span>}
    </div>
  );
}

/** Next week's goal: one of the finished week's candidates, or the state that says why not yet. */
export function NextGoal({ view, onPick, onClear }: { view: InsightView; onPick: (key: string) => void; onClear: () => void }) {
  const t = useT();
  return (
    <section aria-labelledby="ins-goal">
      <h2 id="ins-goal" className="mb-1 text-[15px] font-semibold">{t.insight.nextGoalTitle}</h2>
      {!view.finished ? <p className="text-muted">{t.insight.goalAfterWeek}</p> : (
        <>
          <p className="mb-3 text-[12px] text-faint">{t.insight.nextGoalHint}</p>
          {view.candidates.length === 0 ? <p className="text-muted">{t.insight.noCandidates}</p> : (
            <div role="radiogroup" aria-labelledby="ins-goal" className="grid gap-2 sm:grid-cols-2">
              {view.candidates.map((c) => {
                const on = view.goal?.key === c.key;
                return (
                  <button key={c.key} type="button" role="radio" aria-checked={on} onClick={() => (on ? onClear() : onPick(c.key))}
                    className={"flex items-center gap-3 rounded-lg border px-3 py-2.5 text-left motion-safe:transition-colors motion-safe:duration-fast " + (on ? "border-fg/40 bg-fg/10" : "border-line hover:bg-fg/5")}>
                    <span className="rounded-full border border-line px-1.5 text-[11px] text-faint">{t.insight.goalKind[c.kind]}</span>
                    <span className="min-w-0 flex-1">{goalLabel(t, c.key)}</span>
                    <span className="text-[12px] text-muted tabular-nums">{t.insight.thisWeekValue(goalValue(t, c.key, c.value))}</span>
                  </button>
                );
              })}
            </div>
          )}
          {view.goal && <p className="mt-3 text-[12.5px]">{t.insight.goalChosen(goalLabel(t, view.goal.key))}</p>}
          <p className="mt-2 text-[12px] text-faint">{t.insight.goalDisclaimer}</p>
        </>
      )}
    </section>
  );
}
