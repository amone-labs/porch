# 0002. Hooks as the primary source

- Status: accepted
- Date: 2026-09-27

## Context

[0001](0001-observe-not-infer.md) settled on "use only what the agents report". Where to receive those facts was still open.

- Claude Code's session registry (`~/.claude/sessions/<pid>.json`) gives `busy`/`waiting`/`idle` and why a session waits, but only for Claude Code, and not the order of events.
- Codex's record files keep no permission requests. Whether Codex is waiting cannot be told from its records alone.
- Both Claude Code and Codex support hooks. Orca receives both agents' states the same way.

## Decision

A small hook binary, `porch-hook`, is installed into Claude Code and every Codex config folder, and hook events are the primary source. Claude Code's session registry is a secondary source that fills in sessions already open before the hooks were installed. Transcripts are not used for state.

The hook appends one line to a per-day, append-only file and exits at once. It exits with code 0 on any failure, so it never blocks the agent. It writes no prompts and no tool input.

## Consequences

- An install step is needed. It edits the user's settings files, so installing and removing must preserve existing hooks exactly.
- Codex sessions started before the hooks were installed are not visible.
- Events accumulate, which makes recorded replay tests easy.
- Distribution through the App Store becomes impossible: a sandboxed app cannot edit another app's settings files.
