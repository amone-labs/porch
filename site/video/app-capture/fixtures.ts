// Demo data for the capture harness: one invented day (2026-09-29, Tue) across three projects.
// Every value is made up; shapes follow app/src/api.ts. Numbers match site/src/mock.ts where they overlap
// (5h 12m of work, "Add login" 74 min / 24%, "Order alert fix" 52 min / 17%).
import type {
  AccountLimits,
  Board,
  DayReport,
  Entry,
  Environment,
  Listing,
  OpenItem,
  ProjectDay,
  ProjectHealth,
  ProjectOverview,
  Suggestion,
  SettingsView,
  Tokens,
  Turn,
  Usage,
  UsageReport,
} from "../../../app/src/api";

const DAY = "2026-09-29";
const MIDNIGHT = new Date(`${DAY}T00:00:00+09:00`).getTime();
// Minutes past the hour may overflow (11:30 + 34 min); add them instead of formatting a clock string.
const at = (h: number, m = 0) => MIDNIGHT + (h * 60 + m) * 60_000;
const NOW = at(17, 40);

const tokens = (input: number, output: number, cacheRead: number): Tokens => ({
  input,
  output,
  cache_read: cacheRead,
  cache_write_5m: Math.round(input * 0.4),
  cache_write_1h: 0,
});
const usage = (cost: number, t: Tokens, byHour: number[] = []): Usage => ({
  tokens: t,
  cost,
  unpriced: 0,
  by_model: { "claude-opus-5-5": { tokens: t, cost: cost * 0.8 }, "gpt-5.6-codex": { tokens: t, cost: cost * 0.2 } },
  by_hour: byHour,
});

let turnId = 0;
const turn = (h: number, m: number, minutes: number, prompt: string, extra: Partial<Turn> = {}): Turn => ({
  id: `t${++turnId}`,
  start: at(h, m),
  end: at(h, m + minutes),
  active_ms: minutes * 60_000,
  prompt,
  reply: null,
  tool_calls: 6,
  tool_errors: 0,
  denials: 0,
  interrupted: false,
  errors: [],
  cost: minutes * 0.11,
  model: "claude-opus-5-5",
  ...extra,
});

const shopWeb = [
  turn(9, 10, 22, "로그인 화면에 이메일 인증 추가해 줘"),
  turn(9, 40, 18, "인증 메일 템플릿을 정리해 줘"),
  turn(11, 30, 34, "로그인 실패 시 잠금 정책 넣어 줘"),
  turn(14, 0, 20, "결제 화면 버튼 문구 정리"),
];
const shopApi = [
  turn(10, 10, 26, "주문 알림이 두 번 가는 원인 찾아 줘", { tool_errors: 5, errors: ["TypeError: Cannot read properties of undefined (reading 'orderId')"] }),
  turn(10, 50, 26, "같은 오류가 또 나. 재시도 로직 봐 줘", { tool_errors: 7, errors: ["TypeError: Cannot read properties of undefined (reading 'orderId')"] }),
  turn(15, 0, 30, "알림 큐 테스트 추가"),
];
const porchSite = [turn(13, 0, 24, "랜딩 페이지 히어로 문구 다듬기"), turn(16, 0, 40, "요약 화면 스크린샷 업데이트")];

