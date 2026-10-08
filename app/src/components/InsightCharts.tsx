// Small charts for the insight screen. Achromatic: brightness carries the
// series and one bright mark carries what the section is about. Every value is
// in a tooltip on hover and keyboard focus.
import { useId, useState, type CSSProperties } from "react";
import { hm } from "../format";

export type Tone = "hi" | "mid" | "lo";
const FILL: Record<Tone, string> = { hi: "bg-fg/85", mid: "bg-fg/40", lo: "bg-fg/15" };
export const KEY: Record<Tone, string> = { hi: "bg-fg", mid: "bg-fg/40", lo: "bg-fg/20" };

export interface TipRow {
  value: string;
  label: string;
  tone?: Tone;
}

export function Tip({ id, title, rows, style }: { id: string; title: string; rows: TipRow[]; style: CSSProperties }) {
  return (
    <div id={id} role="tooltip" style={style}
      className="pointer-events-none absolute z-10 w-max min-w-40 max-w-64 rounded-md border border-line bg-elevated p-2.5 text-[12px] shadow-lg">
      <p className="mb-1 text-faint">{title}</p>
      {rows.map((r, i) => (
        <p key={i} className="grid grid-cols-[10px_auto_minmax(0,1fr)] items-center gap-2 leading-6">
          <span aria-hidden className={"h-0.5 w-2.5 rounded " + (r.tone ? KEY[r.tone] : "")} />
          <span className="font-medium text-fg tabular-nums">{r.value}</span>
          <span className="text-muted">{r.label}</span>
        </p>
      ))}
    </div>
  );
}

export interface Column {
  key: string;
  tick?: string;
  /** Printed over the column; leave out for small charts. */
  top?: string;
  segments: { value: number; tone: Tone; series?: string }[];
  tip: { title: string; rows: TipRow[] };
  onClick?: () => void;
}

/**
 * Columns from one baseline; a column's segments stack bottom-up with a 2px gap.
 * Hovering or focusing a column dims the others; `emphasis` dims every segment
 * not of that series (the legend).
 */
export function Columns({ columns, height, label, emphasis = null, ticks = true }: {
  columns: Column[]; height: number; label: string; emphasis?: string | null; ticks?: boolean;
}) {
  const [active, setActive] = useState<number | null>(null);
  const [pointer, setPointer] = useState<{ x: number; y: number; w: number } | null>(null);
  const tipId = useId();
  const sum = (c: Column) => c.segments.reduce((a, s) => a + s.value, 0);
  const max = Math.max(0, ...columns.map(sum));
  const room = height - (columns.some((c) => c.top) ? 16 : 0);
  const shown = active === null ? null : columns[active];
  const tipStyle: CSSProperties = pointer
    ? { left: Math.min(Math.max(0, pointer.x + 14), Math.max(0, pointer.w - 220)), top: Math.max(0, pointer.y - 12) }
    : { left: `clamp(0px, ${((active ?? 0) + 0.5) / columns.length * 100}% - 100px, calc(100% - 220px))`, top: 0 };
  return (
    <div className="relative" role="group" aria-label={label}
      onMouseMove={(e) => { const b = e.currentTarget.getBoundingClientRect(); setPointer({ x: e.clientX - b.left, y: e.clientY - b.top, w: b.width }); }}
      onMouseLeave={() => { setActive(null); setPointer(null); }}>
      <div className="flex items-end gap-1" style={{ height }}>
        {columns.map((c, i) => {
          const v = sum(c);
          const lastShown = c.segments.map((s) => s.value > 0).lastIndexOf(true);
          const dim = active !== null && active !== i;
          return (
            <button key={c.key} type="button"
              aria-label={`${c.tip.title} · ${c.tip.rows.map((r) => `${r.label} ${r.value}`).join(" · ")}`}
              aria-describedby={active === i ? tipId : undefined}
              onMouseEnter={() => setActive(i)}
              onFocus={() => { setActive(i); setPointer(null); }}
              onBlur={() => setActive(null)}
              onKeyDown={(e) => { if (e.key === "Escape") setActive(null); }}
              onClick={c.onClick}
              className={"flex h-full min-w-0 flex-1 flex-col items-center justify-end rounded-sm outline-none focus-visible:ring-1 focus-visible:ring-fg/60 " + (c.onClick ? "cursor-pointer" : "cursor-default")}>
              {c.top && v > 0 && <span className={"mb-1 text-[10.5px] tabular-nums motion-safe:transition-colors motion-safe:duration-fast " + (dim ? "text-faint" : "text-muted")}>{c.top}</span>}
              <span aria-hidden className="flex w-full max-w-6 flex-col-reverse gap-[2px]" style={{ height: max > 0 ? Math.max(v > 0 ? 1 : 0, (v / max) * room) : 0 }}>
                {c.segments.map((s, j) => s.value > 0 && (
                  <span key={j}
                    className={FILL[s.tone] + " w-full motion-safe:transition-opacity motion-safe:duration-fast " + (j === lastShown ? "rounded-t-[4px]" : "")}
                    style={{ flexGrow: s.value, flexBasis: 0, minHeight: 1, opacity: dim || (emphasis !== null && s.series !== emphasis) ? 0.3 : 1 }} />
                ))}
              </span>
            </button>
          );
        })}
      </div>
      {ticks && (
        <div className="mt-1.5 flex gap-1" aria-hidden>
          {columns.map((c) => <span key={c.key} className="min-w-0 flex-1 text-center text-[10.5px] text-faint tabular-nums">{c.tick ?? ""}</span>)}
        </div>
      )}
      {shown && <Tip id={tipId} title={shown.tip.title} rows={shown.tip.rows} style={tipStyle} />}
    </div>
  );
}

