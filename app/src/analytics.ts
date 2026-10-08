// App usage events, exactly the table in docs/decisions/0006-usage-analytics.md.
// Nothing else goes out: no paths, projects, branches, prompts, summaries,
// tokens or cost. A build without VITE_POSTHOG_KEY, or the setting turned off,
// sends nothing. Only the main window calls this; the popover never does.
import posthog from "posthog-js";
import { getVersion } from "@tauri-apps/api/app";
import { isEnabled } from "@tauri-apps/plugin-autostart";
import { api, worst, type AgentView, type Provider } from "./api";
import type { SettingsTab } from "./screens/Settings";

const KEY = import.meta.env.VITE_POSTHOG_KEY;

type Kind = "day" | "week" | "month";
type Hooks = "installed" | "partial" | "missing" | "error" | "absent";
type Reason = "limit" | "login" | "missing" | "bad_answer" | "other";

type Events = {
  app_opened: { version: string };
  app_active: {
    hooks_claude: Hooks;
    hooks_codex: Hooks;
    writer: Provider;
    auto_summary: boolean;
    notify: boolean;
    mirror: "none" | "folder" | "obsidian";
    autostart: boolean;
    cli_link: boolean;
    statusline: boolean;
  };
  screen_viewed: { screen: "onboarding" | "summary" | "insight" | "status" | "usage" | "projects" | "settings" };
  tab_viewed: { screen: "summary"; tab: Kind } | { screen: "settings"; tab: SettingsTab };
  session_focused: { ok: boolean };
  summary_created: { kind: Kind; auto: boolean; ok: boolean; duration_ms: number; writer: Provider; refresh: boolean; reason?: Reason };
  onboarding_finished: { hooks: boolean; autostart: boolean };
  hooks_installed: { agent: "claude" | "codex" | "all"; from: "onboarding" | "settings" };
  notification_opened: { kind: Kind; target: "obsidian" | "app" };
  suggestions_made: { ok: boolean; duration_ms: number; writer: Provider; added?: number; dropped?: number; reason?: Reason };
  suggestion_status: { status: "applied" | "dismissed" | "new" };
  suggestion_copied: Record<string, never>;
  report_exported: { kind: Kind; format: "md" | "pdf"; ok: boolean };
  mirror_connected: { target: "folder" | "obsidian" };
  mirror_disconnected: Record<string, never>;
};

let started = false;
let enabled = false;

function start() {
  if (started || !KEY) return;
  started = true;
  posthog.init(KEY, {
    api_host: "https://us.i.posthog.com",
    persistence: "localStorage",
    person_profiles: "identified_only",
    autocapture: false,
    capture_pageview: false,
    capture_pageleave: false,
    capture_performance: false,
    capture_exceptions: false,
    capture_heatmaps: false,
    capture_dead_clicks: false,
    disable_session_recording: true,
    disable_surveys: true,
    disable_web_experiments: true,
    advanced_disable_feature_flags: true,
    advanced_disable_flags: true,
    disable_external_dependency_loading: true,
    save_referrer: false,
    // The webview's URL and referrer say nothing about use; keep them home.
    property_denylist: ["$current_url", "$pathname", "$host", "$referrer", "$referring_domain", "$initial_current_url", "$initial_pathname", "$initial_referrer", "$initial_referring_domain"],
  });
}

/** Turn sending on or off at once, as the setting says. */
export function setAnalytics(on: boolean) {
  enabled = on && !!KEY;
  if (enabled) {
    start();
    posthog.opt_in_capturing({ captureEventName: false });
  } else if (started) {
    posthog.opt_out_capturing();
  }
}

// Events raised before the setting is read wait for it rather than slip out.
let ready: Promise<void> | null = null;
function load(): Promise<void> {
  ready ??= api.settings().then(
    (s) => setAnalytics(s.settings.analytics),
    () => setAnalytics(false),
  );
  return ready;
}

export function track<E extends keyof Events>(event: E, props: Events[E]) {
  if (!KEY) return;
  load().then(() => {
    if (enabled) posthog.capture(event, props);
  });
}

export function trackAppOpened() {
  if (!KEY) return;
  getVersion().then((version) => track("app_opened", { version }), () => {});
}

/** Which features are on: at launch, and again each day the app stays up. */
export function trackActive() {
  if (!KEY) return;
  Promise.all([api.settings(), isEnabled().catch(() => false)]).then(([v, autostart]) => {
    const hooks = (agent: AgentView["agent"]): Hooks => {
      const a = v.agents.find((x) => x.agent === agent);
      return a?.present ? worst(a.targets) : "absent";
    };
    const s = v.settings;
    track("app_active", {
      hooks_claude: hooks("claude"),
      hooks_codex: hooks("codex"),
      writer: s.provider,
      auto_summary: s.auto_summary_at !== null,
      notify: s.notify,
      mirror: !s.mirror_dir ? "none" : s.obsidian_vault ? "obsidian" : "folder",
      autostart,
      cli_link: v.cli_link === "linked",
      statusline: v.statusline === "on",
    });
  }, () => {});
}

/** The kind of a writer failure, read from how writer.rs words it in either language (its LIMIT, LOGIN, MISSING, BAD_JSON). The words stay here. */
export function failureReason(error: string): Reason {
  if (/사용 한도에 걸렸습니다|hit its usage limit/.test(error)) return "limit";
  if (/로그인되어 있지 않습니다|is not logged in/.test(error)) return "login";
  if (/\((claude|codex)\)(를 찾지 못했습니다| was not found)/.test(error)) return "missing";
  if (/JSON을 읽지 못했습니다|응답을 읽지 못했습니다|응답에 result가 없습니다|답이 비어 있습니다|Could not read the (summary|weekly summary|monthly summary) JSON|Could not read the Claude Code response|The Claude Code response contained no result|The summary agent returned an empty response/.test(error)) return "bad_answer";
  return "other";
}

/** A summary job ended. `writer` is who writes now; the backend names it for automatic ones. */
export function trackSummary(p: { kind: Kind; auto: boolean; duration_ms: number; refresh: boolean; error?: string | null; writer?: Provider }) {
  if (!KEY) return;
  const { error, writer, ...rest } = p;
  const send = (w: Provider) =>
    track("summary_created", { ...rest, writer: w, ok: !error, ...(error ? { reason: failureReason(error) } : {}) });
  if (writer) send(writer);
  else api.settings().then((v) => send(v.settings.provider), () => {});
}

/** Make suggestions ended: how many came and went, or why it failed. */
export function trackSuggestions(p: { duration_ms: number; result?: { added: number; dropped: number }; error?: string }) {
  if (!KEY) return;
  api.settings().then(
    (v) =>
      track("suggestions_made", {
        ok: !p.error,
        duration_ms: p.duration_ms,
        writer: v.settings.provider,
        ...(p.error ? { reason: failureReason(p.error) } : p.result ? { added: p.result.added, dropped: p.result.dropped } : {}),
      }),
    () => {},
  );
}
