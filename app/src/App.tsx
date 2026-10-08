import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, type Environment, type Provider } from "./api";
import { EnvContext } from "./env";
import { Onboarding } from "./screens/Onboarding";
import { Icon, type IconName } from "./components/Icon";
import { Now } from "./screens/Now";
import { Usage } from "./screens/Usage";
import { InsightScreen } from "./screens/Insight";
import { Summary, type SummaryOpen } from "./screens/Reports";
import { Projects } from "./screens/Projects";
import { Settings, type SettingsTab } from "./screens/Settings";
import { notify } from "./notice";
import { NoticeStack } from "./components/NoticeStack";
import { showUpdate, useUpdate, watchUpdates } from "./update";
import { UpdateModal } from "./components/UpdateModal";
import { Spinner } from "./components/ui";
import { onJobDone, useJobs, type Job } from "./jobs";
import { keep } from "./kept";
import { failureReason, track, trackActive, trackAppOpened, trackSuggestions, trackSummary } from "./analytics";
import { useT } from "./i18n";
import type { Dict } from "./i18n/ko";

type Screen = "summary" | "insight" | "status" | "usage" | "projects" | "settings";

/** Names the tray and popover may send, old ones included. */
function screenFrom(name: string): { screen: Screen; open?: "today" } {
  switch (name) {
    case "now":
    case "status":
      return { screen: "status" };
    case "today":
      return { screen: "summary", open: "today" };
    case "history":
    case "summary":
      return { screen: "summary" };
    case "insight":
    case "usage":
    case "projects":
    case "settings":
      return { screen: name };
    default:
      return { screen: "summary" };
  }
}

/** The screen a job's result shows up on. */
function screenOf(job: Job): Screen {
  if (job.key.startsWith("insight:")) return "insight";
  return job.key === "backfill" || job.key === "suggestions" || job.key === "classify-blockers" ? "projects" : "summary";
}

/** For a summary job, the day or week to open when the notice is followed. */
function openOf(job: Job): SummaryOpen | undefined {
  const [kind, date] = job.key.split(":");
  if (kind === "month") return { kind, date: `${date}-01` };
  return kind === "day" || kind === "week" ? { kind, date } : undefined;
}

/** `label` names a key in the `nav` dictionary section; `id` stays the screen id analytics reads. */
type NavItem = { id: Screen; label: keyof Dict["nav"]; icon: IconName; key?: string; soon?: boolean };

const NAV: NavItem[] = [
  { id: "summary", label: "summary", icon: "today", key: "1" },
  { id: "insight", label: "insight", icon: "insight", key: "2" },
  { id: "status", label: "sessions", icon: "now", key: "3" },
  { id: "usage", label: "usage", icon: "usage", key: "4" },
  { id: "projects", label: "projects", icon: "projects", key: "5" },
];

/** Sessions waiting on the person, refreshed as often as the menubar count. */
function useWaiting(): number {
  const [n, setN] = useState(0);
  useEffect(() => {
    const load = () => api.waiting().then(setN, () => {});
    load();
    const id = setInterval(load, 3000);
    return () => clearInterval(id);
  }, []);
  return n;
}

const SEEN_KEY = "porch.insightSeenAt";

function seenAt(): number {
  try {
    return Number(localStorage.getItem(SEEN_KEY)) || 0;
  } catch {
    return 0;
  }
}

/** A dot on Insights while an evaluation newer than the last visit waits. */
function useInsightNew(screen: Screen): boolean {
  const [latest, setLatest] = useState<number | null>(null);
  const [seen, setSeen] = useState(seenAt);
  useEffect(() => {
    const load = () => api.insightLatest().then(setLatest, () => {});
    load();
    const un = listen("insight-updated", load);
    window.addEventListener("focus", load);
    return () => {
      un.then((f) => f());
      window.removeEventListener("focus", load);
    };
  }, []);
  useEffect(() => {
    if (screen !== "insight") return;
    const now = Date.now();
    try {
      localStorage.setItem(SEEN_KEY, String(now));
    } catch {
      /* storage unavailable: the dot just comes back next launch */
    }
    setSeen(now);
  }, [screen, latest]);
  return latest !== null && latest > seen && screen !== "insight";
}