const day: DayReport = {
  date: DAY,
  generated_at: at(18, 2),
  model: "claude-opus-5-5",
  usual_minutes: 268,
  digest: {
    date: DAY,
    metrics: {
      sessions: 7,
      prompts: 41,
      active_minutes: 312,
      tool_calls: 238,
      tool_errors: 14,
      denials: 2,
      files_touched: 37,
      commits: 9,
      my_commits: 9,
      insertions: 612,
      deletions: 188,
      by_hour: [0, 0, 0, 0, 0, 0, 0, 0, 0, 40, 52, 38, 0, 36, 44, 46, 56, 0, 0, 0, 0, 0, 0, 0],
      usage: usage(31.4, tokens(1_820_000, 410_000, 21_400_000)),
    },
    projects: [
      {
        name: "shop-web",
        root: "~/work/shop-web",
        commits: [
          { hash: "a1c9e02", t: at(10, 5), subject: "feat(auth): email verification on sign-in", insertions: 210, deletions: 34, mine: true },
          { hash: "b77d1f4", t: at(12, 10), subject: "feat(auth): lock after five failed attempts", insertions: 96, deletions: 12, mine: true },
          { hash: "c03aa91", t: at(14, 25), subject: "copy(checkout): button labels", insertions: 18, deletions: 18, mine: true },
        ],
        metrics: {
          sessions: 3, prompts: 17, active_minutes: 138, tool_calls: 96, tool_errors: 2, denials: 1, files_touched: 15,
          commits: 3, my_commits: 3, insertions: 324, deletions: 64, by_hour: [],
          usage: usage(14.2, tokens(820_000, 180_000, 9_600_000)),
        },
        sessions: [{ agent: "claude", session: "s-web-1", title: "로그인 기능 추가", branch: "feat/email-auth", first_at: at(9, 10), last_at: at(14, 20), turns: shopWeb }],
      },
      {
        name: "shop-api",
        root: "~/work/shop-api",
        commits: [
          { hash: "d4e2b10", t: at(11, 20), subject: "fix(alerts): dedupe order notifications", insertions: 74, deletions: 41, mine: true },
          { hash: "e91f3c7", t: at(15, 35), subject: "test(alerts): retry queue", insertions: 120, deletions: 6, mine: true },
        ],
        metrics: {
          sessions: 2, prompts: 14, active_minutes: 106, tool_calls: 88, tool_errors: 12, denials: 1, files_touched: 11,
          commits: 2, my_commits: 2, insertions: 194, deletions: 47, by_hour: [],
          usage: usage(10.1, tokens(610_000, 140_000, 7_300_000)),
        },
        sessions: [{ agent: "codex", session: "s-api-1", title: "주문 알림 오류 수정", branch: "fix/alert-dupes", first_at: at(10, 10), last_at: at(15, 30), turns: shopApi }],
      },
      {
        name: "porch-site",
        root: "~/work/porch-site",
        commits: [
          { hash: "f2a8d55", t: at(13, 30), subject: "copy(hero): shorter headline", insertions: 12, deletions: 9, mine: true },
          { hash: "0b6c4e1", t: at(16, 45), subject: "chore: refresh screenshots", insertions: 82, deletions: 68, mine: true },
        ],
        metrics: {
          sessions: 2, prompts: 10, active_minutes: 68, tool_calls: 54, tool_errors: 0, denials: 0, files_touched: 11,
          commits: 2, my_commits: 2, insertions: 94, deletions: 77, by_hour: [],
          usage: usage(7.1, tokens(390_000, 90_000, 4_500_000)),
        },
        sessions: [{ agent: "claude", session: "s-site-1", title: "랜딩 페이지 정리", branch: "main", first_at: at(13, 0), last_at: at(16, 40), turns: porchSite }],
      },
    ],
  },
  summary: {
    headline: "결제 화면 정리, 주문 알림 오류 수정",
    projects: [
      {
        name: "shop-web",
        summary: "로그인에 이메일 인증과 실패 잠금을 넣고, 결제 화면 버튼 문구를 정리했습니다.",
        done: ["이메일 인증 추가", "다섯 번 실패하면 잠금", "결제 버튼 문구 정리"],
        in_progress: ["환불 버튼 문구 정하기"],
        next: ["인증 메일 다국어"],
      },
      {
        name: "shop-api",
        summary: "주문 알림이 두 번 가던 원인을 재시도 로직에서 찾아 막고, 알림 큐 테스트를 추가했습니다.",
        done: ["중복 알림 차단", "재시도 큐 테스트"],
        in_progress: [],
        next: ["알림 지연 측정"],
      },
      {
        name: "porch-site",
        summary: "히어로 문구를 줄이고 요약 화면 스크린샷을 새로 찍었습니다.",
        done: ["히어로 문구 정리", "스크린샷 갱신"],
        in_progress: [],
        next: [],
      },
    ],
    analysis: {
      observations: ["같은 오류를 두 요청에 걸쳐 12번 만났습니다."],
      suggestions: ["알림 핸들러에 orderId가 없을 때를 먼저 확인하는 테스트를 두면 같은 오류를 줄일 수 있습니다."],
    },
    time: [
      { label: "로그인 기능 추가", project: "shop-web", turns: ["t1", "t2", "t3"], note: "이메일 인증과 실패 잠금", minutes: 74 },
      { label: "주문 알림 오류 수정", project: "shop-api", turns: ["t5", "t6"], note: "중복 알림 원인 추적", minutes: 52 },
      { label: "알림 큐 테스트", project: "shop-api", turns: ["t7"], note: "재시도 큐", minutes: 48 },
      { label: "랜딩 페이지 정리", project: "porch-site", turns: ["t8", "t9"], note: "문구와 스크린샷", minutes: 92 },
      { label: "결제 화면 정리", project: "shop-web", turns: ["t4"], note: "버튼 문구", minutes: 46 },
    ],
    blockers: [
      {
        title: "주문 알림이 두 번 감",
        project: "shop-api",
        turns: ["t5", "t6"],
        signal: "같은 오류 12번, 같은 요청 3번",
        cause: "재시도할 때 orderId 없이 핸들러를 다시 불렀습니다.",
        resolved: true,
        fix: "재시도 전에 orderId를 확인하는 테스트를 먼저 둡니다.",
        minutes: 52,
      },
    ],
    visuals: [],
  },
  summary_error: null,
};

