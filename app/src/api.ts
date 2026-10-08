// Types mirror the JSON that porch-core serializes (snake_case, as sent).
import { invoke } from "@tauri-apps/api/core";

export type State = "permission" | "question" | "failed" | "turn_done" | "running" | "ended";

export interface Entry {
  agent: string;
  session: string;
  state: State;
  pending: { tool_use: string | null; tool: string | null } | null;
  subagents: string[];
  last_event_at: number;
  cwd: string | null;
  log: string | null;
  term_program: string | null;
  term_pane: string | null;
  focusable: boolean;
  title: string;
  alive: boolean | null;
  stale: boolean;
  /** When the state last changed. */
  state_since: number;
  /** When the current turn's prompt was sent. */
  turn_started_at: number | null;
  /** Tool failures in the current turn. */
  turn_errors: number;
  /** The last thing the person asked, one line. */
  last_prompt: string | null;
  /** What a permission prompt wants to do: "Bash · pnpm test". */
  ask: string | null;
  model: string | null;
  /** How full the context window is, 0..100, when known. */
  context_percent: number | null;
  /** Tokens in the context window when the percentage is not known. */
  context_tokens: number | null;
}

export interface Worktree {
  root: string;
  branch: string | null;
  dirty: number | null;
  entries: Entry[];
}

export interface Board {
  now: number;
  worktrees: Worktree[];
}

export interface Metrics {
  sessions: number;
  prompts: number;
  active_minutes: number;
  tool_calls: number;
  tool_errors: number;
  denials: number;
  files_touched: number;
  commits: number;
  my_commits?: number | null;
  insertions: number;
  deletions: number;
  by_hour: number[];
  /** Absent on days whose transcripts are gone. */
  usage?: Usage | null;
}

export interface Commit {
  repo?: string;
  hash: string;
  t: number;
  subject: string;
  insertions: number;
  deletions: number;
  /** Absent on old reports. */
  mine?: boolean;
  author?: string;
}

/** Tokens by kind; cache writes kept 5 minutes or 1 hour price differently. */
export interface Tokens {
  input: number;
  output: number;
  cache_read: number;
  cache_write_5m: number;
  cache_write_1h: number;
}

/** Tokens and list-price cost (API-equivalent cost). `unpriced` tokens had no known price. */
export interface Usage {
  tokens: Tokens;
  cost: number;
  unpriced: number;
  by_model: Record<string, { tokens: Tokens; cost: number | null }>;
  by_hour: number[];
}

/** A part of the span's usage, with its cost and tokens per day (or per hour for one day). */
export interface UsageGroup {
  name: string;
  usage: Usage;
  series_cost: number[];
  series_tokens: number[];
}

export interface UsageReport {
  /** Last day of the range (YYYY-MM-DD). */
  end: string;
  days: { date: string; usage: Usage; minutes: number }[];
  total: Usage;
  active_minutes: number;
  by_project: UsageGroup[];
  by_model: UsageGroup[];
  by_agent: UsageGroup[];
  unknown_days: string[];
  month_cost: number;
  month_tokens: number;
  monthly_budget: number | null;
  prices_as_of: string;
}

export interface LimitWindow {
  used_percent: number;
  resets_at: number | null;
}

export interface AccountLimits {
  agent: "claude" | "codex";
  label: string;
  five_hour: LimitWindow | null;
  weekly: LimitWindow | null;
  spend: LimitWindow | null;
  at: number;
}

export interface Turn {
  id: string;
  start: number;
  end: number;
  active_ms: number;
  prompt: string;
  reply: string | null;
  tool_calls: number;
  tool_errors: number;
  denials: number;
  interrupted: boolean;
  errors: string[];
  tokens?: Tokens;
  cost?: number;
  unpriced?: number;
  model?: string | null;
}

export interface SessionDigest {
  agent: string;
  session: string;
  title: string | null;
  branch: string | null;
  first_at: number;
  last_at: number;
  /** Absent on reports saved before turns were recorded. */
  turns?: Turn[];
}

/** The summary names these; `minutes` is added up from recorded turns, not written by the model. */
export interface TimeBlock {
  label: string;
  project: string;
  turns: string[];
  note: string;
  minutes: number;
}