export default function App() {
  const t = useT();
  // The job-done listener registers once; keep it reading the language now in use.
  const tRef = useRef(t);
  tRef.current = t;
  const [screen, setScreen] = useState<Screen>("summary");
  const [today, setToday] = useState<string | null>(null);
  const [env, setEnv] = useState<Environment | null>(null);
  // Settings opened from a notice land on the tab it names; the sidebar opens the default.
  const [settingsAt, setSettingsAt] = useState<{ tab: SettingsTab; focus?: "summary"; at: number } | undefined>(undefined);
  // Keep summary navigation here so leaving the screen preserves its period.
  const [summaryOpen, setSummaryOpen] = useState<SummaryOpen | null>(null);
  const waiting = useWaiting();
  const insightNew = useInsightNew(screen);
  const update = useUpdate();
  useEffect(() => watchUpdates(), []);
  const jumpTo = (open: SummaryOpen | null) => setSummaryOpen(open);
  const jobs = useJobs();
  const screenRef = useRef(screen);
  screenRef.current = screen;

  // Leaving a screen whose summary is still being written is allowed: the job
  // keeps going and saves itself. Say so once, so nobody waits on the old tab.
  const go = (next: Screen) => {
    const here = jobs.filter((j) => screenOf(j) === screen);
    if (next !== screen && here.length > 0) {
      notify({ text: t.notices.stillWriting(here[0].label(t)) });
    }
    setSettingsAt(undefined);
    setScreen(next);
  };
  const goRef = useRef(go);
  goRef.current = go;

  useEffect(
    () =>
      onJobDone((job, ok, error, value) => {
        const [kind] = job.key.split(":");
        const duration_ms = Date.now() - job.startedAt;
        if (kind === "day" || kind === "week" || kind === "month") {
          // Nothing worked on in the period: no summary and no error, nothing written.
          if (!ok || (value as { summary?: unknown } | undefined)?.summary) {
            trackSummary({ kind, auto: false, duration_ms, refresh: !!job.refresh, error });
          }
        } else if (job.key === "suggestions") {
          trackSuggestions({ duration_ms, result: value as { added: number; dropped: number } | undefined, error });
        }
        const where = screenOf(job);
        // A usage limit is worth a notice even on the screen that shows the error:
        // switching the writer in Settings gets the summary now.
        const limit = error && failureReason(error) === "limit" ? error.split(". ")[0] : undefined;
        if (limit) {
          notify({
            key: `limit:${job.key}`,
            text: tRef.current.notices.limitNotice(job.label(tRef.current), limit),
            action: {
              label: tRef.current.notices.changeAgent,
              run: () => {
                setSettingsAt({ tab: "general", focus: "summary", at: Date.now() });
                setScreen("settings");
              },
            },
            sticky: true,
          });
          return;
        }
        if (where === screenRef.current) return;
        const text = ok ? tRef.current.notices.ready(job.label(tRef.current)) : tRef.current.notices.failed(job.label(tRef.current));
        const open = openOf(job);
        notify({
          text,
          action: {
            label: tRef.current.notices.view,
            run: () => {
              if (open) jumpTo(open);
              setScreen(where);
            },
          },
        });
      }),
    [],
  );

  // The first-run page stands in front of every screen until it is finished.
  const shown = !env ? null : env.onboarded ? screen : "onboarding";
  useEffect(() => {
    if (shown) track("screen_viewed", { screen: shown });
  }, [shown]);

  useEffect(() => {
    trackAppOpened();
    trackActive();
    const unDay = listen("day-changed", () => trackActive());
    const unAuto = listen<{ kind: "day" | "week" | "month"; duration_ms: number; writer: Provider; error: string | null }>("auto-summary-done", (e) => {
      const { kind, duration_ms, writer, error } = e.payload;
      trackSummary({ kind, auto: true, duration_ms, refresh: false, writer, error });
    });
    const unOpened = listen<{ kind: "day" | "week" | "month"; target: "obsidian" | "app" }>("notification-opened", (e) =>
      track("notification_opened", e.payload),
    );
    return () => {
      unDay.then((f) => f());
      unAuto.then((f) => f());
      unOpened.then((f) => f());
    };
  }, []);

  useEffect(() => {
    api.environment().then(setEnv);
    api.today().then((t) => {
      setToday(t.date);
    });
    // The tray menu asks the window to show a screen.
    const un = listen<string>("navigate", (e) => {
      const to = screenFrom(e.payload);
      if (to.open === "today") jumpTo(null);
      goRef.current(to.screen);
    });
    // A clicked summary notification asks for that report.
    const unReport = listen<{ kind: "day" | "week" | "month"; key: string }>("open-report", (e) => {
      const { kind, key } = e.payload;
      jumpTo({ kind, date: kind === "month" ? `${key}-01` : key });
      goRef.current("summary");
    });
    // ⌘1–5 switch screens.
    const onKey = (e: KeyboardEvent) => {
      if (!e.metaKey || e.shiftKey || e.altKey) return;
      const hit = NAV.find((n) => n.key === e.key && !n.soon);
      if (hit) {
        e.preventDefault();
        goRef.current(hit.id);
      } else if (e.key === ",") {
        e.preventDefault();
        goRef.current("settings");
      }
    };
    window.addEventListener("keydown", onKey);
    return () => {
      un.then((f) => f());
      unReport.then((f) => f());
      window.removeEventListener("keydown", onKey);
    };
  }, []);

  const item = (n: NavItem) => (
    <button
      key={n.id}
      type="button"
      disabled={n.soon}
      onClick={() => go(n.id)}
      aria-current={screen === n.id ? "page" : undefined}
      className={
        "flex h-8 w-full items-center gap-2.5 rounded-md px-2.5 text-left motion-safe:transition-colors motion-safe:duration-fast motion-safe:ease-calm disabled:opacity-40 " +
        (screen === n.id ? "bg-fg/10 text-fg" : "text-muted hover:bg-fg/5 hover:text-fg")
      }
    >
      <Icon name={n.icon} size={15} />
      <span>{t.nav[n.label]}</span>
      {jobs.some((j) => screenOf(j) === n.id) ? (
        <span className="ml-auto flex items-center gap-1.5 text-[10.5px] text-muted" title={t.nav.making}>
          <Spinner size={10} />
          {t.nav.making}
        </span>
      ) : n.id === "status" && waiting > 0 ? (
        <span className="ml-auto rounded-full bg-fg/15 px-1.5 text-[10.5px] font-medium text-fg" title={t.nav.waiting}>
          {waiting}
        </span>
      ) : n.id === "insight" && insightNew ? (
        <span className="ml-auto h-1.5 w-1.5 rounded-full bg-accent" role="img" aria-label={t.nav.insightNew} title={t.nav.insightNew} />
      ) : (
        <span className={"ml-auto text-[10.5px] text-faint " + (n.soon ? "" : "font-mono")}>{n.soon ? t.nav.soon : n.key ? `⌘${n.key}` : ""}</span>
      )}
    </button>
  );

  if (env && !env.onboarded) {
    return <Onboarding env={env} onDone={() => api.environment().then(setEnv)} />;
  }

  return (
    <EnvContext.Provider value={env}>
    <div className="flex h-full bg-canvas print:block print:h-auto">
      <nav className="print:hidden flex w-52 shrink-0 flex-col border-r border-line-soft bg-surface px-2.5 pb-3 pt-11" data-tauri-drag-region>
        <div className="mb-5 px-2.5 text-[13px] font-medium tracking-tight" data-tauri-drag-region>
          porch<span className="text-accent">.</span>
        </div>
        <div className="space-y-px">{NAV.map(item)}</div>
        <div className="mt-auto space-y-px">
          {update.status !== "idle" && !update.open && (
            <button
              type="button"
              onClick={showUpdate}
              className="flex h-8 w-full items-center gap-2.5 rounded-md px-2.5 text-left text-fg hover:bg-fg/5"
            >
              <span aria-hidden className="ml-[3px] h-1.5 w-1.5 rounded-full bg-accent" />
              <span className="ml-[3px]">{update.status === "failed" ? t.nav.updateFailed : t.nav.update}</span>
              <span className="ml-auto font-mono text-[10.5px] text-faint">v{update.version}</span>
            </button>
          )}
          {item({ id: "settings", label: "settings", icon: "settings", key: "," })}
        </div>
      </nav>
      <main className="min-w-0 flex-1 overflow-y-auto print:overflow-visible">
        <div className="sticky top-0 z-10 h-9 bg-canvas/80 backdrop-blur print:hidden" data-tauri-drag-region />
        <div className="mx-auto max-w-[1040px] px-10 pb-16 print:max-w-none print:px-0 print:pb-0">
          {screen === "summary" && today && <Summary today={today} open={summaryOpen} onChange={setSummaryOpen} onOpenInsight={(week: string) => { keep("insight.date", week); go("insight"); }} />}
          {screen === "insight" && today && (
            <InsightScreen today={today}
              onOpenDay={(d) => { jumpTo({ kind: "day", date: d }); go("summary"); }}
              onOpenProjects={() => go("projects")}
              onChangeAgent={() => { setSettingsAt({ tab: "general", focus: "summary", at: Date.now() }); setScreen("settings"); }} />
          )}
          {screen === "status" && <Now />}
          {screen === "usage" && today && (
            <Usage
              today={today}
              onOpenDay={(date) => {
                jumpTo({ kind: "day", date });
                setScreen("summary");
              }}
            />
          )}
          {screen === "projects" && (
            <Projects
              onOpenDay={(date) => {
                jumpTo({ kind: "day", date });
                setScreen("summary");
              }}
            />
          )}
          {screen === "settings" && <Settings key={settingsAt ? `${settingsAt.tab}:${settingsAt.at}` : "default"} tab={settingsAt?.tab} focus={settingsAt?.focus} />}
        </div>
      </main>
      <UpdateModal />
      <NoticeStack />
    </div>
    </EnvContext.Provider>
  );
}
