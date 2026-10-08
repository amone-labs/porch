# 0012. A model evaluates the week's way of working

- Status: accepted
- Date: 2026-10-05
- Related: [0001](0001-observe-not-infer.md), [0005](0005-model-written-summaries.md), [0007](0007-model-suggestions-human-applies.md), [0010](0010-choose-who-writes.md)

## Context

Summaries say what was done. The user also wanted to see at a glance whether they use AI well, and to learn patterns they had not noticed themselves (turns stopped on error piling up on one day, requests they had to say again). The frame for judging is the four criteria of an evaluation skill the user wrote: how they direct the AI, how they use it, how they ask, and how they weigh trade-offs. Judging needs the records read and grouped, so a model has to write it.

## Decision

- The Insights screen has two layers. Observed numbers (time, errors, concurrent sessions, checkpoints) are counted by replaying hook records with the state rules (0001), with no model. The evaluation (scores on four items, direction changes, requests said again) is written by the model.
- The evaluation is made once, only for finished weeks, right after the weekly summary, with the same summary agent (0010). If the evaluation fails, the weekly summary stays as it is.
- The material is the same as for day summaries (0005). The day summary step also picks out candidate direction changes and request notes; the weekly evaluation reads only those, the observed numbers and last week's goal. Only each session's first request is sent, up to 1,500 characters.
- Every score needs a quote from a request, and a quote that does not match the records is dropped. The sentences describing each score come only from the rubric fixed in code. It never judges the person's character or ability (the same rule as 0007).
- It can be turned off in Settings; then the model is not called. The observed numbers remain.
- Next week's goal compares the same measure over two periods of the same length. It is never written as cause.

## Consequences

- Each weekly summary adds one model call. Weeks with fewer than 20 requests are skipped.
- Some request text goes to the summary agent once more: the same place it goes during normal use of that agent.
- ~~The observed "idle time" includes Done turns after they were checked. The screen says so.~~ The check marker went away with [0013](0013-status-board-shows-progress.md): every Done turn counts.
- The topics overlap with Claude Code's `/insights` (time of day, parallel sessions, friction, CLAUDE.md suggestions). `/insights` runs only when called, reads only Claude Code sessions and has the model judge satisfaction and friction. porch does not recreate that judgement. It looks at set weeks automatically, counts observed numbers from hook records, checks quotes against the records, compares goals over periods of the same length, and covers Codex too.
