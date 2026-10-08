// Where the time went and where it got stuck: the day timeline, the time
// blocks, the blockers and the diagrams the summary chose to draw. Minutes
// come from recorded turns; the model only named and grouped them.
import { useEffect, useId, useMemo, useState } from "react";
import type { Blocker, ProjectDigest, TimeBlock, Turn, Visual } from "../api";
import { duration, hhmm, money, tokenTotal, tokens } from "../format";
import { onPaper } from "../paper";
import { Section } from "./ui";
import { useT } from "../i18n";
import { CONTINUED_MARKER } from "../i18n/ko";

/** Turn ids the pointer is on, shared by the lists and the timeline. */
export function useFocus() {
  const [focus, setFocus] = useState<Set<string>>(new Set());
  const on = (ids: string[]) => ({
    onMouseEnter: () => setFocus(new Set(ids)),
    onMouseLeave: () => setFocus(new Set()),
    onFocus: () => setFocus(new Set(ids)),
    onBlur: () => setFocus(new Set()),
  });
  return { focus, on };
}

// ---------- timeline ----------

interface Row {
  name: string;
  turns: Turn[];
}

function rows(projects: ProjectDigest[]): Row[] {
  return projects
    .map((p) => ({ name: p.name, turns: p.sessions.flatMap((s) => s.turns ?? []) }))
    .filter((r) => r.turns.length > 0);
}

function hourOf(t: number) {
  const d = new Date(t);
  return d.getHours() + d.getMinutes() / 60;
}

/** The day as rows of turns on a clock: each block starts when the request did and runs to its last activity. */
export function Timeline({ projects, blockers, focus }: { projects: ProjectDigest[]; blockers: Blocker[]; focus: Set<string> }) {
  const t = useT();
  const data = rows(projects);
  const stuck = useMemo(() => new Set(blockers.flatMap((b) => b.turns)), [blockers]);
  if (data.length === 0) return null;
  const all = data.flatMap((r) => r.turns);
  const from = Math.floor(Math.min(...all.map((turn) => hourOf(turn.start))));
  const to = Math.min(24, Math.ceil(Math.max(...all.map((turn) => hourOf(Math.max(turn.end, turn.start + 60_000))))));
  const span = Math.max(1, to - from);
  const x = (ms: number) => `${((hourOf(ms) - from) / span) * 100}%`;
  const hours = Array.from({ length: span + 1 }, (_, i) => from + i);
  const dim = focus.size > 0;

  return (
    <div className="mt-5" role="img" aria-label={t.summary.timelineAria(from, to)}>
      <div className="grid grid-cols-[7.5rem_1fr] gap-x-3">
        <span />
        <div className="relative h-4 text-[10px] text-faint">
          {hours.map((h, i) => (
            <span key={h} className="absolute -translate-x-1/2 font-mono" style={{ left: `${(i / span) * 100}%` }}>
              {h}
            </span>
          ))}
        </div>
        {data.map((r) => (
          <div key={r.name} className="contents">
            <span className="truncate py-1.5 text-right text-[11.5px] text-muted" title={r.name}>
              {r.name}
            </span>
            <div className="relative h-7 border-t border-line-soft">
              {hours.map((h, i) => (
                <span key={h} aria-hidden className="absolute top-0 h-full border-l border-line-soft/60" style={{ left: `${(i / span) * 100}%` }} />
              ))}
              {r.turns.map((turn) => {
                const lit = focus.has(turn.id);
                const trouble = turn.tool_errors > 0 || turn.interrupted || turn.denials > 0;
                return (
                  <span
                    key={turn.id}
                    title={`${hhmm(turn.start)} · ${duration(Math.round(turn.active_ms / 60_000))}${turn.tool_errors ? t.summary.turnErrors(turn.tool_errors) : ""}${turn.interrupted ? t.summary.turnInterrupted : ""}\n${turn.prompt === CONTINUED_MARKER ? t.summary.continued : turn.prompt}`}
                    className={
                      "absolute top-1.5 h-4 min-w-[3px] rounded-[2px] motion-safe:transition-opacity motion-safe:duration-fast " +
                      (stuck.has(turn.id) ? "bg-fg/70 ring-1 ring-inset ring-fg " : "bg-fg/30 ") +
                      (dim && !lit ? "opacity-25" : "opacity-100")
                    }
                    style={{ left: x(turn.start), width: `calc(${x(Math.max(turn.end, turn.start + 60_000))} - ${x(turn.start)})` }}
                  >
                    {trouble && <span aria-hidden className="absolute -top-1.5 left-0 h-1 w-1 rounded-full bg-accent" />}
                  </span>
                );
              })}
            </div>
          </div>
        ))}
      </div>
      <p className="mt-2 text-[11px] text-faint">{t.summary.timelineNote}</p>
    </div>
  );
}

