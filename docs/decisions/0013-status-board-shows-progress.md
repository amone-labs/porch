# 0013. Sessions shows progress and takes you to the session's window; finished turns get no "unread" marker

- Status: accepted
- Date: 2026-10-06
- Supersedes: "needs review becomes a 'check later' marker the user clicks" in [0001](0001-observe-not-infer.md)

## Context

The Sessions screen was defined only as "the sessions open now and their states". What a person does on it, and what they get from it, had not been set.

0001 put a "check later" marker the person clicks on finished turns. The app had no click to confirm. Expanding a row and loading the last reply counted as seen, and leaving it expanded marked the next turn as seen within 2 seconds. `states.md` said "by clicking confirm", so the document and the behaviour differed. The screen showed it as "unread", but a person could not explain what they had to read to clear it.

porch does not see the terminal screen. It cannot know whether a person read a result in the terminal, so "seen" always had to stand in for some action inside porch.

## Decision

- The purpose of Sessions is to show at a glance what every open session is doing now, including sessions that are working.
- A person does two things there: looks at the screen, and clicks a row to bring that session's terminal window or tab to the front. Reading the last reply, granting permission and answering do not happen inside porch. porch sends no input to agents (0001 as is).
- The "check later" marker on finished turns (`needs_ack`, "unread" on screen) is removed. Every finished turn is Done.
- The popover stays a smaller version of all open sessions. Clicking a row opens Sessions in the main window. Taking you to a session's window happens only in the main window.
- Rejected (maintainer's decision, 2026-10-05):
  - Counting a session as seen when its window is opened from porch. A trip to the terminal that skips porch cannot be told apart, and the marker still needs explaining.
  - Clearing it only when the next request is sent. Sessions whose result needs nothing more would keep piling up.
  - Keeping a way to read the last reply inside porch. Results are read in the terminal.
  - Granting permission or answering inside porch. That means sending input to the agent, which goes past 0001's observe-only principle.

## Consequences

- The menu bar count and "Waiting on you" count only Needs permission, Has a question and Stopped on error.
- Ended sessions leave the list at once. The rule that hides Codex sessions after 12 hours has no exception any more.
- Sessions with a new result are not told apart from ones already seen. The person checks results in the terminal.
- `ack_session`, `porch ack`, `acks.jsonl` and expand-to-read (`session_tail`, `board::last_replies`) are removed. An existing `acks.jsonl` is no longer read.
- Taking you to the session's window works only for Orca tabs for now (`ORCA_PANE_KEY` → `orca terminal switch`). For other terminals the row shows but cannot be clicked. The scope is not decided yet.
- Whether the screen does its job is judged three ways: whether the states are right, whether opening terminal tabs to check on sessions happens less, and whether people actually use the jump to a session's window.
