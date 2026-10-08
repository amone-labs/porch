# 0005. A model writes the day and week summaries

- Status: accepted (the writing agent is replaced by [0010](0010-choose-who-writes.md): the person picks Claude Code or Codex)
- Date: 2026-09-28

## Context

The first version built only the session status board and left summaries for later in the roadmap ([0001](0001-observe-not-infer.md): "no guessing"; the first version: "no network"). But what the user wanted in the first place was to look back, whenever they liked at the end of the day, on what they did in each project, and to use that every day like a product. A summary built from records alone is never wrong, but it cannot say in sentences what was done.

## Decision

The day summary (`porch today`) and the week summary are written by a model, by running Claude Code in the background (`claude -p`).

- The material is gathered mechanically (`digest`): requests, the agent's final answers, Claude Code's automatic summaries (`away_summary`), edited files, commands, commits, and numbers that need no interpretation (work time, time of day, tool errors, denied permissions).
- The model is told to write only facts that are in the material. Numbers go straight from the material to the screen, not through the model.
- The summary run has no tools, no user settings (no hooks, MCP or plugins) and saves no conversation, and it runs in its own folder so it never ends up in the next summary's material.
- The result is saved as a JSON file and an HTML page per date and opened in the browser.

[0001](0001-observe-not-infer.md) is a decision about **state** and stays as it is: the status board still does not guess. The page says that the summary was written by a model.

## Consequences

- Conversation content goes to Claude Code once more, for the summary: the same place it goes during normal Claude Code use. The first version's "no network" now applies only to the status board.
- One day summary takes about 40 seconds and some subscription usage. Opening the same day again uses the saved result; only `--refresh` writes it again.
- The summary material, including request text, is saved in `~/Library/Application Support/porch/reports/`.