// ---------- time ----------

export function TimeSpent({
  blocks,
  on,
  usual,
  total,
  spent,
  showCost = true,
}: {
  blocks: TimeBlock[];
  on: ReturnType<typeof useFocus>["on"];
  usual?: number | null;
  total: number;
  /** Turns by id, to add up what each block spent in tokens and cost (days only). */
  spent?: Map<string, Turn>;
  showCost?: boolean;
}) {
  const t = useT();
  if (blocks.length === 0) return null;
  const max = Math.max(1, ...blocks.map((b) => b.minutes));
  const sum = blocks.reduce((a, b) => a + b.minutes, 0) || 1;
  const aside = usual ? (
    <span className="text-[11px] text-faint">
      {duration(total)} {t.summary.recentAverage(duration(usual))}
      {total > usual * 1.2 ? t.summary.longer : total < usual * 0.8 ? t.summary.shorter : ""}
    </span>
  ) : undefined;
  return (
    <Section title={t.summary.timeByTask} aside={aside}>
      <ul className="divide-y divide-line-soft">
        {blocks.map((b) => (
          <li key={b.label} tabIndex={0} {...on(b.turns)} className="group py-2.5 outline-none">
            <div className="flex items-baseline gap-3">
              <span className="min-w-0 flex-1 truncate text-fg/90">
                {b.label}
                {b.project && <span className="ml-2 text-[11.5px] text-faint">{b.project}</span>}
              </span>
              <span className="shrink-0 text-[12px] text-muted">
                {duration(b.minutes)} <span className="text-faint">· {Math.round((b.minutes / sum) * 100)}%</span>
                <BlockSpend ids={b.turns} spent={spent} showCost={showCost} />
              </span>
            </div>
            <div className="mt-1.5 h-1 rounded-full bg-fg/[0.06]">
              <div className="h-1 rounded-full bg-fg/50 group-hover:bg-fg/80 group-focus:bg-fg/80" style={{ width: `${(b.minutes / max) * 100}%` }} />
            </div>
            {b.note && <p className="mt-1.5 max-w-[80ch] text-[12.5px] text-muted">{b.note}</p>}
          </li>
        ))}
      </ul>
    </Section>
  );
}

/** " · $8.20 · 2.1M" for a time block, from the turns it groups. */
function BlockSpend({ ids, spent, showCost }: { ids: string[]; spent?: Map<string, Turn>; showCost: boolean }) {
  if (!spent) return null;
  let cost = 0;
  let toks = 0;
  for (const id of ids) {
    const t = spent.get(id);
    if (!t?.tokens) continue;
    cost += t.cost ?? 0;
    toks += tokenTotal(t.tokens);
  }
  if (toks === 0) return null;
  return (
    <span className="text-faint">
      {showCost && cost > 0 && ` · ${money(cost)}`} · {tokens(toks)}
    </span>
  );
}

// ---------- blockers ----------

export function Blockers({ items, on }: { items: Blocker[]; on: ReturnType<typeof useFocus>["on"] }) {
  const t = useT();
  if (items.length === 0) return null;
  const lost = items.reduce((a, b) => a + b.minutes, 0);
  return (
    <Section title={t.summary.stuckTasks} aside={<span className="text-[11px] text-faint">{t.summary.stuckTime(duration(lost))}</span>}>
      <ul className="divide-y divide-line-soft">
        {items.map((b) => (
          <li key={b.title} tabIndex={0} {...on(b.turns)} className="py-3.5 outline-none">
            <div className="flex items-baseline gap-3">
              <h3 className="min-w-0 flex-1 font-medium">{b.title}</h3>
              <span className="shrink-0 text-[12px] text-muted">{duration(b.minutes)}</span>
              <span
                className={
                  "shrink-0 rounded-md border px-1.5 text-[11px] " + (b.resolved ? "border-line text-faint" : "border-muted text-fg/90")
                }
              >
                {b.resolved ? t.summary.resolved : t.summary.unresolved}
              </span>
            </div>
            <dl className="mt-1.5 grid max-w-[80ch] grid-cols-[4.5rem_1fr] gap-x-3 gap-y-1 text-[12.5px]">
              {b.signal && (
                <>
                  <dt className="text-faint">{t.summary.evidence}</dt>
                  <dd className="text-muted">{b.signal}</dd>
                </>
              )}
              {b.cause && (
                <>
                  <dt className="text-faint">{t.summary.cause}</dt>
                  <dd className="text-muted">{b.cause}</dd>
                </>
              )}
              {b.fix && (
                <>
                  <dt className="text-faint">{t.summary.fix}</dt>
                  <dd className="text-fg/90">{b.fix}</dd>
                </>
              )}
            </dl>
          </li>
        ))}
      </ul>
    </Section>
  );
}