const entry = (e: Partial<Entry> & Pick<Entry, "session" | "state" | "title">): Entry => ({
  agent: "claude",
  pending: null,
  subagents: [],
  last_event_at: NOW - 60_000,
  cwd: null,
  log: null,
  term_program: null,
  term_pane: null,
  focusable: false,
  alive: true,
  stale: false,
  state_since: NOW - 60_000,
  turn_started_at: NOW - 240_000,
  turn_errors: 0,
  last_prompt: null,
  ask: null,
  model: "claude-opus-5-5",
  context_percent: 46,
  context_tokens: null,
  ...e,
});

const board: Board = {
  now: NOW,
  worktrees: [
    {
      root: "~/work/shop-web",
      branch: "feat/refund-copy",
      dirty: 3,
      entries: [
        entry({
          session: "s-web-2",
          state: "permission",
          title: "결제 화면 정리",
          pending: { tool_use: "tu1", tool: "Bash" },
          ask: "Bash · pnpm test checkout",
          last_prompt: "환불 버튼 문구 정하고 테스트 돌려 줘",
          state_since: NOW - 4 * 60_000,
          context_percent: 62,
        }),
      ],
    },
    {
      root: "~/work/shop-api",
      branch: "fix/alert-latency",
      dirty: 1,
      entries: [
        entry({
          agent: "codex",
          session: "s-api-2",
          state: "turn_done",
          title: "주문 알림 오류 수정",
          last_prompt: "알림 지연 측정 스크립트 만들어 줘",
          last_event_at: NOW - 12 * 60_000,
          state_since: NOW - 12 * 60_000,
          model: "gpt-5.6-codex",
          context_percent: 38,
        }),
      ],
    },
    {
      root: "~/work/porch-site",
      branch: "main",
      dirty: 0,
      entries: [
        entry({
          session: "s-site-2",
          state: "running",
          title: "로그인 기능 추가",
          last_prompt: "인증 메일 다국어 초안",
          last_event_at: NOW - 2 * 60_000,
          state_since: NOW - 2 * 60_000,
          context_percent: 84,
        }),
      ],
    },
  ],
};

const days7 = ["2026-09-23", "2026-09-24", "2026-09-25", "2026-09-26", "2026-09-27", "2026-09-28", DAY];
const costs7 = [18.2, 24.6, 21.3, 6.4, 3.1, 27.5, 31.4];
const usageReport: UsageReport = {
  end: DAY,
  days: days7.map((date, i) => ({ date, usage: usage(costs7[i], tokens(900_000 + i * 120_000, 200_000, 9_000_000)), minutes: [210, 260, 240, 60, 20, 290, 312][i] })),
  total: usage(142.5, tokens(9_800_000, 2_100_000, 26_300_000)),
  active_minutes: 1392,
  by_project: [
    { name: "shop-web", usage: usage(61.2, tokens(4_100_000, 900_000, 11_000_000)), series_cost: [8, 11, 9, 2, 1, 13, 14.2], series_tokens: [] },
    { name: "shop-api", usage: usage(49.8, tokens(3_300_000, 700_000, 9_100_000)), series_cost: [6, 9, 8, 3, 1, 10, 10.1], series_tokens: [] },
    { name: "porch-site", usage: usage(31.5, tokens(2_400_000, 500_000, 6_200_000)), series_cost: [4, 4, 4, 1, 1, 4, 7.1], series_tokens: [] },
  ],
  by_model: [
    { name: "claude-opus-5-5", usage: usage(113.9, tokens(7_800_000, 1_700_000, 21_000_000)), series_cost: [], series_tokens: [] },
    { name: "gpt-5.6-codex", usage: usage(28.6, tokens(2_000_000, 400_000, 5_300_000)), series_cost: [], series_tokens: [] },
  ],
  by_agent: [
    { name: "claude", usage: usage(113.9, tokens(7_800_000, 1_700_000, 21_000_000)), series_cost: [], series_tokens: [] },
    { name: "codex", usage: usage(28.6, tokens(2_000_000, 400_000, 5_300_000)), series_cost: [], series_tokens: [] },
  ],
  unknown_days: [],
  month_cost: 486.2,
  month_tokens: 128_000_000,
  monthly_budget: 800,
  prices_as_of: "2026-09-01",
};

