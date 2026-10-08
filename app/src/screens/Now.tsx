import { useCallback, useEffect, useState } from "react";
import { track } from "../analytics";
import { api, type Board, type Entry } from "../api";
import { ago, homeRelative, shortModel, tokens } from "../format";
import { useT } from "../i18n";
import { Empty, Headline, StatLine, StateDot, needsYou, stateLabel, stateTime } from "../components/ui";

const REFRESH_MS = 2000;
const key = (e: Entry) => e.agent + ":" + e.session;

function basename(p: string) {
  return p.split("/").filter(Boolean).pop() ?? p;
}

type FocusErrorCode = "no_orca" | "not_running" | "tab_gone" | "not_focusable";
const FOCUS_ERRORS: FocusErrorCode[] = ["no_orca", "not_running", "tab_gone", "not_focusable"];
const focusCode = (err: unknown): FocusErrorCode => FOCUS_ERRORS.find((c) => String(err).includes(c)) ?? "not_running";

/** "12 min" under the state, bright once a wait passes ten minutes; the turn's failures beside it. */
function StateTime({ e, now }: { e: Entry; now: number }) {
  const t = useT();
  const st = stateTime(e, now);
  const errors = e.turn_errors > 0 && (e.state === "running" || e.state === "permission" || e.state === "question") ? e.turn_errors : 0;
  if (!st && !errors) return null;
  return (
    <div className="text-[11.5px]">
      {st && <span className={st.long ? "font-medium text-fg" : "text-faint"}>{st.text}</span>}
      {errors > 0 && <span className="text-muted">{st ? " · " : ""}{t.sessions.errors(errors)}</span>}
    </div>
  );
}

/** "opus 5.5 · context 72%": which model, and how full its context window is; bright past 80%. */
function ModelContext({ e }: { e: Entry }) {
  const t = useT();
  if (!e.model && e.context_percent == null && e.context_tokens == null) return null;
  const full = e.context_percent != null && e.context_percent >= 80;
  const ctx = e.context_percent != null ? t.sessions.context(`${Math.round(e.context_percent)}%`) : e.context_tokens != null ? t.sessions.context(tokens(e.context_tokens)) : null;
  return (
    <span className="mt-[1px] shrink-0 text-[11.5px] text-faint" title={e.model ?? undefined}>
      {e.model && shortModel(e.model)}
      {e.model && ctx && " · "}
      {ctx && <span className={full ? "font-medium text-fg" : ""}>{ctx}</span>}
    </span>
  );
}

