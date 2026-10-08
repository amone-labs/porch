# 0007. A model writes suggestions, a person applies them

- Status: accepted (the writing agent is replaced by [0010](0010-choose-who-writes.md))
- Date: 2026-10-01

## Context

Project checks (the first layer) only count what the blockers were and how many there were. The user wanted to know what to change because of them. That sentence comes only from reading and grouping the records, so a model has to write it. [0005](0005-model-written-summaries.md) covers only the day and week summaries.

## Decision

- Suggestions are written once, on request, by Claude Code (`claude -p`, no tools, no user settings, no saved conversation).
- What is sent: the check results; each blocker's date, title, kind, evidence, cause and that day's suggestion; the project's `CLAUDE.md` and `AGENTS.md` and `~/.claude/CLAUDE.md` (up to 8,000 characters each); and earlier suggestions. Request text, answers and conversations are not sent.
- The suggestion feature never writes the user's settings files (installing hooks is separate). The screen offers the text to paste and a copy button, nothing more. The person marks whether they applied it.
- A suggestion may target only values that were in the material. Evidence dates are kept only if they are in the material, and a suggestion with no date left is dropped.
- Habit suggestions rest only on recorded behaviour and never judge the person's character.
- Effect is a comparison of the same numbers over periods of the same length before and after applying, and is never written as cause.

## Consequences

- Blocker text and the instruction files go to Claude Code once more: the same place they go during normal Claude Code use.
- Suggestions and what was applied are kept in `suggestions.json`, and the app and the CLI share a lock on it.