// ---------- diagrams ----------

type Palette = { bg: string; line: string; text: string; node: string; muted: string };

function cssRgb(name: string): string {
  const v = getComputedStyle(document.documentElement).getPropertyValue(`--${name}`).trim().split(/\s+/).join(",");
  return `rgb(${v})`;
}

/** Screen colors follow the theme; paper uses the print tokens (see index.css). */
function palette(paper: boolean): Palette {
  if (paper) return { bg: "rgb(255,255,255)", line: "rgb(140,140,150)", text: "rgb(24,24,27)", node: "rgb(248,248,249)", muted: "rgb(96,96,106)" };
  return { bg: cssRgb("canvas"), line: cssRgb("faint"), text: cssRgb("text"), node: cssRgb("elevated"), muted: cssRgb("muted") };
}

let seq = 0;

async function draw(src: string, paper: boolean): Promise<string> {
  const { default: mermaid } = await import("mermaid");
  const p = palette(paper);
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    theme: "base",
    fontFamily: '-apple-system, BlinkMacSystemFont, "Apple SD Gothic Neo", sans-serif',
    flowchart: { curve: "basis", padding: 12, nodeSpacing: 28, rankSpacing: 36, htmlLabels: false },
    themeVariables: {
      background: p.bg,
      primaryColor: p.node,
      primaryBorderColor: p.line,
      primaryTextColor: p.text,
      lineColor: p.line,
      textColor: p.text,
      edgeLabelBackground: p.bg,
      labelBackground: p.bg,
      clusterBkg: p.node,
      tertiaryTextColor: p.muted,
      fontSize: "12.5px",
    },
  });
  // A long left-to-right chain shrinks to unreadable text at page width; stack it instead.
  const edges = (src.match(/-->/g) ?? []).length;
  const shaped = edges > 4 ? src.replace(/^\s*flowchart LR/, "flowchart TD") : src;
  const { svg } = await mermaid.render(`nr-diagram-${++seq}`, shaped);
  return svg;
}

function Diagram({ v }: { v: Visual }) {
  const [svg, setSvg] = useState<string | null>(null);
  const [failed, setFailed] = useState(false);
  const id = useId();

  useEffect(() => {
    let live = true;
    const render = (paper: boolean) =>
      draw(v.mermaid, paper).then(
        (s) => live && setSvg(s),
        () => live && setFailed(true),
      );
    render(false);
    // Redrawn in paper colors while a PDF is printed, and back after.
    const off = onPaper((paper) => render(paper));
    return () => {
      live = false;
      off();
    };
  }, [v.mermaid]);

  // A diagram that does not parse says nothing; leave it out rather than show an error.
  if (failed) return null;
  return (
    <figure className="break-inside-avoid py-3 [break-inside:avoid-page]" aria-labelledby={id}>
      <figcaption id={id} className="mb-2 text-fg/90">
        {v.title}
      </figcaption>
      <div
        className="overflow-x-auto rounded-lg border border-line-soft bg-surface p-4 [&_svg]:mx-auto [&_svg]:h-auto [&_svg]:max-w-full print:overflow-visible print:bg-transparent"
        // Mermaid in strict mode escapes labels; the source was also filtered to plain flowcharts.
        dangerouslySetInnerHTML={svg ? { __html: svg } : undefined}
      />
      {v.caption && <p className="mt-2 max-w-[80ch] text-[12.5px] text-muted">{v.caption}</p>}
    </figure>
  );
}

export function Diagrams({ items }: { items: Visual[] }) {
  const t = useT();
  if (items.length === 0) return null;
  return (
    <Section title={t.summary.diagrams}>
      {items.map((v) => (
        <Diagram key={v.title + v.mermaid.length} v={v} />
      ))}
    </Section>
  );
}
