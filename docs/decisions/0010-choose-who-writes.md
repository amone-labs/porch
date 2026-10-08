# 0010. The person picks which agent writes summaries

- Status: accepted
- Date: 2026-10-03
- Supersedes: "written by Claude Code (`claude -p`)" in [0005](0005-model-written-summaries.md) and [0007](0007-model-suggestions-human-applies.md)

## Context

0005 and 0007 had summaries and suggestions written by Claude Code alone. Hitting the Claude usage limit stopped summaries, automatic summaries, suggestions and blocker labelling all at once (2026-10-03, met by the maintainer). Someone who uses only Codex got no written summary at all.

## Decision

- Settings › General › Summary has a **Summary agent** choice: Claude Code or Codex. Both names are shown in English. Summaries (day, week, month, automatic), suggestions and blocker labelling all follow this choice.
- porch never switches to the other agent by itself: the company the material goes to changes, so a person chooses. When a limit is hit, porch says so, and the notification's "Change agent" goes straight to the summary section in Settings. The failure notification for an automatic summary also names the limit.
- The model is remembered per agent. Claude Code offers sonnet/opus/haiku; Codex offers the models `codex debug models` lists (only those it exposes, in Codex's order), and "Default" is the model Codex picks. If the list cannot be read, only "Default" shows.
- Saved summaries stay as they are. Making or rewriting one uses the agent picked now. If rewriting fails (a limit, a broken answer), the saved summary is not deleted. Errors show only on screen and in notifications, never in files or notes. A report records its `provider`; an older report without one is read as written by Claude Code.
- Both agents run on the same principles: in the summary-only folder (`runner/`), without user settings, hooks or plugins, and without saving the conversation.
  - Claude Code: `claude -p --no-session-persistence --tools "" --setting-sources "" --strict-mcp-config` (as in 0005).
  - Codex: `codex exec --ephemeral --ignore-user-config --ignore-rules --disable hooks --disable plugins -s read-only -c web_search="disabled" -c project_doc_max_bytes=0`, with the system instructions in `developer_instructions` and the answer read from the `-o` file.
- Taking an API key and sending to OpenAI or Anthropic directly is left out for now.

## Consequences

- With Codex picked, the summary material (parts of requests and answers, errors, file paths, git history) and the suggestion material (CLAUDE.md/AGENTS.md) go to OpenAI: the same place they go during normal Codex use. Settings and the privacy notes say so.
- Codex has no switch to turn off its shell tool. The read-only sandbox lets it change nothing, but the model can read files. The instructions ask for a single JSON answer.
- Because of `--ignore-user-config`, the default model and reasoning effort set in the user's `config.toml` are not used. porch's settings choose the model.
- No conversation is saved, so tokens spent on summaries do not show in porch's usage. The same holds for Claude Code summaries.
- Limit and login errors are recognised by the error sentence the agent prints. If that sentence changes, the error shows as a general one, with the original text beside it.
