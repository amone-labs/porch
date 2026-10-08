# 0001. An observation board, not a judge

- Status: accepted (the "check later" marker is replaced by [0013](0013-status-board-shows-progress.md))
- Date: 2026-09-27

## Context

The first design read the transcript files and judged each session as "stuck", "waiting for my answer", "done, needs review" or "abandoned". A last reply ending in a question mark meant waiting for an answer, a failed last command meant stuck, and uncommitted files this session had edited meant needs review.

A critical review pointed out that none of these rules proves the user has to step in.

- A failing test may be the agent in the middle of fixing it.
- A reply ending in a question mark may not ask for anything, and a permission wait has no question mark.
- The overlap between paths touched by file-editing tools and git changes is no proof of ownership. Another session or the user can overwrite them, and edits from the shell, formatters or subagents are missed.
- A quiet log does not mean a long command has finished.

After seeing a wrong state a few times, the user opens the tabs to check, and then the app has no reason to exist.

## Decision

State comes only from events the agent itself reports (hooks, Claude Code's session registry). Transcript content is never interpreted. There are six states: Needs permission, Has a question, Stopped on error, Done, Working and Ended, each with defined events that enter and leave it. ~~"Needs review", which needed interpretation, becomes a "check later" marker the user clicks~~ Replaced by [0013](0013-status-board-shows-progress.md): a finished turn gets no marker. "Abandoned" becomes the "stale observation" marker.

## Consequences

- Much less room for a wrong state. When one is wrong, the only cause is a missed event, which is easy to trace.
- The first version gives up conveniences such as a one-line description, stuck detection and changes per session.
- Uncommitted changes show only per work folder.
