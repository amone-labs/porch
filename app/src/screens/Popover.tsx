import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type AccountLimits, type Board, type Entry, type Usage } from "../api";
import { ago, money, tokenTotal, tokens } from "../format";
import { useShowCost } from "../prefs";
import { useT } from "../i18n";
import { StateDot, needsYou, stateLabel } from "../components/ui";

const REFRESH_MS = 2000;

function repoName(root: string) {
  return root.split("/").filter(Boolean).pop() ?? root;
}

/** The 380px menubar popover: what needs you right now, then everything else. */
/** "Codex 5h 81% · Claude weekly 41% · today $23": limits and today's spend in one line. */
function UsageLine() {
  const t = useT();
  const [limits, setLimits] = useState<AccountLimits[]>([]);
  const [today, setToday] = useState<Usage | null>(null);
  const showCost = useShowCost();
  useEffect(() => {
    const load = () => {
      api.limits().then(setLimits, () => {});
      api.usage(1).then((r) => setToday(r.total), () => {});
    };
    load();
    const id = setInterval(load, 5 * 60_000);
    const un = listen("popover-shown", () => api.limits().then(setLimits, () => {}));
    return () => {
      clearInterval(id);
      un.then((f) => f());
    };
  }, []);
  const parts: string[] = [];
  for (const l of limits) {
    const name = l.label.replace(" (Orca)", "");
    if (l.five_hour) parts.push(t.popover.fiveHour(name, Math.round(l.five_hour.used_percent)));
    if (l.weekly) parts.push(t.popover.weekly(Math.round(l.weekly.used_percent)));
  }
  if (today && today.cost + tokenTotal(today.tokens) > 0) parts.push(t.popover.today(showCost ? money(today.cost) : tokens(tokenTotal(today.tokens))));
  if (parts.length === 0) return null;
  return <div className="truncate border-t border-white/[0.06] px-4 py-2 text-[11px] text-muted">{parts.join(" · ")}</div>;
}

export function Popover() {
  const t = useT();
  const [board, setBoard] = useState<Board | null>(null);

  const load = useCallback(() => {
    api.board().then(setBoard, () => {});
  }, []);

  useEffect(() => {
    load();
    const id = setInterval(load, REFRESH_MS);
    const un = listen("popover-shown", load);
    return () => {
      clearInterval(id);
      un.then((f) => f());
    };
  }, [load]);

  const rows: { e: Entry; repo: string }[] = (board?.worktrees ?? []).flatMap((w) => w.entries.map((e) => ({ e, repo: repoName(w.root) })));
  rows.sort((a, b) => Number(needsYou(b.e)) - Number(needsYou(a.e)));
  const waiting = rows.filter((r) => needsYou(r.e)).length;

  return (
    <div className="flex h-full flex-col overflow-hidden rounded-xl border border-white/10 bg-elevated/[0.94] shadow-[inset_0_1px_0_rgba(255,255,255,0.10)] backdrop-blur-xl">
      <header className="flex items-baseline justify-between px-4 pb-2 pt-3.5">
        <span className="text-[12.5px] font-medium">
          porch<span className="text-accent">.</span>
        </span>
        <span className="text-[11.5px] text-muted">{waiting > 0 ? t.popover.waiting(waiting) : t.popover.noneWaiting}</span>
      </header>

      <ul className="min-h-0 flex-1 overflow-y-auto px-1.5">
        {board && rows.length === 0 && <li className="px-3 py-8 text-center text-muted">{t.common.noSessions}</li>}
        {rows.map(({ e, repo }, i) => (
          <li key={e.agent + e.session}>
            {i > 0 && needsYou(rows[i - 1].e) && !needsYou(e) && <div className="mx-2.5 my-1.5 border-t border-white/[0.06]" />}
            <button
              type="button"
              onClick={() => api.openMain("status")}
              className="flex h-9 w-full items-center gap-2.5 rounded-md px-2.5 text-left motion-safe:transition-colors motion-safe:duration-fast hover:bg-fg/5"
            >
              <StateDot e={e} />
              <span className="min-w-0 flex-1">
                <span className={"block truncate text-[12.5px] " + (needsYou(e) ? "text-fg" : "text-muted")}>{e.title}</span>
                <span className="block truncate text-[11px] text-muted">
                  {stateLabel(e, t)} · {repo}
                </span>
              </span>
              <span className="shrink-0 text-[11px] text-muted">{board ? ago(board.now, e.last_event_at) : ""}</span>
            </button>
          </li>
        ))}
      </ul>

      <UsageLine />
      <footer className="flex gap-1.5 border-t border-white/[0.06] p-2">
        <button type="button" onClick={() => api.openMain("today")} className="h-8 flex-1 rounded-md text-[12.5px] text-muted hover:bg-fg/5 hover:text-fg">
          {t.summary.todaySummary}
        </button>
        <button type="button" onClick={() => api.openMain("summary")} className="h-8 flex-1 rounded-md text-[12.5px] text-muted hover:bg-fg/5 hover:text-fg">
          {t.popover.openApp}
        </button>
      </footer>
    </div>
  );
}