const limits: AccountLimits[] = [
  { agent: "claude", label: "Claude Max", five_hour: { used_percent: 64, resets_at: at(19, 40) }, weekly: { used_percent: 41, resets_at: new Date("2026-10-01T09:00:00+09:00").getTime() }, spend: null, at: NOW },
  { agent: "codex", label: "Codex", five_hour: { used_percent: 81, resets_at: at(20, 10) }, weekly: { used_percent: 41, resets_at: new Date("2026-10-02T09:00:00+09:00").getTime() }, spend: null, at: NOW },
];

const settings: SettingsView = {
  settings: {
    // Claude Code writes the summaries (ADR 0010 default), so the summary screens read as before.
    provider: "claude",
    model: "sonnet",
    codex_model: null,
    auto_summary_at: "18:00",
    onboarded: true,
    monthly_budget: 800,
    show_cost: true,
    analytics: false,
    // Mirror and notifications stay off; no shot screen shows them.
    mirror_dir: null,
    obsidian_vault: null,
    mirror_daily: "daily",
    mirror_weekly: "weekly",
    mirror_monthly: "monthly",
    notify: false,
    notify_open: "porch",
  },
  models: ["sonnet", "opus", "haiku"],
  excluded: [],
  agents: [
    { agent: "claude", present: true, targets: [{ agent: "claude", file: "~/.claude/settings.json", state: "installed", detail: null }] },
    { agent: "codex", present: true, targets: [{ agent: "codex", file: "~/.codex/hooks.json", state: "installed", detail: null }] },
  ],
  hook_binary: "/Applications/Porch.app/Contents/MacOS/porch-hook",
  hook_binary_present: true,
  statusline: "on",
  statusline_had_original: false,
  statusline_show_line: true,
  statusline_projects: [],
  mirror: { dir: null, vault: null, vault_missing: false, last_error: null, notes: 0, last_saved: null, samples: [], elsewhere: [] },
};

const environment: Environment = { claude_bin: "/usr/local/bin/claude", codex_bin: "/usr/local/bin/codex", claude_data: true, codex_homes: ["~/.codex"], hooks_installed: true, onboarded: true };

const listing: Listing = {
  days: days7.map((date, i) => ({ date, headline: i === 6 ? day.summary!.headline : null, minutes: [210, 260, 240, 60, 20, 290, 312][i], commits: [6, 8, 7, 1, 0, 8, 9][i], waiting: i === 6 ? 1 : 0, issues: 0 })),
  weeks: [],
};

const openItems: OpenItem[] = [
  { id: "o1", project: "shop-web", text: "환불 버튼 문구 정하기", kind: "waiting", since: DAY, closed_on: null, closed_how: null },
  { id: "o2", project: "shop-api", text: "주문 알림이 두 번 갈 때가 있음", kind: "issue", since: "2026-09-28", closed_on: DAY, closed_how: "끝남" },
];

// --- projects, 30-day health and suggestions (ADR 0007) ---
const daysAgo = (n: number) => {
  const d = new Date(`${DAY}T12:00:00+09:00`);
  d.setDate(d.getDate() - n);
  return d.toISOString().slice(0, 10);
};
const ev = (n: number, label: string) => ({ date: daysAgo(n), turns: [`t${n}`], label });

