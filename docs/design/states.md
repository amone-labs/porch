# State rules

The last event received sets the state. Nothing is interpreted. Two markers sit beside the state, and a marker never changes the state. ([ADR 0001](../decisions/0001-observe-not-infer.md)) A finished turn gets no marker: porch cannot know whether the person has seen the result. ([ADR 0013](../decisions/0013-status-board-shows-progress.md))

## States (one per session)

| State | `state` value | Entered on | Left on |
| --- | --- | --- | --- |
| Needs permission | `permission` | hook `PermissionRequest`, or session registry `waiting` + `"permission prompt"` | `PostToolUse` of the same tool, `PermissionDenied` |
| Has a question | `question` | session registry `waiting` + `"input needed"`, or `PreToolUse` of `AskUserQuestion` | `PostToolUse` of the same tool |
| Stopped on error | `failed` | hook `StopFailure` (Claude Code only) | the next `UserPromptSubmit` |
| Done | `turn_done` | hook `Stop` | the next `UserPromptSubmit` |
| Working | `running` | `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PreCompact`, session registry `busy` | an event that leads to one of the states above |
| Ended | `ended` | `SessionEnd`, or the process is gone (checked with `pid` + `procStart`) | none |

Codex has no hook that matches `StopFailure`, so Codex sessions never show `failed`.

## Markers (separate from the state)

- **Stale observation:** a working session with no event for more than 10 minutes shows "last heard 12 min ago". It does not mean the work stopped; it may be a long command.
- **Subagents:** `SubagentStart` and `SubagentStop` count the subagents that are running. They show even when the parent's turn is over.

## Work folder marker

The count of uncommitted changes belongs to the work folder, not to a session. When several sessions use the same folder, it shows once, at the head of that folder's group. porch does not guess who made the changes.

## Screen order

Needs permission → Has a question → Stopped on error → Working → Done. What needs the person comes first. The menu bar count and "Waiting on you" count only the first three states.

## Matching a request with its resolution

A permission request or a question is matched with the event that resolves it by `tool_use_id` (Claude Code hooks pass it to `PreToolUse`, `PermissionRequest` and `PostToolUse`). Another tool finishing in parallel does not clear Needs permission. When one side has no id, the tool name is used; when neither has one, the next tool event resolves it.

## New sessions

A session that has sent only `SessionStart` is Done. So is a session that was already open before the hooks were installed and is known only from the session registry.

## When a session has ended

A Claude Code session has ended when the session registry has no live entry for it (`pid` + `procStart` match). Codex offers no way to check that a session is alive, so a Codex session with no event for more than 12 hours is hidden. Ended sessions leave the list at once.

## When two sources disagree

When a hook event and the session registry say different things, the **more recent** one wins: the hook event's time (`t`) is compared with the registry's `statusUpdatedAt`.

## Unknown events

A new kind of event, or a format version porch does not know, leaves the state unchanged and only adds to a count of unknown events. Settings shows that count, so a format change gets noticed.

## Tests

`state` is a pure function: it takes the previous state and an event and returns the new state. The tests in `crates/core/src/state.rs` feed it event sequences and check the resulting states. Change those tests first when this document changes.