export interface Blocker {
  title: string;
  project: string;
  turns: string[];
  signal: string;
  cause: string;
  resolved: boolean;
  fix: string;
  minutes: number;
}

export interface Visual {
  title: string;
  mermaid: string;
  caption: string;
}

/** Fields every summary may carry; older ones have none of them. */
export interface Insight {
  time?: TimeBlock[];
  blockers?: Blocker[];
  visuals?: Visual[];
}

export interface ProjectDigest {
  name: string;
  root: string;
  sessions: SessionDigest[];
  commits: Commit[];
  metrics: Metrics;
}

export interface Analysis {
  observations: string[];
  suggestions: string[];
}

export interface ProjectSummary {
  name: string;
  summary: string;
  done: string[];
  in_progress: string[];
  next: string[];
}

export interface DayReport {
  date: string;
  generated_at: number;
  model: string | null;
  /** Who wrote the summary; missing or null on reports from before the choice (Claude Code). */
  provider?: Provider | null;
  digest: { date: string; projects: ProjectDigest[]; metrics: Metrics };
  /** `lang` is the language the summary was written in; missing on old summaries, which are Korean. */
  summary: ({ headline: string; projects: ProjectSummary[]; analysis: Analysis; lang?: "ko" | "en" | null } & Insight) | null;
  summary_error: string | null;
  usual_minutes?: number | null;
}

export interface WeekProject {
  name: string;
  summary: string;
  shipped: string[];
  ongoing: string[];
  next: string[];
}

export interface DayLine {
  date: string;
  minutes: number;
  commits: number;
  my_commits?: number | null;
  cost?: number | null;
  tokens?: number | null;
  headline: string | null;
  projects: Record<string, number>;
}

export interface WeekReport {
  week_start: string;
  generated_at: number;
  model: string | null;
  /** Who wrote the summary; missing or null on reports from before the choice (Claude Code). */
  provider?: Provider | null;
  days: DayLine[];
  /** `lang` is the language the summary was written in; missing on old summaries, which are Korean. */
  summary: ({ headline: string; projects: WeekProject[]; analysis: Analysis; lang?: "ko" | "en" | null } & Insight) | null;
  summary_error: string | null;
}

export interface MonthReport {
  /** "2026-09" */
  month: string;
  generated_at: number;
  model: string | null;
  /** Who wrote the summary; missing or null on reports from before the choice (Claude Code). */
  provider?: Provider | null;
  days: DayLine[];
  summary: WeekReport["summary"];
  summary_error: string | null;
}

export interface Listing {
  /** waiting/issues: items first written down that day and still open. */
  days: { date: string; headline: string | null; minutes: number; commits: number; waiting: number; issues: number }[];
  weeks: { week_start: string; headline: string | null }[];
}

export interface ProjectOverview {
  name: string;
  root: string;
  days: number;
  minutes: number;
  commits: number;
  first_date: string;
  last_date: string;
  latest_summary: string | null;
}

export interface ProjectDay {
  date: string;
  minutes: number;
  sessions: number;
  commits: string[];
  summary: string | null;
  done: string[];
  next: string[];
}

export interface Evidence {
  date: string;
  turns: string[];
  label: string;
}

export interface HealthCheck {
  kind: "repeat_kind" | "repeat_error" | "blocked_share" | "old_issue";
  title: string;
  now: number;
  before: number | null;
  evidence: Evidence[];
}

export interface KindStat {
  kind: string;
  label: string;
  blockers: number;
  minutes: number;
  unresolved_that_day: number;
  days: number;
  evidence: Evidence[];
}

export interface ErrorStat {
  command: string;
  days: number;
  failures: number;
  evidence: Evidence[];
}

export interface OpenIssue {
  id: string;
  text: string;
  since: string;
  days_open: number;
}

/** Last 30 days of one project. Minutes come from recorded turns; kinds from the day summaries. */
export interface ProjectHealth {
  root: string;
  name: string;
  days: number;
  active_minutes: number;
  blocked_minutes: number;
  kinds: KindStat[];
  repeated_errors: ErrorStat[];
  interrupted: number;
  denials: number;
  open_issues: OpenIssue[];
  checks: HealthCheck[];
  previous: { active_minutes: number; blocked_minutes: number; blockers: number } | null;
  session_attributed_days: number;
  issues_tracked_since: string | null;
}

