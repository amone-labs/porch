# 0003. Every session on this Mac, not one terminal

- Status: accepted
- Date: 2026-09-27

## Context

Orca already shows a `working`, `blocked`, `waiting` or `done` state per tab through hooks. For someone who works only inside Orca, the first version of this app overlaps with Orca.

But Claude Code is the same program whether it runs in Ghostty, iTerm, VS Code or Orca, and its data piles up in one place, `~/.claude`. Orca's Claude Code account folder held only login information.

## Decision

The target is every terminal and IDE that launches Claude Code. The app does not integrate with a particular terminal tool; it hooks directly into `~/.claude` and the Codex config folders. Orca sees only the tabs in its own window; this app sees every session on this Mac, wherever it was launched.

## Consequences

- No dependence on a particular terminal. It works without Orca.
- Jumping to a tab works differently in each terminal, so it moves to a second step. The hook records terminal identifiers ahead of time.
- A different folder set with `CLAUDE_CONFIG_DIR`, and multiple `CODEX_HOME`s, have to be found and installed separately.
