# Docs

| Document | What it covers |
| --- | --- |
| [privacy.md](privacy.md) | what porch reads, stores and sends, and how the data flows |
| [design/data-sources.md](design/data-sources.md) | what is read, and from where; facts checked directly on a Mac |
| [design/states.md](design/states.md) | the rules that turn events into states |
| [design/architecture.md](design/architecture.md) | stack, modules, event format, storage, folders |
| [design/ui.md](design/ui.md) | the screens of the menu bar popover and the main window |
| [design/note-format.md](design/note-format.md) | the format of summary notes written for Obsidian sync (`porch_format`) |
| [research/2026-10-03-usage-vs-ccusage.md](research/2026-10-03-usage-vs-ccusage.md) | seven counting errors found by comparing usage with ccusage, and their fixes |
| [decisions/](decisions/) | what was decided and why (ADRs) |

## Decision records

| No. | Decision |
| --- | --- |
| [0001](decisions/0001-observe-not-infer.md) | An observation board, not a judge |
| [0002](decisions/0002-hooks-as-primary-source.md) | Hooks as the primary source |
| [0003](decisions/0003-every-terminal-not-one.md) | Every session on this Mac, not one terminal |
| [0004](decisions/0004-stack.md) | Tauri 2 + Rust + React |
| [0005](decisions/0005-model-written-summaries.md) | A model writes the day and week summaries |
| [0006](decisions/0006-usage-analytics.md) | Usage events from the app and the landing page go to PostHog |
| [0007](decisions/0007-model-suggestions-human-applies.md) | A model writes suggestions, a person applies them |
| [0008](decisions/0008-mirror-to-a-chosen-folder.md) | Summaries are written as notes to a folder the user picks |
| [0009](decisions/0009-porch-notes-are-rewritten.md) | porch rewrites the notes in its folder |
| [0010](decisions/0010-choose-who-writes.md) | The person picks which agent writes summaries |
| [0011](decisions/0011-english.md) | Korean or English, with one set of summary instructions per language |
| [0012](decisions/0012-model-evaluates-work-style.md) | A model evaluates the week's way of working |
| [0013](decisions/0013-status-board-shows-progress.md) | Sessions shows progress and takes you to the session's window; no "unread" marker |

## Diagrams

The PNGs in `images/` are rendered from the HTML in `images/src/`. To change a diagram, edit the HTML and render it again.
