# What porch reads, stores and sends

No porch account or server is needed for your work records. Session state, tokens and cost never leave your Mac, and the hook sends nothing. What follows is the full list for the app. The website also collects visit statistics, and requests to the installer link (the README's included) are counted on the server; both are described in [ADR 0006](decisions/0006-usage-analytics.md). The privacy policy for the website and the app is at [getporch.pages.dev/privacy](https://getporch.pages.dev/privacy/).

## How it works

```
agent hooks ──▶ porch-hook ──▶ metadata events ──┐
Claude Code session registry ────────────────────┴──▶ session status
agent token records ──▶ usage (stays on your Mac)
agent transcripts + git ──▶ day digest ──▶ your Claude Code or Codex ──▶ summaries
```

- Session state comes only from what the agents report, never from reading their text ([ADR 0001](decisions/0001-observe-not-infer.md), [state rules](design/states.md)).
- `porch-hook` exits 0 on every path, prints nothing, and records no prompts or tool input.
- Summaries are written by the Claude Code or Codex already signed in on your Mac, whichever you pick. porch never switches between them by itself.

## Reads

Claude Code and Codex session records on this Mac (including token counts) and your projects' git history. With Claude limits on, what Claude Code passes to its status line (model, context window, limits). When you ask for suggestions, your projects' `CLAUDE.md`/`AGENTS.md` and `~/.claude/CLAUDE.md` (up to 8,000 characters each). When you connect Obsidian, its vault list (`~/Library/Application Support/obsidian/obsidian.json`).

## Stores

Session state, summaries with part of the records behind them, usage, open work and blockers, next week's goal, the weekly evaluation, suggestions and whether you applied them, and settings. All in `~/Library/Application Support/porch`, except summary notes, which go to the folder you chose ([ADR 0008](decisions/0008-mirror-to-a-chosen-folder.md)). Session detection itself stores no conversation text.

## Sends

- **Summaries:** the records a summary needs (parts of requests and answers, errors, file paths, git history) go to the model through the agent you picked, Claude Code (to Anthropic) or Codex (to OpenAI), and use that subscription. A saved summary is reused; rewriting it calls the model again, and a failed rewrite keeps the saved one ([ADR 0005](decisions/0005-model-written-summaries.md), [ADR 0010](decisions/0010-choose-who-writes.md)).
- **Suggestions:** only when you ask. Project checks, blocker details, earlier suggestions and the instruction files above go to the model through the same agent. Requests, answers and conversations are not sent. You apply any change yourself ([ADR 0007](decisions/0007-model-suggestions-human-applies.md)).
- **Blocker classification:** blocker titles, evidence and causes go through the same agent to assign each blocker a kind.
- **Weekly evaluation:** for a finished week, after the automatic weekly summary, or when you press **Make evaluation** in **Insights** or run `porch insight --refresh`. Weeks with fewer than 20 requests are skipped. It sends each session's first request (up to 1,500 characters), the direction changes and request notes from the day summaries, the week's observed counts and times, the days without a daily summary, and last week's goal, scores and items marked as wrong, through the same agent. Turn it off with **Insights evaluation** in **Settings**; the observed numbers stay ([ADR 0012](decisions/0012-model-evaluates-work-style.md)).
- **App usage:** on by default. The main window sends a fixed list of events with a random anonymous ID and basic device info to PostHog Cloud US: app opened, which features are on (at launch and when the date changes while it runs), screen and tab viewed, clicking a session to bring its window to the front (with whether it worked), summary and suggestions finished (with the agent and, on failure, only the kind of failure), onboarding finished, hooks installed, summary notification opened, suggestion applied, dismissed, undone or copied, report exported, and notes folder connected or disconnected. The full list is the table in [ADR 0006](decisions/0006-usage-analytics.md). Never sessions, project names or paths, branches, requests, answers, summaries, suggestion text, error messages, tokens or cost. Turn it off with **Send app usage events** in **Settings**; builds without a PostHog key send nothing.
- **Notes:** written only to the folder you chose; nothing leaves this Mac. If that folder syncs (iCloud, for example), the sync is yours.
- **Updates:** the app checks for a new version at launch and every 24 hours while it runs, and downloads one only when you choose to install it.