/** How often a suggestion's target showed up: blockers or failures, and on how many days. */
export interface Count {
  items: number;
  days: number;
}

/** Before/after counts of a suggestion's target over equal windows. A comparison, not a cause. */
export interface Effect {
  days: number;
  before: Count;
  after: Count;
  state: "early" | "no_records" | "uncomparable" | "measured";
}

export interface Suggestion {
  id: string;
  created: string;
  /** A repo root, or "common". */
  scope: string;
  title: string;
  why: string;
  action: "claude_md" | "hook" | "permission" | "habit";
  text: string;
  target: string;
  target_label: string;
  evidence: string[];
  status: "new" | "applied" | "dismissed";
  applied_on: string | null;
  baseline: Count;
  effect: Effect | null;
}

export interface HookTarget {
  agent: string;
  file: string;
  state: "installed" | "missing" | "partial" | "error";
  detail: string | null;
}

export interface AgentView {
  agent: "claude" | "codex";
  /** Whether the agent's config dir exists on this Mac. */
  present: boolean;
  targets: HookTarget[];
}

const WORST: HookTarget["state"][] = ["error", "partial", "missing", "installed"];

/** One state for an agent with several hooks files: the worst of them. */
export function worst(targets: HookTarget[]): HookTarget["state"] {
  return WORST.find((s) => targets.some((t) => t.state === s)) ?? "missing";
}

/** Latest hook event time (unix ms) for an agent seen in the last week. */
export interface LastSeen {
  agent: string;
  t: number;
}

export interface SettingsView {
  settings: {
    /** Who writes summaries and suggestions. */
    provider: Provider;
    /** Claude Code model alias. */
    model: string;
    /** Codex model id; null = Codex's default. */
    codex_model: string | null;
    auto_summary_at: string | null;
    onboarded: boolean;
    monthly_budget: number | null;
    show_cost: boolean;
    analytics: boolean;
    /** Whether the weekly insight evaluation runs (ADR 0012). */
    insight_eval: boolean;
    mirror_dir: string | null;
    obsidian_vault: string | null;
    mirror_daily: string;
    mirror_weekly: string;
    mirror_monthly: string;
    notify: boolean;
    notify_open: "porch" | "obsidian";
    language: "system" | "ko" | "en";
  };
  /** The language screens use now: settings.language resolved against macOS. */
  resolved_language: "ko" | "en";
  models: string[];
  excluded: string[];
  agents: AgentView[];
  hook_binary: string;
  hook_binary_present: boolean;
  /** The `porch` command in /usr/local/bin. */
  cli_link: "missing" | "linked" | "stale" | "other";
  cli_link_path: string;
  statusline: "off" | "on" | "disconnected";
  statusline_had_original: boolean;
  statusline_show_line: boolean;
  statusline_projects: { root: string; command: string; wrapped: boolean; disconnected: boolean }[];
  mirror: {
    dir: string | null;
    vault: Vault | null;
    vault_missing: boolean;
    last_error: string | null;
    notes: number;
    last_saved: number | null;
    samples: string[];
    elsewhere: { dir: string; notes: number }[];
  };
}

export interface Vault {
  id: string;
  name: string;
  path: string;
}

export type Provider = "claude" | "codex";

export interface Environment {
  claude_bin: string | null;
  codex_bin: string | null;
  claude_data: boolean;
  codex_homes: string[];
  hooks_installed: boolean;
  onboarded: boolean;
}

/** Work left open across days: Pending (in progress or next) and Blocker (not resolved). */
export interface OpenItem {
  id: string;
  project: string;
  text: string;
  kind: "waiting" | "issue";
  since: string;
  closed_on: string | null;
  /** "done" | "dropped" | "manual" */
  closed_how: string | null;
}

/** "records": numbers only; "generate": saved summary or write one; "refresh": write a new one. */
export type Mode = "records" | "generate" | "refresh";

export interface DayObs {
  date: string;
  running_ms: number;
  turn_done_ms: number;
  question_ms: number;
  failed_ms: number;
  permission_ms: number;
  requests: number;
  stop_failures: number;
  tool_calls: number;
  tool_errors: Record<string, number>;
  permission_requests: number;
}

