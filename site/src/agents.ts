import type { Locale } from "./i18n/types";

export type AgentId = "claude-code" | "codex";
export type FeatureId = "permission" | "question" | "failed" | "ended" | "worktime" | "summary" | "tokens" | "limits" | "setup";
export const FEATURES: FeatureId[] = ["permission", "question", "failed", "ended", "worktime", "summary", "tokens", "limits", "setup"];

type Text = { en: string; ko: string };
export interface Agent {
  id: AgentId;
  name: Text;
  features: Record<FeatureId, Text>;
  sources: Record<FeatureId, string>;
}

const SOURCES: Record<FeatureId, string> = {
  permission: "states.md, data-sources.md",
  question: "states.md",
  failed: "states.md",
  ended: "states.md",
  worktime: "README",
  summary: "README, ADR 0010",
  tokens: "README, ui.md",
  limits: "README, ui.md",
  setup: "README, data-sources.md",
};

/** Agents porch supports today. Only add one once it installs and works; never a roadmap item. */
export const AGENTS: Agent[] = [
  {
    id: "claude-code",
    name: { en: "Claude Code", ko: "클로드코드" },
    sources: SOURCES,
    features: {
      permission: { en: "Supported", ko: "됨" },
      question: { en: "Supported", ko: "됨" },
      failed: { en: "Supported", ko: "됨" },
      ended: {
        en: "Checks whether the session's process is still alive. Ended sessions are hidden at once",
        ko: "세션 목록에서 프로세스가 살아 있는지 확인. 끝난 세션은 바로 숨김",
      },
      worktime: { en: "Supported", ko: "됨" },
      summary: { en: "Written by Claude Code when picked in Settings (installed and signed in)", ko: "설정에서 고르면 클로드코드가 씀 (설치·로그인 필요)" },
      tokens: {
        en: "Supported, subagents included. Tokens only for models with unknown prices",
        ko: "됨. 하위 에이전트 포함. 가격을 모르는 모델은 토큰만",
      },
      limits: {
        en: "Needs Pro or Max. Appears after Claude Code's next reply once turned on in Settings. Projects with their own status line need a separate connection",
        ko: "Pro·Max 구독 필요. 설정에서 켠 뒤 클로드코드에서 다음 답이 오면 나타남. 상태 표시줄을 따로 둔 프로젝트는 따로 연결",
      },
      setup: { en: "Adds hooks", ko: "훅 추가" },
    },
  },
  {
    id: "codex",
    name: { en: "Codex", ko: "코덱스" },
    sources: SOURCES,
    features: {
      permission: { en: "Supported", ko: "됨" },
      question: { en: "Not yet verified", ko: "확인 필요" },
      failed: { en: "Not supported (no matching hook)", ko: "안 됨 (해당 훅 없음)" },
      ended: {
        en: "Can't confirm the end. Hidden after more than 12 hours without events",
        ko: "끝났는지 확인하지 못함. 12시간 넘게 사건이 없으면 숨김",
      },
      worktime: { en: "Supported", ko: "됨" },
      summary: {
        en: "Written by Codex when picked in Settings (installed and signed in). Either agent summarizes both agents' records",
        ko: "설정에서 고르면 코덱스가 씀 (설치·로그인 필요). 어느 쪽이든 두 에이전트 기록을 함께 요약",
      },
      tokens: { en: "Supported. Tokens only for models with unknown prices", ko: "됨. 가격을 모르는 모델은 토큰만" },
      limits: { en: "Last values in Codex's records", ko: "코덱스 기록에 남은 마지막 값" },
      setup: { en: "Adds hooks; approve once on the next run", ko: "훅 추가, 다음 실행 때 한 번 승인" },
    },
  },
];

const PAIRS = { 와: ["와", "과"], 로: ["로", "으로"], 를: ["를", "을"] } as const;

/** Appends a Korean particle chosen by the last syllable's final consonant. */
export function josa(word: string, kind: keyof typeof PAIRS): string {
  const code = word.charCodeAt(word.length - 1) - 0xac00;
  const [vowel, consonant] = PAIRS[kind];
  if (code < 0 || code > 11171) return word + vowel;
  const final = code % 28;
  if (final === 0) return word + vowel;
  if (kind === "로" && final === 8) return word + vowel; // ㄹ takes 로
  return word + consonant;
}

export function joinNames(names: string[], locale: Locale): string {
  if (names.length <= 1) return names[0] ?? "";
  const head = names.slice(0, -1);
  const last = names[names.length - 1];
  if (locale === "en") return `${head.join(", ")} and ${last}`;
  const lead = head.slice(0, -1);
  const joiner = josa(head[head.length - 1], "와");
  return [...lead, joiner].join(", ") + " " + last;
}

export function agentNames(locale: Locale): string[] {
  return AGENTS.map((a) => a.name[locale]);
}
