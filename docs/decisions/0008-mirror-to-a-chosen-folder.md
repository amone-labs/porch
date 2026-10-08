# 0008. Summaries are written as notes to a folder the user picks

- Status: accepted (the rule for edited notes is replaced by [0009](0009-porch-notes-are-rewritten.md))
- Date: 2026-10-02

## Context

Summaries are visible only when the app is open. The user wanted them to pile up in Obsidian and to arrive as notifications. Email was dropped because it needs a server and an account.

## Decision

- porch writes day, week and month summaries as Markdown to a folder the user picks, outside the data dir (usually `porch/` inside an Obsidian vault).
- It writes only the summary Markdown and its frontmatter (`porch_format`, kind, date, title, project names, active minutes, API-equivalent cost when cost display is on, and when it was made).
- ~~Notes a person edited after porch wrote them are not overwritten.~~ Replaced by [0009](0009-porch-notes-are-rewritten.md): porch rewrites the notes in the folder. A deleted note is written again on the next save.
- Obsidian's vault list (`~/Library/Application Support/obsidian/obsidian.json`) is only read. If its format differs, an empty list and the reason are shown.
- Nothing leaves this Mac through this feature. If the vault is in iCloud, syncing is done by the user's iCloud.

## Consequences

- Summaries stay in a note app the person already uses. The note format (`docs/design/note-format.md`) becomes a public promise.
- Failing to save to the folder never fails the summary.
