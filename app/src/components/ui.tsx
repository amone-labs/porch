import { useEffect, useState, type ReactNode } from "react";
import type { Entry } from "../api";
import { lasting } from "../format";
import { useT } from "../i18n";
import type { Dict } from "../i18n/ko";
import { Icon } from "./Icon";

/** App-shell buttons: h-7, rounded-md, white-alpha ramp (docs/design/ui.md). */
export function Button({
  children,
  onClick,
  disabled,
  primary,
  title,
}: {
  children: ReactNode;
  onClick: () => void;
  disabled?: boolean;
  primary?: boolean;
  title?: string;
}) {
  return (
    <button
      type="button"
      onClick={onClick}
      disabled={disabled}
      title={title}
      className={
        "inline-flex h-7 shrink-0 items-center gap-1.5 whitespace-nowrap rounded-md px-2.5 text-[12.5px] font-medium motion-safe:transition-colors motion-safe:duration-fast motion-safe:ease-calm disabled:opacity-50 " +
        (primary ? "bg-accent text-on-accent hover:opacity-90" : "border border-line text-muted hover:bg-fg/5 hover:text-fg")
      }
    >
      {children}
    </button>
  );
}

/** "3h 50m · 3 projects · 4 sessions" — numbers in one quiet line instead of stat boxes. */
export function StatLine({ items }: { items: (string | false | null | undefined)[] }) {
  return (
    <p className="text-muted">
      {items.filter(Boolean).map((s, i) => (
        <span key={i}>
          {i > 0 && <span className="px-1.5 text-faint">·</span>}
          {s}
        </span>
      ))}
    </p>
  );
}

export function Group({ title, children }: { title: string; children: ReactNode }) {
  return (
    <div className="mt-3 first:mt-0">
      <h4 className="label-ko mb-1">{title}</h4>
      {children}
    </div>
  );
}

export function Bullets({ title, items }: { title: string; items: string[] }) {
  if (items.length === 0) return null;
  return (
    <Group title={title}>
      <ul className="space-y-0.5">
        {items.map((i, n) => (
          <li key={n} className="flex gap-2">
            <span aria-hidden className="text-faint">–</span>
            <span>{i}</span>
          </li>
        ))}
      </ul>
    </Group>
  );
}

export function Section({ title, children, aside, id }: { title: string; children: ReactNode; aside?: ReactNode; id?: string }) {
  return (
    <section id={id} className="mt-10 scroll-mt-6">
      <div className="mb-2 flex items-baseline justify-between border-b border-line-soft pb-1.5">
        <h2 className="label-ko">{title}</h2>
        {aside}
      </div>
      {children}
    </section>
  );
}

export function Disclosure({ label, children }: { label: string; children: ReactNode }) {
  return (
    <details className="group mt-2 text-[12.5px] text-muted">
      <summary className="inline-flex cursor-pointer items-center gap-1 rounded hover:text-fg">
        <Icon name="chevronRight" size={12} className="motion-safe:transition-transform motion-safe:duration-fast group-open:rotate-90" />
        {label}
      </summary>
      <div className="mt-1 pl-4">{children}</div>
    </details>
  );
}

/** Activity by hour: thin bars, height only. */
export function HoursStrip({ byHour }: { byHour: number[] }) {
  const t = useT();
  const max = Math.max(1, ...byHour);
  return (
    <div className="flex items-end gap-3" role="img" aria-label={t.hours.aria}>
      <div className="flex h-7 flex-1 items-end gap-[2px]">
        {byHour.map((n, h) => (
          <span
            key={h}
            title={t.hours.bar(h, n)}
            className={"block flex-1 rounded-[1px] " + (n > 0 ? "bg-fg/40" : "bg-fg/10")}
            style={{ height: n > 0 ? `${Math.max(12, (n / max) * 100)}%` : "2px" }}
          />
        ))}
      </div>
      <span className="text-[10.5px] text-faint">{t.hours.axis}</span>
    </div>
  );
}

export function needsYou(e: Entry) {
  return e.state === "permission" || e.state === "question" || e.state === "failed";
}

/** Brightness, not colour: bright dot = needs you, grey dot = running, ring = nothing to do. */
export function StateDot({ e }: { e: Entry }) {
  const cls = needsYou(e) ? "bg-accent" : e.state === "running" ? "bg-muted" : "border border-faint";
  return <span aria-hidden className={`inline-block h-2 w-2 shrink-0 rounded-full ${cls}`} />;
}

export function stateLabel(e: Entry, t: Dict): string {
  const s = t.state[e.state];
  // The permission's tool is shown on its own line (the ask), with what it wants to run.
  return e.state === "permission" && e.pending?.tool && !e.ask ? `${s} · ${e.pending.tool}` : s;
}

/** How long a session has been in its state, where that matters: waiting on you, or running. */
export function stateTime(e: Entry, now: number): { text: string; long: boolean } | null {
  const waiting = e.state === "permission" || e.state === "question";
  if (waiting) return { text: lasting(now, e.state_since), long: now - e.state_since >= 10 * 60_000 };
  if (e.state === "running") return { text: lasting(now, e.turn_started_at ?? e.state_since), long: false };
  return null;
}

export function Empty({ children }: { children: ReactNode }) {
  return <p className="py-10 text-center text-muted">{children}</p>;
}

/** Tab-favicon style loading ring: a faint track with one turning arc, in the text colour. Still under reduced motion. */
export function Spinner({ size = 12 }: { size?: number }) {
  return (
    <svg aria-hidden width={size} height={size} viewBox="0 0 16 16" fill="none" className="shrink-0 motion-safe:animate-spin">
      <circle cx="8" cy="8" r="6.5" stroke="currentColor" strokeOpacity="0.2" strokeWidth="2" />
      <path d="M8 1.5a6.5 6.5 0 0 1 6.5 6.5" stroke="currentColor" strokeWidth="2" strokeLinecap="round" />
    </svg>
  );
}

/** Quiet progress: seconds since `since`. Counts from the job's start, so it survives a remount. */
export function useElapsed(since: number | null): number {
  const [now, setNow] = useState(Date.now());
  useEffect(() => {
    if (since === null) return;
    setNow(Date.now());
    const id = setInterval(() => setNow(Date.now()), 1000);
    return () => clearInterval(id);
  }, [since]);
  return since === null ? 0 : Math.max(0, Math.floor((now - since) / 1000));
}

/** Big line with generous space: the one loud element on a screen. */
export function Headline({ eyebrow, children, action }: { eyebrow: ReactNode; children: ReactNode; action?: ReactNode }) {
  return (
    <header className="pb-6 pt-4">
      <div className="flex items-center justify-between gap-4">
        <span className="label-ko">{eyebrow}</span>
        {action}
      </div>
      <h1 className="mt-3 max-w-[40em] text-[21px] font-medium leading-snug tracking-[-0.02em]">{children}</h1>
    </header>
  );
}
