/**
 * The compare table's facts, shared by both languages. A mark other than "–" needs the tool's own page to say so
 * (sources below, checked 2026-10-01). "–" means the source did not say; X only where it says no.
 */
export type Mark = "O" | "X" | "△" | "–";
export type Feature = "agents" | "anyTerminal" | "waiting" | "summary" | "time" | "stuck" | "suggest" | "usage" | "runs";

export const TOOLS: { name: string; url?: string }[] = [
  { name: "Porch" },
  { name: "Chit", url: "https://www.producthunt.com/products/chit-2" },
  { name: "ClaudeUsageBar", url: "https://github.com/Artzainnn/ClaudeUsageBar" },
  { name: "claudebill", url: "https://github.com/aitechexplore/claudebill" },
  { name: "Orca", url: "https://www.onorca.dev/" },
  { name: "Langfuse", url: "https://langfuse.com/integrations/developer-tools/claude-code" },
];

export const FEATURES: Feature[] = ["agents", "anyTerminal", "waiting", "summary", "time", "stuck", "suggest", "usage", "runs"];

// Columns follow TOOLS. Notes on the less obvious marks:
// Chit: Claude Code only and "isn't a time tracker" (its maker, on Product Hunt); it reads transcripts, runs nothing.
// Orca: shows its own worktrees' agents ("awaiting permission"), not sessions started in other terminals; tracks
//   Claude and Codex usage and rate-limit resets. claudebill: API-equivalent cost estimates, no limits.
// Langfuse: traces Claude Code and Codex through hooks with token cost, no subscription limits.
export const MARKS: Record<Feature, Mark[]> = {
  agents: ["O", "X", "–", "–", "O", "O"],
  anyTerminal: ["O", "–", "–", "–", "△", "–"],
  waiting: ["O", "–", "–", "–", "O", "–"],
  summary: ["O", "O", "–", "–", "–", "–"],
  time: ["O", "X", "–", "–", "–", "–"],
  stuck: ["O", "–", "–", "–", "–", "–"],
  suggest: ["O", "–", "–", "–", "–", "–"],
  usage: ["O", "–", "O", "△", "O", "△"],
  runs: ["X", "X", "–", "–", "O", "–"],
};