export interface Observed {
  week_start: string;
  through: number;
  week_end: number;
  days: DayObs[];
  hours_waiting_ms: number[];
  concurrency: { one_ms: number; two_ms: number; three_plus_ms: number; overlap_ms: number };
  projects: { name: string; running_ms: number }[];
  requests: number;
  sessions: number;
  recorded_from: string | null;
}

export interface Checkpoint {
  kind: "day_focus" | "tool_streak" | "hour_band";
  title: string;
  line: string;
  details: string[];
  share: number;
  date: string | null;
  goal_key: string;
  unit: string;
  series: { label: string; value: number; tip: string }[];
  highlight: number[];
}

export type EvalItem = "verify" | "delegate" | "clarity" | "rationale";

export interface Score {
  item: EvalItem;
  score: number;
  turn: string;
  quote: string;
  why: string;
  criterion: string;
  next_criterion: string | null;
}

export interface Pivot {
  turn: string;
  type: "shift" | "revert" | "redo";
  before: string;
  after: string;
  quote: string;
  reason_given: boolean;
  note: string;
}

export interface WeekPivot extends Pivot {
  date: string;
}

export interface Rewrite {
  turn: string;
  date: string;
  quote: string;
  rewritten: string;
  added: string;
  missing: string[];
  note: string;
}

export interface Evaluation {
  evaluated_at: number;
  evaluated_through: number;
  lang: "ko" | "en" | null;
  scores: Score[];
  pivots: WeekPivot[];
  rewrites: Rewrite[];
  pivot_counts: { total: number; with_reason: number; redo: number };
  dropped: number;
}

export interface Goal {
  week_start: string;
  key: string;
  label: string;
  baseline: number;
  unit: string;
  baseline_from: number;
  baseline_to: number;
  set_at: number;
}

export interface GoalResult {
  before: number;
  after: number | null;
  so_far: number | null;
  comparable: boolean;
  reason: "in_progress" | "coverage" | "few_requests" | "no_evaluation" | null;
}

export interface GoalCandidate {
  key: string;
  label: string;
  value: number;
  unit: string;
  kind: "observed" | "score" | "pivots";
}

export interface InsightView {
  observed: Observed;
  checkpoints: Checkpoint[];
  finished: boolean;
  eval_on: boolean;
  has_week_summary: boolean;
  evaluation: Evaluation | null;
  last_scores: Partial<Record<EvalItem, number>>;
  skipped: "off" | "few_requests" | "no_day_summaries" | null;
  error: string | null;
  disputed: string[];
  days_without_summary: string[];
  provider: "claude" | "codex" | null;
  model: string | null;
  goal: Goal | null;
  last_goal: { goal: Goal; result: GoalResult } | null;
  candidates: GoalCandidate[];
}

