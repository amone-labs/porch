import { useLayoutEffect, useRef, useState } from "react";
import { dismiss, pauseNotices, useNotices } from "../notice";
import { useT } from "../i18n";

/** How far each older notice peeks out above the one in front, and how much smaller it is. */
const PEEK = 8;
const SHRINK = 0.05;
/** Behind this many, older notices are hidden until the stack opens. */
const VISIBLE = 3;
const GAP = 8;

/**
 * Notices at the bottom centre. Newest in front; older ones tuck in behind it,
 * a little higher and smaller, like a deck. Hovering opens the deck upward so
 * each one can be read and acted on; leaving folds it back.
 */
export function NoticeStack() {
  const t = useT();
  const list = useNotices();
  const [open, setOpen] = useState(false);
  const refs = useRef(new Map<number, HTMLDivElement>());
  const [heights, setHeights] = useState<Record<number, number>>({});

  // Measure each notice's natural height to lay the opened deck out.
  useLayoutEffect(() => {
    const next: Record<number, number> = {};
    for (const n of list) next[n.id] = refs.current.get(n.id)?.scrollHeight ?? 0;
    if (list.some((n) => next[n.id] !== heights[n.id]) || Object.keys(heights).length !== list.length) setHeights(next);
  });

  // The last one closed under the pointer: no mouseleave will come.
  useLayoutEffect(() => {
    if (list.length === 0 && open) {
      setOpen(false);
      pauseNotices(false);
    }
  }, [list.length, open]);

  if (list.length === 0) return null;

  const front = [...list].reverse(); // index 0 is the newest
  const frontHeight = heights[front[0].id] ?? 0;
  const tops: number[] = [];
  front.reduce((y, n) => (tops.push(y), y + (heights[n.id] ?? 0) + GAP), 0);
  const openHeight = front.reduce((h, n) => h + (heights[n.id] ?? 0), 0) + GAP * (front.length - 1);

  return (
    <section
      aria-label={t.notices.aria}
      aria-live="polite"
      onMouseEnter={() => {
        setOpen(true);
        pauseNotices(true);
      }}
      onMouseLeave={() => {
        setOpen(false);
        pauseNotices(false);
      }}
      className="print:hidden fixed bottom-5 left-1/2 z-20 w-[400px] max-w-[calc(100vw-32px)] -translate-x-1/2"
      style={{ height: open ? openHeight : frontHeight + PEEK * Math.min(front.length - 1, VISIBLE - 1) }}
    >
      {front.map((n, i) => {
        const hidden = !open && i >= VISIBLE;
        const y = open ? -tops[i] : -PEEK * i;
        const scale = open ? 1 : 1 - SHRINK * i;
        return (
          <div
            key={n.id}
            ref={(el) => {
              if (el) refs.current.set(n.id, el);
              else refs.current.delete(n.id);
            }}
            role="status"
            className="absolute inset-x-0 bottom-0 origin-bottom overflow-hidden rounded-lg border border-line bg-elevated text-[12.5px] text-fg/90 shadow-lg motion-safe:transition-[transform,opacity,height] motion-safe:duration-200 motion-safe:ease-out"
            style={{
              transform: `translateY(${y}px) scale(${scale})`,
              zIndex: 50 - i,
              opacity: hidden ? 0 : 1,
              pointerEvents: hidden ? "none" : undefined,
              // Folded, the ones behind take the front one's height so only their top edge shows.
              height: !open && i > 0 && frontHeight ? frontHeight : undefined,
            }}
          >
            <div
              className="notice-in flex items-start gap-3 py-1.5 pl-3.5 pr-1.5 motion-safe:transition-opacity motion-safe:duration-fast"
              style={{ opacity: open || i === 0 ? 1 : 0 }}
            >
              <span className="min-w-0 flex-1 py-[5px] leading-[1.45]">{n.text}</span>
              {n.action && (
                <button
                  type="button"
                  onClick={() => {
                    n.action!.run();
                    dismiss(n.id);
                  }}
                  className="h-7 shrink-0 rounded-md px-2.5 font-medium text-fg hover:bg-fg/10"
                >
                  {n.action.label}
                </button>
              )}
              <button
                type="button"
                aria-label={t.notices.close}
                onClick={() => dismiss(n.id)}
                className="h-7 w-7 shrink-0 rounded-md text-[15px] leading-none text-faint hover:bg-fg/10 hover:text-fg"
              >
                ×
              </button>
            </div>
          </div>
        );
      })}
    </section>
  );
}
