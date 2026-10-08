# Data sources

Only three sources are used, in order of trust. Transcripts are never used to judge state; they feed session titles, summary material and token counts.

The facts below were checked directly on the developer's Mac on 2026-09-27 (Claude Code 2.1.283). All are undocumented internal formats and can change at any time.

| Rank | Source | What it tells | Limits |
| --- | --- | --- | --- |
| 1 | **Our hooks** (Claude Code, Codex) | session start and end, prompt submitted, before and after a tool, permission request, turn over, stopped on error, subagent start and end | only sessions started after the install |
| 2 | **Claude Code session registry** `~/.claude/sessions/<pid>.json` | sessions open since before the install, current status | Claude Code only |
| 3 | **Transcripts** (Claude Code `logPath`, Codex `threads.rollout_path`) | title, summary material, token counts | never used to judge state |

## One place, wherever it was launched

Claude Code is the same program whichever terminal or IDE runs it (Ghostty, iTerm, VS Code, Orca and so on), and its data piles up in one place, `~/.claude`.

| Path | Content |
| --- | --- |
| `~/.claude/projects/<cwd with characters replaced>/<sessionId>.jsonl` | transcript; subagents under `<sessionId>/subagents/` |
| `~/.claude/sessions/<pid>.json` | running sessions |
| `~/.claude/settings.json` | hook settings |

Orca's Claude Code account folder (`~/Library/Application Support/orca/claude-accounts/<id>/`) held only login information; the transcripts were in `~/.claude`.

### Exceptions

- **`CLAUDE_CONFIG_DIR`:** run with this environment variable, the data goes to another folder. Settings accepts extra folders, and hooks are installed there too.
- **Cloud sessions:** sessions running outside this Mac, such as on claude.ai or remote runs, are out of scope.
- **Codex:** does not gather in one place. See below.

## Claude Code session registry

Each running session has one file.

| Field | Use |
| --- | --- |
| `pid`, `procStart` | whether the process is alive. `pid` alone cannot tell a reused number, so `procStart` is compared too. `procStart` is in **UTC**, so `ps -o lstart` must also run with `TZ=UTC` to match |
| `sessionId` | key for matching hook events |
| `cwd` | work folder |
| `status` | `busy` / `waiting` / `idle` |
| `waitingFor` | only when `status` is `waiting`: `"input needed"` or `"permission prompt"` |
| `logPath` | transcript path |
| `kind` | `interactive` / `bg` |
| `name`, `updatedAt`, `statusUpdatedAt` | title candidate, time of last change |

`status` mirrors Claude Code's internal state. The mapping found in the executable is `running → busy`, `requires_action → waiting`, `idle → idle`. `waitingFor` is `"input needed"` when the wait comes from the question tool or a dialog, and `"permission prompt"` otherwise.

## Finding the claude command

Summaries run `claude`. An app opened from Finder, the Dock or a login item gets only launchd's minimal `PATH` (`/usr/bin:/bin:/usr/sbin:/sbin`), so the name alone does not find it as it would in a terminal. The search order: `PORCH_CLAUDE_BIN` → the app's `PATH` → install locations (`~/.local/bin`, `~/.claude/local`, `/opt/homebrew/bin`, `/usr/local/bin`, `~/.npm-global/bin`, `~/.bun/bin`) → the login shell (`$SHELL -lic 'command -v claude'`). Check with `porch doctor`.

## Codex