const projects: ProjectOverview[] = [
  { name: "shop-api", root: "~/work/shop-api", days: 21, minutes: 2310, commits: 48, first_date: daysAgo(29), last_date: DAY, latest_summary: "주문 알림이 두 번 가던 원인을 재시도 로직에서 찾아 막고, 알림 큐 테스트를 추가했습니다." },
  { name: "shop-web", root: "~/work/shop-web", days: 24, minutes: 2760, commits: 61, first_date: daysAgo(29), last_date: DAY, latest_summary: "로그인에 이메일 인증과 실패 잠금을 넣고, 결제 화면 버튼 문구를 정리했습니다." },
  { name: "porch-site", root: "~/work/porch-site", days: 9, minutes: 640, commits: 17, first_date: daysAgo(20), last_date: DAY, latest_summary: "히어로 문구를 줄이고 요약 화면 스크린샷을 새로 찍었습니다." },
];

const apiHealth: ProjectHealth = {
  root: "~/work/shop-api",
  name: "shop-api",
  days: 30,
  active_minutes: 2310,
  blocked_minutes: 402,
  kinds: [
    { kind: "test_fail", label: "테스트 실패", blockers: 7, minutes: 248, unresolved_that_day: 2, days: 6,
      evidence: [ev(0, "주문 알림이 두 번 감"), ev(3, "재시도 큐 테스트가 간헐적으로 실패"), ev(6, "알림 큐 순서 테스트 실패"), ev(11, "주문 알림이 두 번 감"), ev(15, "재시도 큐 테스트가 간헐적으로 실패"), ev(22, "주문 상태 전이 테스트 실패")] },
    { kind: "env", label: "작업 환경", blockers: 3, minutes: 96, unresolved_that_day: 0, days: 3,
      evidence: [ev(4, "로컬 Redis가 안 뜸"), ev(13, "Node 버전 불일치"), ev(25, "로컬 Redis가 안 뜸")] },
    { kind: "auth_external", label: "인증·외부 서비스", blockers: 2, minutes: 58, unresolved_that_day: 1, days: 2,
      evidence: [ev(8, "결제 대행사 샌드박스 401"), ev(19, "결제 대행사 샌드박스 401")] },
  ],
  repeated_errors: [{ command: "pnpm test alerts", days: 5, failures: 19, evidence: [ev(0, "pnpm test alerts"), ev(3, "pnpm test alerts"), ev(11, "pnpm test alerts")] }],
  interrupted: 4,
  denials: 3,
  open_issues: [{ id: "o9", text: "결제 대행사 샌드박스 인증이 가끔 실패", since: daysAgo(19), days_open: 19 }],
  checks: [
    { kind: "repeat_kind", title: "테스트 실패: 6일 막힘, 248분 (이전 30일 9일)", now: 6, before: 9, evidence: [ev(0, "주문 알림이 두 번 감"), ev(3, "재시도 큐 테스트가 간헐적으로 실패")] },
    { kind: "repeat_error", title: "pnpm test alerts 5일 실패", now: 5, before: null, evidence: [ev(0, "pnpm test alerts")] },
    { kind: "blocked_share", title: "작업 시간의 17.4%가 막힘에 걸린 요청 (이전 30일 24.1%)", now: 174, before: 241, evidence: [] },
    { kind: "old_issue", title: "결제 대행사 샌드박스 인증이 가끔 실패: 풀렸다는 기록이 없음 (" + daysAgo(19) + "부터, 19일)", now: 19, before: null, evidence: [ev(19, "결제 대행사 샌드박스 401")] },
  ],
  previous: { active_minutes: 2180, blocked_minutes: 526, blockers: 15 },
  session_attributed_days: 21,
  issues_tracked_since: daysAgo(29),
};