export function Now() {
  const t = useT();
  const [board, setBoard] = useState<Board | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [focusError, setFocusError] = useState<FocusErrorCode | null>(null);

  const load = useCallback(() => {
    api.board().then(
      (b) => {
        setBoard(b);
        setError(null);
      },
      (e) => setError(String(e)),
    );
  }, []);

  useEffect(() => {
    load();
    const id = setInterval(load, REFRESH_MS);
    return () => clearInterval(id);
  }, [load]);

  const entries = board?.worktrees.flatMap((w) => w.entries) ?? [];
  // Clicking or Enter brings the session's terminal tab to the front; nothing is sent to the agent.
  const focus = (e: Entry) => {
    if (!e.focusable) return;
    api.focus(e.term_program, e.term_pane).then(
      () => {
        setFocusError(null);
        track("session_focused", { ok: true });
      },
      (err) => {
        setFocusError(focusCode(err));
        track("session_focused", { ok: false });
      },
    );
  };

  // ↑↓ move, Enter brings the selected session's tab to the front.
  useEffect(() => {
    const onKey = (ev: KeyboardEvent) => {
      if (ev.target instanceof HTMLElement && ["INPUT", "TEXTAREA"].includes(ev.target.tagName)) return;
      const keys = entries.map(key);
      const i = selected ? keys.indexOf(selected) : -1;
      if (ev.key === "ArrowDown" || ev.key === "ArrowUp") {
        ev.preventDefault();
        const next = ev.key === "ArrowDown" ? Math.min(keys.length - 1, i + 1) : Math.max(0, i - 1);
        setSelected(keys[next] ?? null);
      } else if (ev.key === "Enter" && selected) {
        ev.preventDefault();
        const e = entries.find((x) => key(x) === selected);
        if (e) focus(e);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  if (error) return <Empty>{t.sessions.loadFailed(error)}</Empty>;
  if (!board) return <Empty>{t.common.loading}</Empty>;

  const waiting = entries.filter(needsYou).length;
  const mixedAgents = new Set(entries.map((e) => e.agent)).size > 1;

  return (
    <div>
      <Headline eyebrow={t.nav.sessions}>{waiting > 0 ? t.sessions.waiting(waiting) : t.sessions.noneWaiting}</Headline>
      <StatLine items={[t.sessions.live(entries.length), t.sessions.folders(board.worktrees.length), t.sessions.keys]} />
      {focusError && <p className="mt-2 text-[12px] text-muted">{t.sessions.focusFailed[focusError]}</p>}

      {board.worktrees.length === 0 && <Empty>{t.common.noSessions}</Empty>}
      {board.worktrees.map((w) => (
        <section key={w.root} className="mt-8">
          <header className="flex items-baseline gap-2 border-b border-line-soft pb-1.5" title={w.root}>
            <span className="font-medium">{basename(w.root)}</span>
            {w.branch && <span className="font-mono text-[11px] text-faint">{w.branch}</span>}
            {!!w.dirty && <span className="ml-auto text-[11.5px] text-muted">{t.sessions.dirty(w.dirty)}</span>}
          </header>
          <ul>
            {w.entries.map((e) => {
              const k = key(e);
              return (
                <li key={k}>
                  <div
                    onClick={() => {
                      setSelected(k);
                      focus(e);
                    }}
                    title={[e.cwd && homeRelative(e.cwd), e.term_program, !e.focusable && t.sessions.notFocusable].filter(Boolean).join(" · ")}
                    className={
                      "group flex min-h-[52px] items-start gap-3 rounded-md px-2 py-2 motion-safe:transition-colors motion-safe:duration-fast " +
                      (e.focusable ? "cursor-pointer " : "cursor-default ") +
                      (selected === k ? "bg-fg/10" : "hover:bg-fg/5")
                    }
                  >
                    <span className="mt-[6px]">
                      <StateDot e={e} />
                    </span>
                    <div className="w-40 shrink-0">
                      <div className={"truncate " + (needsYou(e) ? "text-fg" : "text-muted")}>{stateLabel(e, t)}</div>
                      <StateTime e={e} now={board.now} />
                    </div>
                    <div className="min-w-0 flex-1">
                      <div className={"truncate " + (needsYou(e) ? "" : "text-muted")}>{e.title}</div>
                      {e.ask ? (
                        <div className="truncate text-[12px] text-fg/90" title={e.ask}>
                          <span className="text-faint">{t.sessions.permissionAsk} </span>
                          {e.ask}
                        </div>
                      ) : (
                        e.last_prompt && (
                          <div className="truncate text-[12px] text-muted" title={e.last_prompt}>
                            <span className="text-faint">{t.sessions.request} </span>
                            {e.last_prompt}
                          </div>
                        )
                      )}
                    </div>
                    <ModelContext e={e} />
                    {e.subagents.length > 0 && <span className="mt-[1px] text-[11.5px] text-faint">{t.sessions.subagents(e.subagents.length)}</span>}
                    {mixedAgents && <span className="mt-[1px] font-mono text-[11px] text-faint">{e.agent}</span>}
                    <span className="mt-[1px] w-24 text-right text-[11.5px] text-faint">
                      {e.stale ? t.sessions.stale(ago(board.now, e.last_event_at)) : ago(board.now, e.last_event_at)}
                    </span>
                  </div>
                </li>
              );
            })}
          </ul>
        </section>
      ))}
    </div>
  );
}