/** Horizontal bars, one row each, value at the tip. */
export function HBars({ rows, label }: { rows: { name: string; value: number; text: string; tip: TipRow[] }[]; label: string }) {
  const [active, setActive] = useState<number | null>(null);
  const tipId = useId();
  const max = Math.max(0, ...rows.map((r) => r.value));
  return (
    <div className="relative grid gap-1.5" role="group" aria-label={label} onMouseLeave={() => setActive(null)}>
      {rows.map((r, i) => (
        <button key={r.name} type="button" aria-describedby={active === i ? tipId : undefined}
          aria-label={`${r.name} · ${r.tip.map((t) => `${t.label} ${t.value}`).join(" · ")}`}
          onMouseEnter={() => setActive(i)} onFocus={() => setActive(i)} onBlur={() => setActive(null)}
          className="grid grid-cols-[5rem_minmax(0,1fr)] items-center gap-3 rounded-sm text-left outline-none focus-visible:ring-1 focus-visible:ring-fg/60">
          <span className="truncate text-[12px] text-muted">{r.name}</span>
          <span className="flex items-center gap-2">
            <span aria-hidden className={"h-3.5 rounded-r-[4px] bg-fg/40 motion-safe:transition-opacity motion-safe:duration-fast " + (active !== null && active !== i ? "opacity-30" : "")}
              style={{ width: max > 0 ? `max(2px, ${(r.value / max) * 85}%)` : 2 }} />
            <span className="shrink-0 text-[12px] text-fg tabular-nums">{r.text}</span>
          </span>
        </button>
      ))}
      {active !== null && <Tip id={tipId} title={rows[active].name} rows={rows[active].tip} style={{ left: "5.75rem", top: active * 26 + 24 }} />}
    </div>
  );
}

/** One bar split into parts of a whole, with a 2px gap between them. */
export function ShareBar({ parts, label }: { parts: { name: string; value: number; tone: Tone; tip: TipRow[] }[]; label: string }) {
  const [active, setActive] = useState<number | null>(null);
  const tipId = useId();
  const total = parts.reduce((a, p) => a + p.value, 0);
  return (
    <div className="relative" role="group" aria-label={label} onMouseLeave={() => setActive(null)}>
      <div className="flex h-3.5 gap-[2px]">
        {parts.map((p, i) => p.value > 0 && (
          <button key={p.name} type="button" aria-describedby={active === i ? tipId : undefined}
            aria-label={`${p.name} · ${p.tip.map((t) => `${t.label} ${t.value}`).join(" · ")}`}
            onMouseEnter={() => setActive(i)} onFocus={() => setActive(i)} onBlur={() => setActive(null)}
            className={FILL[p.tone] + " outline-none first:rounded-l-[4px] last:rounded-r-[4px] focus-visible:ring-1 focus-visible:ring-fg/60 motion-safe:transition-opacity motion-safe:duration-fast " + (active !== null && active !== i ? "opacity-30" : "")}
            style={{ flexGrow: p.value, flexBasis: 0 }} />
        ))}
      </div>
      <div className="mt-1.5 flex justify-between gap-2 text-[12px] text-muted tabular-nums">
        {parts.map((p) => <span key={p.name}>{p.name} {hm(p.value)}</span>)}
      </div>
      {total > 0 && active !== null && <Tip id={tipId} title={parts[active].name} rows={parts[active].tip} style={{ left: 0, top: 22 }} />}
    </div>
  );
}