export const api = {
  board: () => invoke<Board>("board_now"),
  focus: (termProgram: string | null, termPane: string | null) => invoke<void>("focus_session", { termProgram, termPane }),
  today: () => invoke<{ date: string; week_start: string }>("today"),
  day: (date: string, mode: Mode) => invoke<DayReport>("day_report", { date, mode }),
  week: (date: string, mode: Mode) => invoke<WeekReport>("week_report", { date, mode }),
  insight: (date: string) => invoke<InsightView>("insight_week", { date }),
  /** One call to the chosen writer for a finished week (ADR 0012); fails on a running week. */
  makeInsight: (date: string) => invoke<InsightView>("make_insight", { date }),
  setGoal: (week: string, key: string) => invoke<InsightView>("set_goal", { week, key }),
  clearGoal: (week: string) => invoke<InsightView>("clear_goal", { week }),
  insightFeedback: (week: string, item: EvalItem) => invoke<InsightView>("insight_feedback", { week, item }),
  insightLatest: () => invoke<number | null>("insight_latest"),
  saveInsightEval: (on: boolean) => invoke<SettingsView>("save_insight_eval", { on }),
  month: (date: string, mode: Mode) => invoke<MonthReport>("month_report", { date, mode }),
  list: () => invoke<Listing>("list_reports"),
  projects: () => invoke<ProjectOverview[]>("projects"),
  projectDays: (root: string) => invoke<ProjectDay[]>("project_days", { root }),
  backfill: (days: number) => invoke<number>("backfill", { days }),
  projectsHealth: () => invoke<ProjectHealth[]>("projects_health"),
  /** One Claude Code call that gives blockers summarized before kinds existed a kind; returns how many. */
  classifyBlockers: () => invoke<number>("classify_blockers"),
  suggestions: () => invoke<Suggestion[]>("suggestions"),
  /** One call to the chosen writer over the last 30 days; returns how many were added and dropped. */
  makeSuggestions: () => invoke<{ added: number; dropped: number }>("make_suggestions"),
  setSuggestionStatus: (id: string, status: Suggestion["status"]) => invoke<void>("set_suggestion_status", { id, status }),
  settings: () => invoke<SettingsView>("get_settings"),
  /** The language Rust resolved: "ko" or "en". */
  language: () => invoke<"ko" | "en">("get_language"),
  saveLanguage: (language: "system" | "ko" | "en") => invoke<SettingsView>("save_language", { language }),
  saveSettings: (s: { provider: Provider; model: string; codex_model: string | null; auto_summary_at: string | null }) =>
    invoke<SettingsView>("save_settings", { provider: s.provider, model: s.model, codexModel: s.codex_model, autoSummaryAt: s.auto_summary_at }),
  /** Models Codex lists, its default first; empty when Codex cannot say. */
  codexModels: () => invoke<{ slug: string; name: string }[]>("codex_models"),
  saveExcluded: (list: string[]) => invoke<SettingsView>("save_excluded", { list }),
  setCliLink: (on: boolean) => invoke<SettingsView>("set_cli_link", { on }),
  setHooks: (installThem: boolean, agent?: "claude" | "codex") => invoke<SettingsView>("set_hooks", { installThem, agent: agent ?? null }),
  lastEvents: () => invoke<LastSeen[]>("last_events"),
  openMain: (screen?: string) => invoke<void>("open_main", { screen }),
  environment: () => invoke<Environment>("environment"),
  finishOnboarding: () => invoke<void>("finish_onboarding"),
  /** Both write into Downloads and resolve to the file's path. */
  exportMarkdown: (kind: "day" | "week" | "month", date: string) => invoke<string>("export_markdown", { kind, date }),
  exportPdf: (kind: "day" | "week" | "month", date: string) => invoke<string>("export_pdf", { kind, date }),
  reveal: (path: string) => invoke<void>("reveal", { path }),
  openItems: () => invoke<OpenItem[]>("open_items"),
  waiting: () => invoke<number>("waiting"),
  openDownload: () => invoke<void>("open_download"),
  usage: (days: number, end?: string) => invoke<UsageReport>("usage_report", { days, end: end ?? null }),
  limits: () => invoke<AccountLimits[]>("usage_limits"),
  saveUsageSettings: (monthlyBudget: number | null, showCost: boolean) =>
    invoke<SettingsView>("save_usage_settings", { monthlyBudget, showCost }),
  saveAnalytics: (on: boolean) => invoke<SettingsView>("save_analytics", { on }),
  mirrorVaults: () => invoke<{ vaults: Vault[]; note: string | null }>("mirror_vaults"),
  saveMirror: (dir: string | null, vault: string | null, daily: string, weekly: string, monthly: string, notify: boolean) =>
    invoke<SettingsView>("save_mirror", { dir, vault, daily, weekly, monthly, notify }),
  testNotification: () => invoke<void>("test_notification"),
  openNotificationSettings: () => invoke<void>("open_notification_settings"),
  saveNotify: (notify: boolean, open: "porch" | "obsidian") => invoke<SettingsView>("save_notify", { notify, open }),
  openMirrorDir: () => invoke<void>("open_mirror_dir"),
  resaveMirror: () => invoke<SettingsView>("resave_mirror"),
  openVault: () => invoke<void>("open_vault"),
  setStatusline: (on: boolean, showLine: boolean) => invoke<SettingsView>("set_statusline", { on, showLine }),
  /** Empty `roots`: every project listed. */
  setProjectStatusline: (roots: string[], on: boolean) => invoke<SettingsView>("set_project_statusline", { roots, on }),
  closeOpenItem: (id: string) => invoke<OpenItem[]>("close_open_item", { id }),
};