- **Several config folders.** The default is `~/.codex`. Orca uses `~/Library/Application Support/orca/codex-accounts/<id>/home` as `CODEX_HOME` per account. On the developer's Mac, all recent Codex work was in Orca's folders.
- **Thread list:** the `threads` table in each config folder's `state_5.sqlite` (`id`, `rollout_path`, `cwd`, `title`, `updated_at`, `git_branch` and more). Subagent links are in `thread_spawn_edges(parent_thread_id, child_thread_id, status)`.
- **Conversation body:** the JSONL at `rollout_path` (`sessions/YYYY/MM/DD/rollout-<time>-<id>.jsonl`).
- **The records keep no permission requests.** Counting every event kind in the 60 most recent files found none. Whether Codex is waiting can be known only through hooks.
- **Hooks:** `hooks.json` accepts 8 events: `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `PostToolUse`, `SubagentStart`, `SubagentStop`, `Stop` (confirmed from the entries Orca installed). The JSON fields the hooks pass have not been checked yet.

## Record formats to know when counting requests

- **A continuation summary is not a request.** After compacting a conversation, Claude Code puts a summary of the earlier conversation into a record with `type: "user"`, `isCompactSummary: true` and `isVisibleInTranscriptOnly: true`. Older records start with "This session is being continued from a previous conversation…", recent ones with `<artifact-content-authored-by-others/>` (checked 2026-10-05). No person sent it, so whatever its text looks like, `isCompactSummary` leaves it out of request counts and summary material.

## Record formats to know when counting usage

Checked on 2026-10-03 by comparing the last 30 days of records on the developer's Mac with ccusage (Claude Code 2.1.283, Codex 0.159). The evidence and numbers are in the [research note](../research/2026-10-03-usage-vs-ccusage.md).

- **Claude Code writes one answer over several lines.** The same `message.id` appears on several lines, and `output_tokens` grows on later lines (37 → 37 → 211). The last line holds that answer's full usage.
- **Claude Code copies conversations into other session files.** When a session continues in another worktree, the original file ends with `continued-in` (`continuedInSessionId`), and the new file holds the earlier conversation with the same `message.id` and `timestamp`. Sessions split with `/branch` (titles ending in `⑂`) do the same. The same answer is in two files, so an id is counted once for the whole day, not once per file.
- **Codex records live only in the folder of the day they started.** `sessions/YYYY/MM/DD/` is the start date, and one session once kept writing to the same file for more than ten days. Archived sessions are in `archived_sessions/`.
- **Codex running totals sometimes break.** The first `token_count` of a forked session (`session_meta.forked_from_id`) inherits the parent's running total (`total_token_usage` in the millions while `last_token_usage` is one call's share). A total can also restart mid-session at that call's amount.
- **Codex local models cannot be told apart by name.** LM Studio's default model `openai/gpt-oss-20b` has the same name as a paid OpenRouter model. Only `session_meta.model_provider` being `ollama` or `lmstudio` tells them apart (Codex source `codex-rs/model-provider-info/src/lib.rs`).

## Codex trusts hooks by position

Codex does not run a new hook right away. Each hook gets `enabled` and `trusted_hash` in the config folder's `config.toml`, under a key like `[hooks.state."<hooks.json path>:stop:0:0"]`, and works only after the user approves it in Codex. The `0:0` in the key is which group and which hook within that event's list. So inserting a hook ahead of another tool's hook shifts that hook's position and drops the approval it already had.

## Hook install rules

- Existing hooks (other tools, Orca for one, install their own) are never overwritten; only our entries are added. Removal takes out only our entries. Our entries are recognised only by the `# porch` marker at the end of the command.
- If our entry already exists, it is fixed in place; it is appended at the end only when missing. Codex trust hashes are never written.
- A settings file that cannot be read is left alone. Before writing, it is backed up to `<file>.porch-backup-<time>`, written to a temporary file and swapped in at once.
- Claude Code gets hooks in `~/.claude/settings.json` and any registered extra config folders; Codex gets them in `hooks.json` of every `CODEX_HOME` found.
- Of what it receives, the hook writes only the session ID, event kind, tool name, work folder, record path and time. It writes no prompts and no tool input.
- Terminal identifiers (`TERM_PROGRAM`, `ORCA_PANE_KEY`, `TMUX_PANE`, `ITERM_SESSION_ID`) are recorded too, for jumping to tabs later.
- In an excluded folder, the hook writes nothing and exits at once.

## Orca

Orca hooks in the same way to track a `working`, `blocked`, `waiting` or `done` state per tab, and `orca worktree ps --json` gives the agent type, state, last reply and the tool in use. Orca sees only the tabs in its own window. Whether to use Orca as an extra source is not decided yet.

## Facts the first draft got wrong

The design draft generalised from a few files, and a critical review found four of its claims wrong. They are kept here so the same mistake is not made again.

| The draft said | In fact |
| --- | --- |
| The registry's `status` has two values, `busy` / `idle` | there is also `waiting`, with the reason in `waitingFor` |
| Every transcript line has `cwd`, `gitBranch` and `timestamp` | about 2,400 of 12,673 lines in the 12 most recent files lack them |
| The newest Codex record on the developer's Mac was from 2026-08-22 | that was the default folder only; Orca's folders had records from that same day |
| Codex command results are `custom_tool_call_output` | there is also `function_call_output` |