const suggestions: Suggestion[] = [
  {
    id: "sg1", created: daysAgo(14), scope: "~/work/shop-api",
    title: "재시도 전에 orderId를 확인하는 테스트부터",
    why: "같은 테스트 실패가 30일 중 9일 있었고, 대부분 재시도할 때 orderId가 빠진 경우였습니다.",
    action: "claude_md",
    text: "- 알림·결제 핸들러를 고칠 때는 orderId가 없는 경우의 테스트를 먼저 추가하고, 그 테스트가 실패하는 것을 확인한 뒤 고친다.",
    target: "kind:test_fail", target_label: "테스트 실패",
    evidence: [daysAgo(15), daysAgo(22), daysAgo(25)],
    status: "applied", applied_on: daysAgo(14),
    baseline: { items: 6, days: 4 },
    effect: { days: 14, before: { items: 6, days: 4 }, after: { items: 2, days: 2 }, state: "measured" },
  },
  {
    id: "sg2", created: daysAgo(1), scope: "~/work/shop-api",
    title: "로컬 Redis를 테스트 전에 띄우는 훅",
    why: "작업 환경 막힘 3번 중 2번이 로컬 Redis가 꺼져 있던 경우였습니다.",
    action: "hook",
    text: '{ "hooks": { "PreToolUse": [{ "matcher": "Bash(pnpm test*)", "hooks": [{ "type": "command", "command": "redis-cli ping || brew services start redis" }] }] } }',
    target: "kind:env", target_label: "작업 환경",
    evidence: [daysAgo(4), daysAgo(25)],
    status: "new", applied_on: null,
    baseline: { items: 3, days: 3 },
    effect: null,
  },
  {
    id: "sg3", created: daysAgo(1), scope: "common",
    title: "테스트 명령은 묻지 않고 실행",
    why: "세 프로젝트에서 pnpm test 허락 요청이 30일 동안 41번 있었고, 모두 허락했습니다.",
    action: "permission",
    text: '"allow": ["Bash(pnpm test:*)", "Bash(pnpm lint)"]',
    target: "command:pnpm test", target_label: "pnpm test 실패",
    evidence: [daysAgo(2), daysAgo(5)],
    status: "new", applied_on: null,
    baseline: { items: 41, days: 17 },
    effect: null,
  },
];

const projectDays: ProjectDay[] = [
  { date: DAY, minutes: 106, sessions: 2, commits: ["fix(alerts): dedupe order notifications", "test(alerts): retry queue"], summary: "주문 알림이 두 번 가던 원인을 재시도 로직에서 찾아 막고, 알림 큐 테스트를 추가했습니다.", done: ["중복 알림 차단", "재시도 큐 테스트"], next: ["알림 지연 측정"] },
  { date: daysAgo(1), minutes: 132, sessions: 2, commits: ["feat(alerts): retry with backoff"], summary: "알림 재시도에 지수 백오프를 넣었습니다.", done: ["재시도 백오프"], next: [] },
];

const waitingCount = board.worktrees.flatMap((w) => w.entries).filter((e) => ["permission", "question", "failed"].includes(e.state)).length;

export const fixtures: Record<string, (args: unknown) => unknown> = {
  board_now: () => board,
  waiting: () => waitingCount,
  environment: () => environment,
  today: () => ({ date: DAY, week_start: "2026-09-28" }),
  day_report: () => day,
  list_reports: () => listing,
  open_items: () => openItems,
  usage_report: (args) => {
    const days = (args as { days?: number } | undefined)?.days ?? 7;
    if (days > 1) return usageReport;
    const today = usageReport.days[usageReport.days.length - 1];
    const byHour = [0, 0, 0, 0, 0, 0, 0, 0, 0, 3.8, 4.6, 3.1, 0.4, 3.3, 4.2, 4.4, 5.9, 1.7, 0, 0, 0, 0, 0, 0];
    // One day: every breakdown is that day's share, so the parts add up to the day's $31.40.
    const share = (g: (typeof usageReport.by_project)[number], cost: number) => ({
      ...g, usage: usage(cost, tokens(Math.round(g.usage.tokens.input * cost / g.usage.cost!), Math.round(g.usage.tokens.output * cost / g.usage.cost!), Math.round(g.usage.tokens.cache_read * cost / g.usage.cost!))),
      series_cost: [cost], series_tokens: [],
    });
    const total = today.usage.cost!;
    return {
      ...usageReport, days: [today], total: { ...today.usage, by_hour: byHour }, active_minutes: today.minutes,
      by_project: usageReport.by_project.map((g) => share(g, g.series_cost[g.series_cost.length - 1])),
      by_model: usageReport.by_model.map((g, i) => share(g, i === 0 ? +(total * 0.8).toFixed(2) : +(total * 0.2).toFixed(2))),
      by_agent: usageReport.by_agent.map((g, i) => share(g, i === 0 ? +(total * 0.8).toFixed(2) : +(total * 0.2).toFixed(2))),
    };
  },
  usage_limits: () => limits,
  get_settings: () => settings,
  projects: () => projects,
  projects_health: () => [apiHealth],
  suggestions: () => suggestions,
  project_days: () => projectDays,
  last_events: () => [{ agent: "claude", t: NOW - 60_000 }, { agent: "codex", t: NOW - 12 * 60_000 }],
  "plugin:event|listen": () => 1,
  "plugin:event|unlisten": () => null,
  "plugin:webview|set_webview_zoom": () => null,
};
